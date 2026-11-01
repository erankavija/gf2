#!/usr/bin/env bash
# Resumable dynamic profile of DVB-T2 interleaver consumers (jit:9fb40c83).
# Usage: run-profile.sh [--session] <output-dir> [rep] [counters]
set -euo pipefail

[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo 'dynamic timing and perf collection run only in the scheduled benchmark window' >&2
    exit 2
}

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"
BIN="${REPO}/target/9fb40c83-arms-native/release/dvb-profile"
CASES="${HERE}/profile-cases.txt"
BUILD_RECORD="${HERE}/harness-validation.txt"
REPETITIONS=9
COUNTER_EVENTS=cycles:u,instructions:u,branches:u,branch-misses:u,cache-references:u,cache-misses:u
SAMPLE_FREQUENCY=4000
REPORT_PERCENT_LIMIT=0.5
ANNOTATE_PERCENT_LIMIT=1.0

MODE=series
TOP_LEVEL_ARGV=("$@")
if [[ "${1:-}" == --session ]]; then
    MODE=session
    shift
fi
OUT="${1:?usage: run-profile.sh [--session] <output-dir> [rep] [counters]}"
mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"
LOG="${OUT}/repetitions.log"
touch "${LOG}"

[[ -x "${BIN}" ]] || {
    echo "${BIN} is absent; run build-dvb-harness.sh before profiling" >&2
    exit 2
}
DIGEST="$(sha256sum "${BIN}" | cut -d' ' -f1)"

if [[ "${MODE}" == series ]]; then
    grep -q "^# dvb-profile sha256: ${DIGEST}$" "${BUILD_RECORD}" || {
        echo "build record does not pin dvb-profile ${DIGEST}" >&2
        exit 2
    }
    identity_arguments=()
    for setting in REPETITIONS COUNTER_EVENTS SAMPLE_FREQUENCY REPORT_PERCENT_LIMIT \
        ANNOTATE_PERCENT_LIMIT; do
        identity_arguments+=(--setting "${setting}=${!setting}")
    done
    IDENTITY="$(python3 -B "${REPO}/dev/scripts/resume_identity.py" --log "${LOG}" \
        --recorded-digests "${OUT}/host.txt" --executable "dvb-profile=${BIN}" \
        --table "profile-cases.txt=${CASES}" "${identity_arguments[@]}")"
fi

if [[ "${MODE}" == session ]]; then
    REP="${2:?session mode needs a repetition label}"
    COUNTERS="${3:?session mode needs yes/no counter selection}"
    mkdir -p "${OUT}/${REP}"
    "${BIN}" session "${OUT}/${REP}/cases.json"
    if [[ "${COUNTERS}" == yes ]]; then
        mkdir -p "${OUT}/${REP}/counters" "${OUT}/${REP}/hot"
        while IFS= read -r case; do
            [[ -n "${case}" && "${case}" != \#* ]] || continue
            set +e
            perf stat -x, --no-big-num \
                -e "${COUNTER_EVENTS}" \
                -o "${OUT}/${REP}/counters/${case}.csv" -- \
                "${BIN}" counters "${case}" \
                >"${OUT}/${REP}/counters/${case}.json" \
                2>"${OUT}/${REP}/counters/${case}.err"
            stat_rc=$?
            echo "${stat_rc}" >"${OUT}/${REP}/counters/${case}.status"
            perf record -q -e cycles:u -F "${SAMPLE_FREQUENCY}" --no-buildid-cache \
                -o "${OUT}/${REP}/hot/${case}.data" -- \
                "${BIN}" counters "${case}" \
                >"${OUT}/${REP}/hot/${case}.json" \
                2>"${OUT}/${REP}/hot/${case}.err"
            record_rc=$?
            if [[ "${record_rc}" == 0 ]]; then
                perf report --stdio --no-children --percent-limit "${REPORT_PERCENT_LIMIT}" \
                    --sort dso,symbol -F overhead,sample,dso,symbol \
                    -i "${OUT}/${REP}/hot/${case}.data" \
                    >"${OUT}/${REP}/hot/${case}.report.txt" 2>>"${OUT}/${REP}/hot/${case}.err"
                report_rc=$?
                perf annotate --stdio --percent-limit "${ANNOTATE_PERCENT_LIMIT}" \
                    -i "${OUT}/${REP}/hot/${case}.data" \
                    >"${OUT}/${REP}/hot/${case}.instructions.txt" 2>>"${OUT}/${REP}/hot/${case}.err"
                annotate_rc=$?
                if [[ "${report_rc}" != 0 || "${annotate_rc}" != 0 ]]; then
                    printf 'perf report exit %s; perf annotate exit %s\n' \
                        "${report_rc}" "${annotate_rc}" >>"${OUT}/${REP}/hot/${case}.err"
                    record_rc=1
                fi
            else
                printf 'perf record unavailable for %s (exit %s); see %s.err\n' \
                    "${case}" "${record_rc}" "${case}" \
                    >"${OUT}/${REP}/hot/${case}.report.txt"
                cp "${OUT}/${REP}/hot/${case}.report.txt" \
                    "${OUT}/${REP}/hot/${case}.instructions.txt"
            fi
            echo "${record_rc}" >"${OUT}/${REP}/hot/${case}.status"
            set -e
        done <"${CASES}"
    fi
    exit 0
fi

if [[ ! -e "${OUT}/host.txt" ]]; then
{
    echo '# DVB-T2 interleaver profile host record'
    echo "# generated_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision (informational): $(git rev-parse HEAD)"
    echo "# dvb-profile sha256: ${DIGEST}"
    printf '# top-level argv:'
    printf ' %q' "${BASH_SOURCE[0]}" "${TOP_LEVEL_ARGV[@]}"
    printf '\n'
    echo
    echo '## rustc'; rustc --version --verbose
    echo; echo '## uname'; uname -a
    echo; echo '## perf'; perf --version
    echo; echo '## perf_event_paranoid'; cat /proc/sys/kernel/perf_event_paranoid
    echo; echo '## lscpu'; lscpu
    echo; echo '## SMT'; cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo unavailable
    echo; echo '## governors'
    for policy in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
        printf '%s %s\n' "${policy}" "$(cat "${policy}")"
    done 2>/dev/null || echo unavailable
    echo; echo '## executable'; sha256sum "${BIN}"
    echo; echo '## profile case declaration'; sha256sum "${CASES}"
    echo; echo '## load average at start'; uptime
} >"${OUT}/host.txt"
else
    { echo; echo "## resumed $(date -u +%Y-%m-%dT%H:%M:%SZ)"; uptime; } >>"${OUT}/host.txt"
fi
"${BIN}" ladder >"${OUT}/ladder.json"

echo "profile execution log: ${LOG}" >&2
for index in $(seq 1 "${REPETITIONS}"); do
    rep="rep-$(printf '%02d' "${index}")"
    if grep -q "^${rep} done " "${LOG}"; then
        [[ -s "${OUT}/${rep}/cases.json" ]] || {
            echo "${rep} is logged complete but its case record is absent" >&2
            exit 2
        }
        while IFS= read -r case; do
            [[ -n "${case}" && "${case}" != \#* ]] || continue
            [[ -s "${OUT}/${rep}/counters/${case}.status" &&
               -s "${OUT}/${rep}/hot/${case}.status" ]] || {
                echo "${rep} is logged complete but perf status is absent for ${case}" >&2
                exit 2
            }
        done <"${CASES}"
        continue
    fi
    if [[ -d "${OUT}/${rep}" ]]; then
        abandoned="${OUT}/${rep}.abandoned-$(date -u +%Y%m%dt%H%M%Sz)"
        mv "${OUT}/${rep}" "${abandoned}"
        echo "${rep} discarded $(date -u +%Y-%m-%dT%H:%M:%SZ) incomplete output preserved at ${abandoned}" >>"${LOG}"
    fi
    counters=yes
    echo "${rep} start $(date -u +%Y-%m-%dT%H:%M:%SZ) ${IDENTITY} load=[$(uptime)]" >>"${LOG}"
    {
        printf '%s inner argv:' "${rep}"
        printf ' %q' "${BASH_SOURCE[0]}" --session "${OUT}" "${rep}" "${counters}"
        printf '\n'
    } >>"${LOG}"
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host \
        "${BASH_SOURCE[0]}" --session "${OUT}" "${rep}" "${counters}"
    echo "${rep} done $(date -u +%Y-%m-%dT%H:%M:%SZ) load=[$(uptime)]" >>"${LOG}"
done
grep -q '^series done ' "${LOG}" || \
    echo "series done $(date -u +%Y-%m-%dT%H:%M:%SZ) sessions=${REPETITIONS}" >>"${LOG}"
{ echo; echo '## load average at end'; uptime; } >>"${OUT}/host.txt"

python3 -B "${HERE}/summarize-profile.py" "${OUT}" >"${OUT}/profile-summary.md"
echo "profile written to ${OUT}" >&2
