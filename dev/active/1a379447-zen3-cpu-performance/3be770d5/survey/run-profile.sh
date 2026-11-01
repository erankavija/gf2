#!/usr/bin/env bash
# Repeated profile series of the steady-state LDPC arms (jit:3be770d5), over
# the executables the preparation build identity pins. Sessions take the full
# host because the cases include 12- and 24-worker rows; the repetitions give
# each figure an interval: the median over sessions with its order-statistic
# interval, sampled shares by Wilson intervals over the pooled samples.
#
# Usage (from the worktree root):
#   <this directory>/run-profile.sh <output-dir>
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
BIN="${REPO}/target/ldpc-throughput/release"
INPUTS="${REPO}/target/ldpc-inputs"
# Nine is the smallest session count whose order-statistic interval for a
# median at 95% coverage excludes the extreme sessions ([x(2), x(8)], 96.1%).
REPETITIONS=9
EXECUTABLES=(ldpc-profile ldpc-alloc-census)

OUT="${1:?usage: run-profile.sh <output-dir>}"
# shellcheck disable=SC1091
source "${HERE}/profile-series.sh"
IDENTITY="${REPO}/$(series_located 3be770d5/preparation/build-identity.json)"
QUALITY="${REPO}/$(series_quality "${IDENTITY}")"
mkdir -p "${OUT}"
OUT="$(cd "${OUT}" && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

series_identity() {
    local binary recorded
    local -a arguments=()
    for binary in "${EXECUTABLES[@]}"; do
        recorded="$(series_recorded "${IDENTITY}" executables "${binary}")"
        if [[ "$(series_digest "${BIN}/${binary}")" != "${recorded}" ]]; then
            echo "${binary} differs from the preparation build identity" >&2
            return 2
        fi
        arguments+=(--executable "${binary}=${BIN}/${binary}")
    done
    python3 -B "$(series_located resume_identity.py)" --log "${OUT}/repetitions.log" \
        "${arguments[@]}" --setting "REPETITIONS=${REPETITIONS}" \
        --shell "profile-session.sh=${HERE}/profile-session.sh" \
        --table "profile-cases.tsv=${HERE}/profile-cases.tsv" \
        --json "arms.json=${HERE}/arms.json"
}
series_cases() {
    python3 - "${HERE}/profile-cases.tsv" "${HERE}/arms.json" "${INPUTS}" "${QUALITY}" <<'PY'
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
}
profile_series "${OUT}" "${BIN}" "${HERE}/profile-session.sh" "${REPETITIONS}" \
    "steady-state LDPC profile host record - produced by run-profile.sh"

# Summaries read files and time nothing, so they run after the mutex.
python3 -B "${HERE}/summarize-profile.py" "${OUT}" --json "${OUT}/profile-summary.json" \
    --markdown "${OUT}/profile.md" \
    || echo "summary failed; the session data is complete and the summary can be rerun" >&2
echo "profile written to ${OUT}" >&2
