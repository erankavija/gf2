#!/usr/bin/env bash
# Mapping-analysis tools for the eda07788 survey.
#
# Usage:
#   dev/active/eda07788/survey/run-analysis.sh [staging-dir]
#
# These answer semantic questions about operations that reach no timed cell:
# whether gf2's arbitrary-offset bit shifts are zero-fill rather than
# wrap-around (so a circular-shift API cannot stand in for them), and whether
# AFF3CT's 5G NR bit selection picks the same bits as gf2's rate matching.
# They link no external comparator and need no fetched checkout, so they run
# without `fetch-build.sh` and cannot perturb the timed arms' content identity.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
EXT="${1:-${REPO}/.agents/ext/eda07788}"
MANIFEST="${REPO}/dev/active/eda07788/survey/analysis/Cargo.toml"

mkdir -p "${EXT}"
EXT="$(cd "${EXT}" && pwd)"
TARGET="${EXT}/target-analysis"

cd "${REPO}"
# The lockfile is committed, so an unreachable registry is not a failure.
CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${MANIFEST}" \
  || CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --offline --manifest-path "${MANIFEST}"

"${TARGET}/release/validate-shift-semantics"
echo
"${TARGET}/release/compare-nr-rate-matching"
