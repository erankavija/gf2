#!/usr/bin/env bash
# Current-code consumer profile of bit-storage costs (jit:04b85d10).
#
# Builds the harness under --release with the repository MSRV toolchain, then
# runs the profile session `REPETITIONS` times. Each session proves the
# compared routes agree, sweeps the case ladder and records hardware counters
# and call-graph profiles for the routes the sweep identifies as the expensive
# ones; the first also records allocation-site traces. Every session runs
# inside its own invocation of the repository's CCX1 lock wrapper in its
# --full-host mode, because the ladder includes twelve- and twenty-four-worker
# rows the six-core pin cannot serve, and a queued sibling gets the host
# between sessions. Builds take the shared side of the mutex through
# `scripts/cargo-budget.sh`, the only supported shared acquirer.
#
# The repetitions are what give every profile figure an interval: the
# summaries report the median over the sessions with its order-statistic
# interval. `repetitions.log` is the series' append-only execution log: each
# session's start with the executable digest, its completion, and the discard
# of a session that did not finish. A re-run with the same output directory
# resumes after the last completed session, re-runs an unfinished one whole,
# and refuses to resume when an executable a session runs or a file that
# defines its cases differs from the first session's record in `host.txt`.
#
# Usage: dev/active/04b85d10/survey/run-profile.sh <output-dir>
#
# Everything written is produced by this run.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
ISSUE=04b85d10
FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"
MANIFEST="${HERE}/gf2-side/Cargo.toml"
# Nine is the smallest session count whose order-statistic interval for a
# median at 95% coverage excludes the extreme sessions ([x(2), x(8)], 96.1%).
REPETITIONS=9

OUT="${1:?usage: run-profile.sh <output-dir>}"
mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"

export RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
export CARGO_TARGET_DIR="${REPO}/target/consumer-profile-${ISSUE}"
BIN="${CARGO_TARGET_DIR}/release"
LOG="${OUT}/repetitions.log"
# The harness files that define what a session measures, besides the two
# executables it runs.
SESSION_FILES=(sweep.sh profile-cases.py counter-cases.txt report-cases.txt alloc-trace-cases.txt)

# ------------------------------------------------------------------ build
# Builds finish before any timed work.
(cd "${REPO}" && ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${MANIFEST}") >>"${OUT}/build.log" 2>&1

# ---------------------------------------------------------------- identity
# A resumed series measures what its first session measured: the same
# `consumer-profile`, as the log's first start line records it, and the same
# `consumer-verify` and session files, as the first session's host record
# lists them.
touch "${LOG}"
DIGEST="$(sha256sum "${BIN}/consumer-profile" | cut -d' ' -f1)"
FIRST="$(sed -n 's/^rep-[0-9]* start .* consumer-profile=\([0-9a-f]*\).*/\1/p' "${LOG}" | head -n 1)"
if [[ -n "${FIRST}" && "${FIRST}" != "${DIGEST}" ]]; then
    echo "consumer-profile changed since the first session (${FIRST} -> ${DIGEST})" >&2
    exit 2
fi
unchanged() {
    local recorded current
    recorded="$(awk -v name="$1" '$2 == name {print $1; exit}' "${OUT}/host.txt")"
    current="$(sha256sum "$2" | cut -d' ' -f1)"
    if [[ "${recorded}" != "${current}" ]]; then
        echo "$1 changed since the first session (${recorded:-unrecorded} -> ${current})" >&2
        exit 2
    fi
}

# ------------------------------------------------------------- provenance
# Written once; a resumed run appends to it instead.
if [[ -e "${OUT}/host.txt" ]]; then
    unchanged consumer-verify "${BIN}/consumer-verify"
    for file in "${SESSION_FILES[@]}"; do
        unchanged "dev/active/${ISSUE}/survey/${file}" "${HERE}/${file}"
    done
    {
        echo
        echo "## resumed"
        echo "# resumed_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "# gf2 revision (informational): $(git -C "${REPO}" rev-parse HEAD)"
        echo "# unchanged since the first session: consumer-profile, consumer-verify, ${SESSION_FILES[*]}"
        uptime
    } >>"${OUT}/host.txt"
else
{
    echo "# consumer profile host record - produced by run-profile.sh"
    echo "# generated_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision (informational): $(git -C "${REPO}" rev-parse HEAD)"
    echo
    echo "## uname"
    uname -a
    echo
    echo "## /etc/os-release"
    cat /etc/os-release
    echo
    echo "## rustc"
    rustc --version --verbose
    echo
    echo "## cargo"
    cargo --version
    echo
    echo "## RUSTFLAGS"
    echo "${RUSTFLAGS:-<unset>}"
    echo
    echo "## perf"
    perf --version
    echo
    echo "## lscpu"
    lscpu
    echo
    echo "## cache topology of cpu0"
    for index in /sys/devices/system/cpu/cpu0/cache/index*; do
        printf '%s level=%s type=%s size=%s shared=%s\n' \
            "$(basename "${index}")" \
            "$(cat "${index}/level")" \
            "$(cat "${index}/type")" \
            "$(cat "${index}/size")" \
            "$(cat "${index}/shared_cpu_list")"
    done
    echo
    echo "## governors"
    for policy in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
        printf '%s %s\n' "${policy}" "$(cat "${policy}")"
    done 2>/dev/null || echo "no cpufreq sysfs entries"
    echo
    echo "## smt"
    cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo "no smt control"
    echo
    echo "## perf_event_paranoid"
    cat /proc/sys/kernel/perf_event_paranoid
    echo
    echo "## generic backend-stall event"
    perf stat -e stalled-cycles-backend:u -- true 2>&1 | grep -E 'stalled-cycles-backend' || true
    echo
    echo "## valgrind"
    valgrind --version 2>&1 || echo "valgrind not installed"
    echo
    echo "## load average at start"
    uptime
    echo
    echo "## harness executables"
    for binary in consumer-arm consumer-profile consumer-verify; do
        printf '%s %s\n' "$(sha256sum "${BIN}/${binary}" | cut -d' ' -f1)" "${binary}"
    done
    echo
    echo "## harness sources"
    (cd "${REPO}" && find "dev/active/${ISSUE}/survey" -type f \
        \( -name '*.rs' -o -name '*.toml' -o -name '*.lock' -o -name '*.py' -o -name '*.sh' -o -name '*.txt' \) \
        | sort | xargs sha256sum)
    echo
    echo "## production crate sources"
    (cd "${REPO}" && find crates/gf2-core/src crates/gf2-coding/src crates/gf2-kernels-simd/src -type f \
        | sort | xargs sha256sum | sha256sum | sed 's/ .*/ (sha256 over the sorted per-file digest list)/')
} >"${OUT}/host.txt"
fi

# --------------------------------------------------------------- timed work
# The ladder is generated outside the mutex; each session then runs inside one
# wrapper invocation, so the host mutex changes hands once per session rather
# than once per measured process.
python3 "${HERE}/profile-cases.py" >"${OUT}/cases.jsonl"

echo "profile execution log: ${LOG}" >&2
for index in $(seq 1 "${REPETITIONS}"); do
    rep="rep-$(printf '%02d' "${index}")"
    if grep -q "^${rep} done " "${LOG}"; then
        continue
    fi
    # A session that did not finish leaves partial output, including the
    # call-graph data file it was recording beside the session directory. The
    # log records the discard of every start since the session's last discard,
    # and the session is re-run whole.
    starts="$(awk -v rep="${rep}" '$1 == rep && $2 == "discarded" {list = ""}
        $1 == rep && $2 == "start" {list = list (list == "" ? "" : " ") $3}
        END {print list}' "${LOG}")"
    if [[ -n "${starts}" ]]; then
        echo "${rep} discarded $(date -u +%Y-%m-%dT%H:%M:%SZ) unfinished start(s) ${starts}; partial output removed, the session re-runs whole" >>"${LOG}"
    fi
    rm -rf "${OUT:?}/${rep}"
    rm -f "${OUT}"/perf-*.data "${OUT}"/perf-*.data.old
    mkdir -p "${OUT}/${rep}"
    trace=no
    if [[ "${index}" == 1 ]]; then
        trace=yes
    fi
    echo "${rep} start $(date -u +%Y-%m-%dT%H:%M:%SZ) consumer-profile=${DIGEST} load=[$(uptime)]" >>"${LOG}"
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host \
        bash "${HERE}/sweep.sh" "${OUT}/${rep}" "${BIN}" "${HERE}" "${OUT}/cases.jsonl" "${trace}"
    echo "${rep} done $(date -u +%Y-%m-%dT%H:%M:%SZ) load=[$(uptime)]" >>"${LOG}"
done
if ! grep -q '^series done ' "${LOG}"; then
    echo "series done $(date -u +%Y-%m-%dT%H:%M:%SZ) sessions=${REPETITIONS}" >>"${LOG}"
fi

{
    echo
    echo "## load average at end"
    uptime
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"${OUT}/host.txt"

# Generated-code evidence reads a file and times nothing, so it runs after
# the mutex is released.
"${HERE}/disassemble.sh" "${OUT}" "${BIN}"

python3 -B "${HERE}/summarize-profile.py" "${OUT}" >"${OUT}/profile-summary.md"
python3 -B "${HERE}/summarize-attribution.py" "${OUT}" >"${OUT}/attribution-summary.md"

echo "profile written to ${OUT}" >&2
