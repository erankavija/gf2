#!/usr/bin/env bash
# Current-code consumer profile of bit-storage costs (jit:04b85d10).
#
# Builds the harness under --release, proves the compared routes agree, then
# sweeps the case ladder and records hardware counters and call-graph profiles
# for the routes the sweep identifies as the expensive ones. Every timed
# process runs through the repository's CCX1 lock wrapper, one process per
# lock acquisition, so the sweep serializes against sibling benchmark work
# without holding the host mutex for its whole duration.
#
# Usage: dev/active/04b85d10/survey/run-profile.sh <output-dir>
#
# Everything written is produced by this run.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
ISSUE=04b85d10
EXT="${REPO}/target/consumer-profile-${ISSUE}"
FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"
MANIFEST="${HERE}/gf2-side/Cargo.toml"

OUT="${1:?usage: run-profile.sh <output-dir>}"
mkdir -p "${OUT}/perf-stat" "${OUT}/perf-report"

export CARGO_TARGET_DIR="${EXT}/target"
BIN="${CARGO_TARGET_DIR}/release"

# ------------------------------------------------------------------ build
# Builds finish before any timed work, and they hold the shared side of the
# benchmark mutex so they can never overlap a timed run.
CARGO_CI_NO_SCCACHE=1 flock -s /tmp/gf2-ccx1.lock "${REPO}/scripts/cargo-budget.sh" \
    cargo build --release --manifest-path "${MANIFEST}" >"${OUT}/build.log" 2>&1

# ------------------------------------------------------------- provenance
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
    for binary in consumer-arm consumer-profile consumer-verify; do
        printf '%s %s\n' "$(sha256sum "${BIN}/${binary}" | cut -d' ' -f1)" "${binary}"
    done
    echo
    echo "## harness sources"
    (cd "${REPO}" && find "dev/active/${ISSUE}/survey" -type f \
        \( -name '*.rs' -o -name '*.toml' -o -name '*.py' -o -name '*.sh' \) \
        | sort | xargs sha256sum)
} >"${OUT}/host.txt"

# --------------------------------------------------------------- timed work
# The ladder is generated outside the mutex; the whole timed session then runs
# inside one wrapper invocation, so the host mutex changes hands once rather
# than once per measured process.
python3 "${HERE}/profile-cases.py" >"${OUT}/cases.jsonl"

GF2_BENCH=1 "${FLOCK}" bash "${HERE}/sweep.sh" "${OUT}" "${BIN}" "${HERE}"

{
    echo
    echo "## load average at end"
    uptime
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"${OUT}/host.txt"

# Generated-code evidence reads a file and times nothing, so it runs after
# the mutex is released.
"${HERE}/disassemble.sh" "${OUT}"

python3 "${HERE}/summarize-profile.py" "${OUT}/profile.jsonl" \
    >"${OUT}/profile-summary.md"

echo "profile written to ${OUT}" >&2
