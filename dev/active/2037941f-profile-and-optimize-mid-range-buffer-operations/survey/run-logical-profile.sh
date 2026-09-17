#!/usr/bin/env bash
# Profile session for the three logical-buffer baselines (jit:18a87159).
#
# `build` compiles the driver and checks the frozen case list; it times nothing
# and runs in an ordinary session. `window` is the measurement and refuses to
# start unless the scheduled-window runner exported GF2_BENCH_WINDOW=1.
#
# One session proves nothing by itself: the repetitions give every figure an
# order-statistic interval, and the summary reports the median over them. Each
# repetition runs inside its own invocation of the full-host lock wrapper, so a
# queued sibling gets the host between repetitions.
#
# `repetitions.log` is the append-only execution log: a repetition's start with
# the driver digest, its completion, and the discard of one that did not
# finish. A re-run with the same output directory resumes after the last
# completed repetition and refuses to resume when the driver changed.
#
# Usage: dev/active/2037941f-.../survey/run-logical-profile.sh build|window [<output-dir>]
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

STORY=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations
SURVEY="${STORY}/survey"
MANIFEST="${REPO}/${SURVEY}/harness/Cargo.toml"
TARGET="${REPO}/target/bb769456-arms"
DRIVER="${TARGET}/release/logical-profile"
FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"
# Nine is the smallest repetition count whose order-statistic interval for a
# median at 95% coverage excludes the extreme repetitions ([x(2), x(8)], 96.1%).
REPETITIONS=9
# One second of measured operations per counter pass. The driver counts its own
# calls, so the session assumes no call constant about this host; the counters
# cover the whole process, including its start-up and fixture construction.
SECONDS_PER_PASS=1
# Two disjoint counter groups, each within the core's general-purpose counters,
# so neither group is multiplexed.
GROUP_ISSUE=cycles,instructions,branches,branch-misses
GROUP_MEMORY=L1-dcache-loads,L1-dcache-load-misses,LLC-loads,LLC-load-misses
# The one case per baseline whose sampled symbol shares the report attributes.
RECORDED_CASES=(
    "xor-8w-a64-warm@public-xor-a"
    "row-xor-8w-full-warm@row-xor-a"
    "nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a"
)

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1
export RAYON_NUM_THREADS=1

MODE="${1:-}"
case "${MODE}" in
    build | window) ;;
    *) echo "usage: $0 build|window [<output-dir>]" >&2; exit 2 ;;
esac
OUT="${2:-dev/bench_results/2037941f/logical-profile}"

# Builds finish before any timed work.
CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${MANIFEST}" --bin logical-profile
mapfile -t CASES < <("${DRIVER}" cases)
[[ "${#CASES[@]}" -gt 0 ]] || { echo 'the driver declares no profile case' >&2; exit 2; }

if [[ "${MODE}" == build ]]; then
    printf '%s\n' "${CASES[@]}"
    echo "# ${#CASES[@]} frozen profile cases; driver ${DRIVER}" >&2
    echo '# non-timed preparation complete' >&2
    exit 0
fi
[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo 'the logical-buffer profile session runs only in the scheduled benchmark window' >&2
    exit 2
}

mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"
LOG="${OUT}/repetitions.log"
touch "${LOG}"
DIGEST="$(sha256sum "${DRIVER}" | cut -d' ' -f1)"
FIRST="$(sed -n 's/^rep-[0-9]* start .* logical-profile=\([0-9a-f]*\).*/\1/p' "${LOG}" | head -n 1)"
if [[ -n "${FIRST}" && "${FIRST}" != "${DIGEST}" ]]; then
    echo "logical-profile changed since the first repetition (${FIRST} -> ${DIGEST})" >&2
    exit 2
fi

if [[ -e "${OUT}/host.txt" ]]; then
    {
        echo
        echo '## resumed'
        echo "# resumed_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        uptime
    } >>"${OUT}/host.txt"
else
    {
        echo '# logical-buffer profile host record - produced by run-logical-profile.sh'
        echo "# generated_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "# gf2 revision (informational): $(git -C "${REPO}" rev-parse HEAD)"
        echo
        echo '## uname'
        uname -a
        echo
        echo '## rustc'
        rustc --version --verbose
        echo
        echo '## perf'
        perf --version
        echo
        echo '## perf_event_paranoid'
        cat /proc/sys/kernel/perf_event_paranoid
        echo
        echo '## lscpu'
        lscpu
        echo
        echo '## cache topology of cpu0'
        for index in /sys/devices/system/cpu/cpu0/cache/index*; do
            printf '%s level=%s type=%s size=%s shared=%s\n' \
                "$(basename "${index}")" "$(cat "${index}/level")" \
                "$(cat "${index}/type")" "$(cat "${index}/size")" \
                "$(cat "${index}/shared_cpu_list")"
        done
        echo
        echo '## governors'
        for policy in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            printf '%s %s\n' "${policy}" "$(cat "${policy}")"
        done 2>/dev/null || echo 'no cpufreq sysfs entries'
        echo
        echo '## smt'
        cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo 'no smt control'
        echo
        echo '## driver'
        printf '%s logical-profile\n' "${DIGEST}"
        echo
        echo '## load average at start'
        uptime
    } >"${OUT}/host.txt"
fi

printf '%s\n' "${CASES[@]}" >"${OUT}/cases.txt"
printf '%s\n' "${RECORDED_CASES[@]}" >"${OUT}/recorded-cases.txt"
echo "profile execution log: ${LOG}" >&2

for index in $(seq 1 "${REPETITIONS}"); do
    rep="rep-$(printf '%02d' "${index}")"
    if grep -q "^${rep} done " "${LOG}"; then
        continue
    fi
    if grep -q "^${rep} start " "${LOG}"; then
        echo "${rep} discarded $(date -u +%Y-%m-%dT%H:%M:%SZ) an earlier start did not finish; partial output removed, the repetition re-runs whole" >>"${LOG}"
    fi
    rm -rf "${OUT:?}/${rep}"
    mkdir -p "${OUT}/${rep}"
    echo "${rep} start $(date -u +%Y-%m-%dT%H:%M:%SZ) logical-profile=${DIGEST} load=[$(uptime)]" >>"${LOG}"
    GF2_BENCH=1 GF2_BENCH_WINDOW=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host \
        bash "${HERE}/sweep-logical-profile.sh" "${OUT}/${rep}" "${DRIVER}" \
        "${SECONDS_PER_PASS}" "${GROUP_ISSUE}" "${GROUP_MEMORY}" \
        "${OUT}/cases.txt" "${OUT}/recorded-cases.txt"
    echo "${rep} done $(date -u +%Y-%m-%dT%H:%M:%SZ) load=[$(uptime)]" >>"${LOG}"
done
if ! grep -q '^series done ' "${LOG}"; then
    echo "series done $(date -u +%Y-%m-%dT%H:%M:%SZ) repetitions=${REPETITIONS}" >>"${LOG}"
fi

{
    echo
    echo '## load average at end'
    uptime
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"${OUT}/host.txt"

python3 -B "${SURVEY}/summarize-logical-profile.py" "${OUT}" >"${OUT}/profile-summary.md"
echo "profile written to ${OUT}" >&2
