#!/usr/bin/env bash
# Repeated profile series of the steady-state LDPC arms (jit:3be770d5).
#
# Runs profile-session.sh `REPETITIONS` times, each session inside its own
# `dev/scripts/ccx1-bench-flock.sh --full-host` invocation, because the cases
# include 12- and 24-worker rows the six-core pin cannot serve and a queued
# sibling gets the host between sessions. The repetitions give every profile
# figure an interval: per-session figures are summarized by the median over
# the sessions with its order-statistic interval, sampled shares by Wilson
# intervals over the pooled samples.
#
# Builds finish before this script: it measures the executables the
# preparation build identity records and refuses to start when their digests
# differ. `repetitions.log` is the series' append-only execution log: each
# session's start with the executable digests, its completion, and the
# discard of a session that did not finish or whose every case failed. A
# session without one usable case stops the series. A re-run with the same output
# directory resumes after the last completed session, re-runs an unfinished
# one whole, and refuses to resume when an executable or a case file differs
# from the first session's record.
#
# Usage (from the worktree root):
#   dev/active/3be770d5/survey/run-profile.sh <output-dir>
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"
BIN="${REPO}/target/ldpc-throughput/release"
INPUTS="${REPO}/target/ldpc-inputs"
QUALITY="${REPO}/dev/bench_results/c077a88b/v3-preparation/quality"
IDENTITY="${REPO}/dev/bench_results/3be770d5/preparation/build-identity.json"
# Nine is the smallest session count whose order-statistic interval for a
# median at 95% coverage excludes the extreme sessions ([x(2), x(8)], 96.1%).
REPETITIONS=9
EXECUTABLES=(ldpc-profile ldpc-alloc-census)
SESSION_FILES=(profile-session.sh profile-cases.tsv arms.json)

OUT="${1:?usage: run-profile.sh <output-dir>}"
mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"
LOG="${OUT}/repetitions.log"
export PATH="${HOME}/.cargo/bin:${PATH}"

# ----------------------------------------------------------------- identity
digest() { sha256sum "$1" | cut -d' ' -f1; }
for binary in "${EXECUTABLES[@]}"; do
    recorded="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executables"][sys.argv[2]])' \
        "${IDENTITY}" "${binary}")"
    if [[ "$(digest "${BIN}/${binary}")" != "${recorded}" ]]; then
        echo "${binary} differs from the preparation build identity" >&2
        exit 2
    fi
done
IDENTITY_LINE="ldpc-profile=$(digest "${BIN}/ldpc-profile") ldpc-alloc-census=$(digest "${BIN}/ldpc-alloc-census")"
for file in "${SESSION_FILES[@]}"; do
    IDENTITY_LINE+=" ${file}=$(digest "${HERE}/${file}")"
done
touch "${LOG}"
FIRST_IDENTITY="$(sed -n 's/^rep-[0-9]* start [^ ]* \(ldpc-profile=.*\) load=.*/\1/p' "${LOG}" | head -n 1)"
if [[ -n "${FIRST_IDENTITY}" && "${FIRST_IDENTITY}" != "${IDENTITY_LINE}" ]]; then
    echo "an executable or case file changed since the first session" >&2
    echo "first:   ${FIRST_IDENTITY}" >&2
    echo "current: ${IDENTITY_LINE}" >&2
    exit 2
fi

# --------------------------------------------------------------- provenance
if [[ ! -e "${OUT}/host.txt" ]]; then
{
    echo "# steady-state LDPC profile host record - produced by run-profile.sh"
    echo "# generated_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision (informational): $(git -C "${REPO}" rev-parse HEAD)"
    echo "# identity: ${IDENTITY_LINE}"
    echo
    echo "## uname"; uname -a
    echo; echo "## perf"; perf --version
    echo; echo "## perf_event_paranoid"; cat /proc/sys/kernel/perf_event_paranoid
    echo; echo "## lscpu"; lscpu
    echo; echo "## smt"; cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo "no smt control"
    echo; echo "## thread siblings"
    for cpu in /sys/devices/system/cpu/cpu[0-9]*; do
        printf '%s %s l3=%s\n' "$(basename "${cpu}")" "$(cat "${cpu}/topology/thread_siblings_list")" \
            "$(cat "${cpu}/cache/index3/shared_cpu_list" 2>/dev/null || echo none)"
    done
    echo; echo "## governors"
    for policy in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
        printf '%s %s\n' "${policy}" "$(cat "${policy}")"
    done 2>/dev/null || echo "no cpufreq sysfs entries"
    echo; echo "## load average at start"; uptime
} >"${OUT}/host.txt"
else
    { echo; echo "## resumed $(date -u +%Y-%m-%dT%H:%M:%SZ)"; uptime; } >>"${OUT}/host.txt"
fi

# ----------------------------------------------------------- expanded cases
# Case names resolve to executables arguments, environments and prepared
# quality outside the mutex.
python3 - "${HERE}/profile-cases.tsv" "${HERE}/arms.json" "${INPUTS}" "${QUALITY}" \
    >"${OUT}/cases.expanded.tsv" <<'PY'
import json, sys
cases, arms, inputs, quality = sys.argv[1:]
catalogue = json.load(open(arms))
for line in open(cases):
    if not line.strip() or line.startswith("#"):
        continue
    kind, label, arm, code, core_arm, batch, passes = line.rstrip("\n").split("\t")
    spec, bundle = catalogue["arms"][arm], catalogue["codes"][code]
    env = " ".join(f"{k}={v}" for k, v in sorted(spec["environment"].items()) if v)
    print("\t".join([kind, label, spec["profile_arm"], f"{inputs}/{bundle['bundle']}",
                     bundle["code"], core_arm, batch, passes,
                     f"{quality}/{arm}-{code}.json", env]))
PY

# ----------------------------------------------------------------- sessions
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
    first=no
    [[ "${index}" == 1 ]] && first=yes
    echo "${rep} start $(date -u +%Y-%m-%dT%H:%M:%SZ) ${IDENTITY_LINE} load=[$(cut -d' ' -f1-3 /proc/loadavg)]" >>"${LOG}"
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host \
        bash "${HERE}/profile-session.sh" "${OUT}/${rep}" "${BIN}" "${OUT}/cases.expanded.tsv" "${first}"
    # profile-session.sh records each case's exit status and keeps going, so
    # the summary can exclude one failed case. A session in which every case
    # failed carries no figure at all, and continuing spends the rest of the
    # window on the same failure.
    statuses="$(cat "${OUT}/${rep}"/stat/*.status "${OUT}/${rep}"/record/*.status 2>/dev/null || true)"
    if ! grep -q '^0$' <<<"${statuses}"; then
        echo "${rep} discarded $(date -u +%Y-%m-%dT%H:%M:%SZ) every profiled case failed" >>"${LOG}"
        echo "every profiled case in ${rep} failed; see ${OUT}/${rep}/*/*.err" >&2
        exit 1
    fi
    echo "${rep} done $(date -u +%Y-%m-%dT%H:%M:%SZ) load=[$(cut -d' ' -f1-3 /proc/loadavg)]" >>"${LOG}"
done
grep -q '^series done ' "${LOG}" || echo "series done $(date -u +%Y-%m-%dT%H:%M:%SZ) sessions=${REPETITIONS}" >>"${LOG}"
{ echo; echo "## load average at end"; uptime; echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"; } >>"${OUT}/host.txt"

# Summaries read files and time nothing, so they run after the mutex.
python3 -B "${HERE}/summarize-profile.py" "${OUT}" --json "${OUT}/profile-summary.json" \
    --markdown "${OUT}/profile.md" \
    || echo "summary failed; the session data is complete and the summary can be rerun" >&2
echo "profile written to ${OUT}" >&2
