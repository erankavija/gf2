#!/usr/bin/env bash
# Profiler attribution of the measured gf2 and gf2x paths (jit:53c5a8c0).
#
# Usage (from the worktree root):
#   dev/active/53c5a8c0/survey/run-profile.sh <output-dir>
#
# The study's receipts answer which path is faster. This session answers why:
# for every row of `profile-cases.tsv` it counts hardware events over the
# path's own process and samples that process's cycles, so instruction
# throughput, branch behaviour, load and store traffic, dispatch-token stalls
# and the symbol the cycles land in are attributed per path.
#
# The counted process is `profile-arm`, which issues a fixed number of logical
# calls of one cell's operation on that cell's frozen fixture and does nothing
# else. Its argument is the call count, so a counter divided by the recorded
# operation count is a per-operation figure of the path rather than of a
# window protocol.
#
# Two event sets are counted because this host's PMU retires six general
# counters and a six-event group multiplexes: the throughput set and the memory
# set share `cycles`, so the summary can check the two sets against each other.
#
# Each row is repeated `REPETITIONS` times so every derived figure carries an
# order-statistic interval over the repetitions rather than a single reading.
# Nine is the smallest count whose median interval at 95% coverage excludes the
# extreme repetitions ([x(2), x(8)], 96.1%).
#
# `CALLS_TARGET_MS` is the wall time each row's counted process aims at. The
# call count that reaches it is derived by this script from two calibration runs
# of the same row, never typed: a path this study has not profiled before has no
# rate anyone could type honestly. Two runs at different call counts are used so
# process start-up falls out of the slope between them; a single run would
# charge start-up to the per-call rate and undershoot the target on every fast
# row.
#
# Every row takes the full host through the canonical mutex, because a counter
# read beside another worker's build attributes that build's cycles to this
# path. The mutex is taken per row rather than for the whole session so a
# sibling's build is not blocked for the session's length; a sibling that lands
# between two repetitions of one row shows up as a wide interval on that row
# rather than as a silent bias. It is not a timed comparison and no receipt
# depends on it.
#
# The per-row mutex is taken by re-invoking this script in its `--row` mode
# through the canonical wrapper, because the wrapper runs a command and cannot
# run a shell function.
#
# Re-running with the same output directory resumes: a row already recorded for
# every repetition is skipped, and the log is append-only.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }

FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"
CASES="${HERE}/profile-cases.tsv"
REPETITIONS=9
CALLS_TARGET_MS=300
CALIBRATION_CALLS=200
SAMPLE_FREQUENCY=4000
THROUGHPUT_EVENTS='cycles,instructions,ex_ret_ops,branches,branch-misses'
MEMORY_EVENTS='cycles,ls_dispatch.ld_dispatch,ls_dispatch.store_dispatch,de_dis_dispatch_token_stalls1.int_phy_reg_file_rsrc_stall,de_dis_dispatch_token_stalls1.store_queue_rsrc_stall'

MODE=session
if [[ "${1:-}" == "--row" ]]; then
    MODE=row
    shift
fi

OUT="${1:?usage: run-profile.sh [--row] <output-dir> [row fields...]}"
mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"
LOG="${OUT}/profile.log"
COUNTERS="${OUT}/counters.jsonl"
RUNS="${OUT}/runs.jsonl"
REPORTS="${OUT}/report"
mkdir -p "${REPORTS}"
touch "${LOG}" "${COUNTERS}" "${RUNS}"
export PATH="${HOME}/.cargo/bin:${PATH}"

python3 "${HERE}/check-profile-cases.py" "${CASES}" >&2

digest() { sha256sum "$1" | cut -d' ' -f1; }
binary_for() { echo "${REPO}/target/53c5a8c0-arms-${1}/release/profile-arm"; }
for variant in conservative native; do
    [[ -x "$(binary_for "${variant}")" ]] || {
        echo "$(binary_for "${variant}") is absent; run ${HERE#"${REPO}/"}/build-arms.sh" >&2
        exit 2
    }
done

IDENTITY="profile-arm-conservative=$(digest "$(binary_for conservative)")"
IDENTITY+=" profile-arm-native=$(digest "$(binary_for native)")"
IDENTITY+=" profile-cases.tsv=$(digest "${CASES}")"
FIRST="$(sed -n 's/^identity //p' "${LOG}" | head -n 1)"
if [[ -n "${FIRST}" && "${FIRST}" != "${IDENTITY}" ]]; then
    echo "an executable or the case table changed since the first session" >&2
    echo "first:   ${FIRST}" >&2
    echo "current: ${IDENTITY}" >&2
    exit 2
fi
[[ -n "${FIRST}" || "${MODE}" == "row" ]] || echo "identity ${IDENTITY}" >>"${LOG}"

if [[ "${MODE}" == "row" ]]; then
    :
elif [[ ! -e "${OUT}/host.txt" ]]; then
{
    echo "# profile host record - produced by dev/active/53c5a8c0/survey/run-profile.sh"
    echo "# generated_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision (informational): $(git -C "${REPO}" rev-parse HEAD)"
    echo "# identity: ${IDENTITY}"
    echo; echo "## uname"; uname -a
    echo; echo "## perf"; perf --version
    echo; echo "## perf_event_paranoid"; cat /proc/sys/kernel/perf_event_paranoid
    echo; echo "## lscpu"; lscpu
    echo; echo "## smt"; cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo "no smt control"
    echo; echo "## governors"
    for policy in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
        printf '%s %s\n' "${policy}" "$(cat "${policy}")"
    done 2>/dev/null || echo "no cpufreq sysfs entries"
    echo; echo "## load average at start"; cat /proc/loadavg
} >"${OUT}/host.txt"
else
    { echo; echo "## resumed $(date -u +%Y-%m-%dT%H:%M:%SZ)"; cat /proc/loadavg; } >>"${OUT}/host.txt"
fi

# Wall nanoseconds of one `profile-arm` process at a given call count.
wall_ns() {
    local calls="$1"; shift
    local start; start=$(date +%s%N)
    "$@" --calls "${calls}" --warmup 0 >/dev/null
    echo $(( $(date +%s%N) - start ))
}

# One row's whole observation: calibration, sampling, then the repetitions of
# both counted event sets. Runs inside the mutex; writes nothing but its own
# records.
observe_row() {
    local label="$1" cell="$2" build="$3" arm="$4" path="$5" case="$6"
    local binary; binary="$(binary_for "${build}")"
    local -a invoke=(env "GF2_CROSSOVER_PATH=${path}" "${binary}"
        --arm "${arm}" --case "${case}" --label "${label}")

    # Calibration at two call counts, so process start-up cancels: the slope
    # between them is the per-call cost and the intercept is discarded.
    local single double slope_ns calls
    single=$(wall_ns "${CALIBRATION_CALLS}" "${invoke[@]}")
    double=$(wall_ns $(( 2 * CALIBRATION_CALLS )) "${invoke[@]}")
    slope_ns=$(( (double - single) / CALIBRATION_CALLS ))
    (( slope_ns > 0 )) || slope_ns=1
    calls=$(( CALLS_TARGET_MS * 1000000 / slope_ns ))
    (( calls >= CALIBRATION_CALLS )) || calls=${CALIBRATION_CALLS}
    echo "row ${label} calibrated ${CALIBRATION_CALLS}->${single}ns, $(( 2 * CALIBRATION_CALLS ))->${double}ns, ${slope_ns}ns/call -> ${calls} calls" >>"${LOG}"

    # The run record: what the process did and which path it selected.
    "${invoke[@]}" --calls "${calls}" --warmup 1 \
        | python3 -c 'import json,sys
record = json.loads(sys.stdin.readline())
record["cell"] = sys.argv[1]
record["build"] = sys.argv[2]
record["crossover_path"] = sys.argv[3]
print(json.dumps(record, sort_keys=True))' "${cell}" "${build}" "${path}" >>"${RUNS}"

    # Cycle sampling, once per row: the symbol attribution does not need an
    # interval, because it reports where a path's cycles are and every
    # repetition of the same process runs the same code.
    perf record -q -e cycles:u -F "${SAMPLE_FREQUENCY}" --no-buildid-cache \
        -o "${REPORTS}/${label}.data" -- \
        "${invoke[@]}" --calls "${calls}" --warmup 1 >/dev/null 2>>"${LOG}"
    {
        echo "# cycle samples of ${label} (${arm} arm, cell ${cell})"
        echo "# recorded by run-profile.sh at ${SAMPLE_FREQUENCY} Hz over ${calls} logical calls"
        perf report --stdio --no-children --percent-limit 0.4 \
            --sort symbol -i "${REPORTS}/${label}.data" 2>/dev/null
    } >"${REPORTS}/${label}.txt"
    rm -f "${REPORTS}/${label}.data"

    local repetition set events
    for repetition in $(seq 1 "${REPETITIONS}"); do
        for set in throughput memory; do
            case "${set}" in
                throughput) events="${THROUGHPUT_EVENTS}" ;;
                memory)     events="${MEMORY_EVENTS}" ;;
            esac
            perf stat -e "${events}" -x, --no-big-num \
                -o "${OUT}/.stat-${label}" -- "${invoke[@]}" --calls "${calls}" --warmup 1 >/dev/null
            python3 -c 'import json,sys
counters = {}
for line in open(sys.argv[1]):
    line = line.strip()
    if not line or line.startswith("#"):
        continue
    fields = line.split(",")
    if len(fields) < 5:
        continue
    value, _unit, event, _run, pct = fields[:5]
    counters[event] = None if value.startswith("<") else int(value)
    counters[event + ".enabled_pct"] = float(pct) if pct else None
print(json.dumps({
    "schema": "clmul-crossover-profile-counters-v1",
    "label": sys.argv[2], "cell": sys.argv[3], "arm": sys.argv[4],
    "build": sys.argv[5], "crossover_path": sys.argv[6],
    "event_set": sys.argv[7], "repetition": int(sys.argv[8]),
    "calls": int(sys.argv[9]), "counters": counters,
}, sort_keys=True))' "${OUT}/.stat-${label}" "${label}" "${cell}" "${arm}" "${build}" \
                "${path}" "${set}" "${repetition}" "${calls}" >>"${COUNTERS}"
        done
    done
    rm -f "${OUT}/.stat-${label}"
    echo "row ${label} done ${REPETITIONS} repetitions of both event sets" >>"${LOG}"
}

if [[ "${MODE}" == "row" ]]; then
    shift
    observe_row "$@"
    exit 0
fi

echo "profile execution log: ${LOG}" >&2
echo "session start $(date -u +%Y-%m-%dT%H:%M:%SZ) load=$(cut -d' ' -f1-3 /proc/loadavg)" >>"${LOG}"

while IFS=$'\t' read -r label cell build arm path case; do
    [[ -n "${label}" && "${label}" != \#* ]] || continue
    if grep -q "^row ${label} done " "${LOG}"; then
        echo "row ${label} already recorded; skipping" >&2
        continue
    fi
    echo "row ${label}" >&2
    "${FLOCK}" --full-host "${BASH_SOURCE[0]}" --row "${OUT}" \
        "${label}" "${cell}" "${build}" "${arm}" "${path}" "${case}"
done <"${CASES}"

echo "session complete $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"${LOG}"
echo "complete" >&2
