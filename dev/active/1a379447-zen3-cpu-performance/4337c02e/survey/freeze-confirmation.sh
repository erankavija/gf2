#!/usr/bin/env bash
# Freeze the lane-threshold confirmation addendum (jit:4337c02e).
#
# Usage: freeze-confirmation.sh <frozen-utc>, from the worktree root
#
# The canonical freezer pins the pilot receipt by path and SHA-256 and writes
# the derivation record beside the addendum. The resolution comes from the
# generated resolution record, the confirmatory cells from the harness listing,
# and `--frozen-rule` names the family addendum the pilot receipt pins, which
# declares both. `dense-campaign verify-confirmation` holds the result to the
# frozen addendum. This script writes no number of its own.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
FROZEN="${1:?usage: freeze-confirmation.sh <YYYY-MM-DDTHH:MM:SSZ>}"

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

SURVEY="${HERE#"${REPO}/"}"
FAMILY=4337c02e-matvec-simd-threshold
PILOT="v4-r1-${FAMILY}"
artifacts="$(git ls-files -- ':(glob)**/repo_artifacts.py' | grep -v /inputs/)"
locate() { python3 -B "${artifacts}" "$@"; }
PILOT_ADDENDUM="$(locate addendum "${PILOT}")"
RULE="$(dirname "${SURVEY}")/matvec-lane-threshold-addendum.md"
RECORD="$(dirname "${SURVEY}")/pilot-resolution.md"
OUTPUT="${PILOT_ADDENDUM%.json}-confirmation.json"
CAMPAIGN_TOOL=target/e1f9a78f-arms/release/dense-campaign
ANALYSIS_TARGET="${REPO}/target/18a87159-analysis"

CARGO_TARGET_DIR="${ANALYSIS_TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "$(locate containing Cargo.toml 'name = "byte-field-analysis"')"
rule="$(python3 -B "${SURVEY}/make-resolution.py" \
    "${ANALYSIS_TARGET}/release/survey-analysis" "${CAMPAIGN_TOOL}")"
[[ "${rule}" =~ ^resolution=([0-9.]+)\ outcome=eligible$ ]] || {
    echo "${RECORD} records ${rule}; the family runs no confirmation" >&2
    exit 2
}
cells=()
while read -r _ cell fields; do
    [[ " ${fields} " == *' confirmatory=true '* ]] && cells+=(--cell "${cell}")
done < <("${CAMPAIGN_TOOL}" list --family "${FAMILY}")

python3 -B "$(locate containing freeze-confirmation.py --frozen-rule)" \
    --pilot-addendum "${PILOT_ADDENDUM}" \
    --pilot "$(locate receipt "${PILOT}")" \
    --frozen-utc "${FROZEN}" \
    --output "${OUTPUT}" \
    --record "${OUTPUT%.json}-derivation.txt" \
    --resolution "${BASH_REMATCH[1]}" --resolution-derivation "${RECORD}" --resolution-decimals 3 \
    --frozen-rule "${RULE}" \
    "${cells[@]}" \
    --selection-rationale "The frozen addendum (Cells) names exactly these confirmatory cells: the warm lane strides whose exploratory rows favour the scalar lane, their neighbours and the interior stride. The control, the further neighbours and the streaming and cold cells stay exploratory pilot evidence. The resolution derivation in this record recomputes P-20's tail support for the retained comparisons from receipt.rs and trial_ledger.rs." \
    --family-description "Confirmatory stage of the lane contrast of the allocated BitMatrix::matvec: the scalar four-accumulator lane, pinned through matvec_with_route, as candidate against the public call on its selected lane in the same build. Every cell is a warm whole-consumer cell at a frozen lane stride, including the output allocation and appends, measured on fresh pairs at the resolution the committed pilot fixes. The frozen resolution rule reads the outcomes and yields one threshold value or no change; the family changes no selector."

"${CAMPAIGN_TOOL}" verify-confirmation --family "${FAMILY}" --addendum "${OUTPUT}"
