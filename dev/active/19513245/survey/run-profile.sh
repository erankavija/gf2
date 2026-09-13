#!/usr/bin/env bash
# Current-code and prototype consumer profile of the byte-field routes
# (jit:19513245).
#
# Builds the harness under --release with the repository MSRV toolchain, then
# runs the profile session `REPETITIONS` times. Each session proves nothing by
# itself: the repetitions are what give every profile figure an interval, and
# the summary reports the median over the sessions with its order-statistic
# interval. The first session additionally records hardware counters for the
# cases `counter-cases.txt` names.
#
# Every session runs inside its own invocation of the repository's CCX1 lock
# wrapper in its --full-host mode, so a queued sibling gets the host between
# sessions. Builds take the shared side of the mutex through
# `scripts/cargo-budget.sh`, the only supported shared acquirer.
#
# `repetitions.log` is the series' append-only execution log: each session's
# start with the executable digest, its completion, and the discard of a
# session that did not finish. A re-run with the same output directory resumes
# after the last completed session, re-runs an unfinished one whole, and
# refuses to resume when the executable a session runs differs from the first
# session's record.
#
# Usage: dev/active/19513245/survey/run-profile.sh <output-dir>
#
# Everything written is produced by this run.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
ISSUE=19513245
FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"
MANIFEST="${HERE}/consumer/Cargo.toml"
# Nine is the smallest session count whose order-statistic interval for a
# median at 95% coverage excludes the extreme sessions ([x(2), x(8)], 96.1%).
REPETITIONS=9

OUT="${1:?usage: run-profile.sh <output-dir>}"
mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"

export RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
export CARGO_TARGET_DIR="${REPO}/target/${ISSUE}-survey"
BIN="${CARGO_TARGET_DIR}/release"
LOG="${OUT}/repetitions.log"

# ------------------------------------------------------------------ build
# Builds finish before any timed work.
(cd "${REPO}" && ./scripts/cargo-budget.sh cargo build --release --locked \
    --manifest-path "${MANIFEST}") >>"${OUT}/build.log" 2>&1

# ---------------------------------------------------------------- identity
touch "${LOG}"
DIGEST="$(sha256sum "${BIN}/consumer-profile" | cut -d' ' -f1)"
FIRST="$(sed -n 's/^rep-[0-9]* start .* consumer-profile=\([0-9a-f]*\).*/\1/p' "${LOG}" | head -n 1)"
if [[ -n "${FIRST}" && "${FIRST}" != "${DIGEST}" ]]; then
    echo "consumer-profile changed since the first session (${FIRST} -> ${DIGEST})" >&2
    exit 2
fi

# ------------------------------------------------------------- provenance
if [[ -e "${OUT}/host.txt" ]]; then
    {
        echo
        echo "## resumed"
        echo "# resumed_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "# gf2 revision (informational): $(git -C "${REPO}" rev-parse HEAD)"
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
    echo "## rustc"
    rustc --version --verbose
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
    echo "## load average at start"
    uptime
    echo
    echo "## harness executables"
    for binary in consumer-profile consumer-arm consumer-verify; do
        printf '%s %s\n' "$(sha256sum "${BIN}/${binary}" | cut -d' ' -f1)" "${binary}"
    done
    echo
    echo "## harness sources"
    (cd "${REPO}" && find "dev/active/${ISSUE}/survey" -type f \
        \( -name '*.rs' -o -name '*.toml' -o -name '*.lock' -o -name '*.py' -o -name '*.sh' -o -name '*.txt' -o -name '*.json' \) \
        | sort | xargs sha256sum)
    echo
    echo "## production crate sources"
    (cd "${REPO}" && find crates/gf2-core/src crates/gf2-kernels-simd/src -type f \
        | sort | xargs sha256sum | sha256sum | sed 's/ .*/ (sha256 over the sorted per-file digest list)/')
} >"${OUT}/host.txt"
fi

# The ladder is written once, outside the mutex, so the summary reads the same
# case list the sessions measured.
"${BIN}/consumer-profile" ladder >"${OUT}/ladder.json"

# --------------------------------------------------------------- timed work
echo "profile execution log: ${LOG}" >&2
for index in $(seq 1 "${REPETITIONS}"); do
    rep="rep-$(printf '%02d' "${index}")"
    if grep -q "^${rep} done " "${LOG}"; then
        continue
    fi
    starts="$(awk -v rep="${rep}" '$1 == rep && $2 == "discarded" {list = ""}
        $1 == rep && $2 == "start" {list = list (list == "" ? "" : " ") $3}
        END {print list}' "${LOG}")"
    if [[ -n "${starts}" ]]; then
        echo "${rep} discarded $(date -u +%Y-%m-%dT%H:%M:%SZ) unfinished start(s) ${starts}; partial output removed, the session re-runs whole" >>"${LOG}"
    fi
    rm -rf "${OUT:?}/${rep}"
    mkdir -p "${OUT}/${rep}"
    counters=no
    if [[ "${index}" == 1 ]]; then
        counters=yes
    fi
    echo "${rep} start $(date -u +%Y-%m-%dT%H:%M:%SZ) consumer-profile=${DIGEST} load=[$(uptime)]" >>"${LOG}"
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host \
        bash "${HERE}/sweep.sh" "${OUT}/${rep}" "${BIN}" "${HERE}" "${counters}"
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

# Generated-code evidence reads a file and times nothing, so it runs after the
# mutex is released.
"${HERE}/disassemble.sh" "${BIN}"

python3 -B "${HERE}/summarize-profile.py" "${OUT}" >"${OUT}/profile-summary.md"

echo "profile written to ${OUT}" >&2
