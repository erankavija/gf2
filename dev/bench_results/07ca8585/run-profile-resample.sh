#!/usr/bin/env bash
# Re-sampled steady-state profile of the changed decoder (jit:07ca8585).
#
# The predecessor `3be770d5` ranks the levers this issue spends, and its
# refutation rule for that ranking is a re-sampled profile rather than a clock:
# a lever the change spends must lose its sampled share once the change is in.
# This series is that re-sampling. It runs the predecessor's own session script,
# case set and arm catalogue unchanged, so the case identities and the sampled
# quantities are the ones the ranking was derived from, and it measures the
# `after` generation of the harness this issue built.
#
# It cannot be `dev/active/3be770d5/survey/run-profile.sh`: that script pins the
# executables of its own preparation build identity and refuses a differing
# digest, which is exactly what the changed decoder produces. The executables
# here are pinned by this issue's kernel preparation build identity instead, and
# `3be770d5`'s launcher and identity are left alone.
#
# Every session runs inside its own `dev/scripts/ccx1-bench-flock.sh
# --full-host` invocation, because the case set includes 12- and 24-worker rows
# a six-core pin cannot serve. `repetitions.log` is the append-only execution
# log: each session's start with the executable digests, its completion, and the
# discard of a session that did not finish or whose every case failed. A re-run
# with the same output directory resumes after the last completed session and
# refuses to resume when an executable or a case file differs from the first
# session's record.
#
# Usage (from the worktree root):
#   dev/bench_results/07ca8585/run-profile-resample.sh <output-dir>
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
PREDECESSOR="${repo}/dev/active/3be770d5/survey"
FLOCK="${repo}/dev/scripts/ccx1-bench-flock.sh"
BIN="${repo}/target/ldpc-throughput/release"
INPUTS="${repo}/target/ldpc-inputs"
QUALITY="${repo}/dev/bench_results/c077a88b/v3-preparation/quality"
IDENTITY="${repo}/dev/bench_results/07ca8585/preparation/kernel-build-identity.json"
# Nine is the predecessor's session count, kept so the two series' medians carry
# the same order-statistic interval.
REPETITIONS=9
EXECUTABLES=(ldpc-profile ldpc-alloc-census)
SESSION_FILES=(profile-session.sh profile-cases.tsv arms.json)

OUT="${1:?usage: run-profile-resample.sh <output-dir>}"
mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"
LOG="${OUT}/repetitions.log"
export PATH="${HOME}/.cargo/bin:${PATH}"

# ----------------------------------------------------------------- identity
digest() { sha256sum "$1" | cut -d' ' -f1; }
IDENTITY_LINE=""
for binary in "${EXECUTABLES[@]}"; do
    recorded="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["profile_resample"]["executables"][sys.argv[2]])' \
        "${IDENTITY}" "${binary}")"
    if [[ "$(digest "${BIN}/${binary}")" != "${recorded}" ]]; then
        echo "${binary} differs from ${IDENTITY}" >&2
        exit 2
    fi
    IDENTITY_LINE+="${IDENTITY_LINE:+ }${binary}=$(digest "${BIN}/${binary}")"
done
for file in "${SESSION_FILES[@]}"; do
    IDENTITY_LINE+=" ${file}=$(digest "${PREDECESSOR}/${file}")"
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
    echo "# re-sampled LDPC profile host record - produced by run-profile-resample.sh"
    echo "# generated_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision: $(git -C "${repo}" rev-parse HEAD)"
    echo "# identity: ${IDENTITY_LINE}"
    echo "# re-samples: dev/bench_results/3be770d5/v4-r1-steady-profile"
    echo
    echo "## uname"; uname -a
    echo; echo "## perf"; perf --version
    echo; echo "## perf_event_paranoid"; cat /proc/sys/kernel/perf_event_paranoid
    echo; echo "## lscpu"; lscpu
    echo; echo "## smt"; cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo "no smt control"
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
# The predecessor's own expansion, unchanged: case names resolve to executables,
# arguments, environments and prepared quality outside the mutex.
python3 - "${PREDECESSOR}/profile-cases.tsv" "${PREDECESSOR}/arms.json" "${INPUTS}" "${QUALITY}" \
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
        bash "${PREDECESSOR}/profile-session.sh" "${OUT}/${rep}" "${BIN}" "${OUT}/cases.expanded.tsv" "${first}"
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
python3 -B "${PREDECESSOR}/summarize-profile.py" "${OUT}" --json "${OUT}/profile-summary.json" \
    --markdown "${OUT}/profile.md" --binary "${BIN}/ldpc-profile" \
    --binary-sha256 "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["profile_resample"]["executables"]["ldpc-profile"])' "${IDENTITY}")" \
    || echo "summary failed; the session data is complete and the summary can be rerun" >&2
echo "re-sampled profile written to ${OUT}" >&2
