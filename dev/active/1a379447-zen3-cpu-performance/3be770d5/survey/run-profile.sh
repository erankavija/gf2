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
# preparation build identity records and refuses a session when their digests
# differ. profile-series.sh states the execution log and resumption.
#
# Usage (from the worktree root):
#   dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/run-profile.sh <output-dir>
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
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
export PATH="${HOME}/.cargo/bin:${PATH}"

digest() { sha256sum "$1" | cut -d' ' -f1; }
series_identity() {
    local binary file recorded line=""
    for binary in "${EXECUTABLES[@]}"; do
        recorded="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executables"][sys.argv[2]])' \
            "${IDENTITY}" "${binary}")"
        if [[ "$(digest "${BIN}/${binary}")" != "${recorded}" ]]; then
            echo "${binary} differs from the preparation build identity" >&2
            return 2
        fi
        line+="${line:+ }${binary}=${recorded}"
    done
    for file in "${SESSION_FILES[@]}"; do
        line+=" ${file}=$(digest "${HERE}/${file}")"
    done
    echo "${line}"
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
# shellcheck disable=SC1091
source "${HERE}/profile-series.sh"
profile_series "${OUT}" "${BIN}" "${HERE}/profile-session.sh" "${REPETITIONS}" \
    "steady-state LDPC profile host record - produced by run-profile.sh"

# Summaries read files and time nothing, so they run after the mutex.
python3 -B "${HERE}/summarize-profile.py" "${OUT}" --json "${OUT}/profile-summary.json" \
    --markdown "${OUT}/profile.md" \
    || echo "summary failed; the session data is complete and the summary can be rerun" >&2
echo "profile written to ${OUT}" >&2
