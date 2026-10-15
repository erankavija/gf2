#!/usr/bin/env bash
# Re-sampled steady-state profile of the changed decoder (jit:07ca8585).
#
# The predecessor `3be770d5` ranks the levers this issue spends, and its
# refutation rule for that ranking is a re-sampled profile rather than a clock:
# a lever the change spends must lose its sampled share once the change is in.
# This series is that re-sampling. It runs the predecessor's own session script,
# case set and arm catalogue unchanged, so the case identities and the sampled
# quantities are the ones the ranking was derived from, and it measures the
# `after` generation of the harness this issue built, whose executables the
# kernel preparation build identity beside this script pins. Every session runs
# under `--full-host` because the case set includes 12- and 24-worker rows a
# six-core pin cannot serve. profile-series.sh states the execution log,
# resumption and refusals.
#
# Usage (from the worktree root):
#   dev/bench_results/07ca8585/run-profile-resample.sh <output-dir>
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${repo}" && pwd -P)" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
IDENTITY="${HERE}/preparation/kernel-build-identity.json"
FILES="$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')"
PREDECESSOR="${repo}/$(dirname "$(python3 -B "${FILES}" file profile-session.sh)")"
QUALITY="${repo}/dev/bench_results/c077a88b/v3-preparation/quality"
# Nine is the predecessor's session count, kept so the two series' medians carry
# the same order-statistic interval.
REPETITIONS=9
EXECUTABLES=(ldpc-profile ldpc-alloc-census)
SESSION_FILES=(profile-session.sh profile-cases.tsv arms.json)

OUT="${1:?usage: run-profile-resample.sh <output-dir>}"
export PATH="${HOME}/.cargo/bin:${PATH}"

# shellcheck disable=SC1091
source "${PREDECESSOR}/profile-series.sh"
recorded() { series_recorded "${IDENTITY}" "$@"; }
BIN="${repo}/$(recorded profile_resample target_dir)"
INPUTS="${repo}/$(recorded inputs extracted_to)"

series_identity() {
    local binary file expected line=""
    for binary in "${EXECUTABLES[@]}"; do
        expected="$(recorded profile_resample executables "${binary}")"
        if [[ "$(series_digest "${BIN}/${binary}")" != "${expected}" ]]; then
            echo "${binary} differs from ${IDENTITY}" >&2
            return 2
        fi
        line+="${line:+ }${binary}=${expected}"
    done
    for file in "${SESSION_FILES[@]}"; do
        line+=" ${file}=$(series_digest "${PREDECESSOR}/${file}")"
    done
    echo "${line}"
}
series_cases() {
    python3 - "${PREDECESSOR}/profile-cases.tsv" "${PREDECESSOR}/arms.json" "${INPUTS}" "${QUALITY}" <<'PY'
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
profile_series "${OUT}" "${BIN}" "${PREDECESSOR}/profile-session.sh" "${REPETITIONS}" \
    "re-sampled LDPC profile host record - produced by run-profile-resample.sh; re-samples 3be770d5 v4-r1-steady-profile"

# Summaries read files and time nothing, so they run after the mutex.
python3 -B "${PREDECESSOR}/summarize-profile.py" "${OUT}" --json "${OUT}/profile-summary.json" \
    --markdown "${OUT}/profile.md" --binary "${BIN}/ldpc-profile" \
    --binary-sha256 "$(recorded profile_resample executables ldpc-profile)" \
    || echo "summary failed; the session data is complete and the summary can be rerun" >&2
echo "re-sampled profile written to ${OUT}" >&2
