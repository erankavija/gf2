#!/usr/bin/env bash
# Freeze the M4RI comparator confirmation addendum (jit:50f0bd42).
#
# Usage: freeze-m4ri-confirmation.sh <frozen-utc>, from the repository root
#
# The freezer is the canonical `freeze-confirmation.py`, the one that takes a
# rounding step: it pins the pilot receipt by path and SHA-256 and writes the
# derivation record beside the addendum. `repo_artifacts.py` locates it, the
# pilot receipt and the pilot addendum at run time. The resolution and the confirmatory cells are read from the
# generated resolution record, which applies the frozen family rule, and
# `dense-campaign verify-confirmation` holds the result to the frozen addendum.
# This script holds the remaining arguments and writes no number of its own.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
FROZEN="${1:?usage: freeze-m4ri-confirmation.sh <YYYY-MM-DDTHH:MM:SSZ>}"

locate() { python3 -B "${HERE}/repo_artifacts.py" "$@"; }
FAMILY=2037941f-dense-matvec-vs-m4ri
PILOT="v4-r1-${FAMILY}"
PILOT_ADDENDUM="$(locate addendum "${PILOT}")"
RECORD="$(dirname "$(locate ledger "${PILOT}")")/m4ri-gap-resolution.md"
OUTPUT="${PILOT_ADDENDUM%.json}-confirmation.json"

"${HERE}/render-m4ri-gap.sh"
mapfile -t rule < <(python3 - "${RECORD}" <<'PY'
import re, sys
text = open(sys.argv[1]).read()
print(re.search(r"\*\*Outcome: `([a-z-]+)`", text).group(1))
print(re.search(r"\$r=([0-9.]+)\$", text).group(1))
print("\n".join(re.findall(r"^\| `(m4ri-gap-[^`]+)` \|", text, re.M)))
PY
)
[[ "${rule[0]}" == eligible ]] || {
    echo "${RECORD} records ${rule[0]}; the family runs no confirmation" >&2
    exit 2
}
cells=()
for cell in "${rule[@]:2}"; do cells+=(--cell "${cell}"); done

python3 -B "$(locate containing freeze-confirmation.py --resolution-decimals)" \
    --pilot-addendum "${PILOT_ADDENDUM}" \
    --pilot "$(locate receipt "${PILOT}")" \
    --frozen-utc "${FROZEN}" \
    --output "${OUTPUT}" \
    --record "${OUTPUT%.json}-derivation.txt" \
    --resolution "${rule[1]}" --resolution-derivation "${RECORD}" --resolution-decimals 3 \
    "${cells[@]}" \
    --selection-rationale "The frozen addendum (M4RI comparator) names exactly these confirmatory cells: the qualified fresh whole-consumer shapes in the story's mid-range band. The remaining qualified shapes and the retained-state cells stay exploratory pilot evidence, and a retained-state cell never replaces a fresh whole-consumer cell. The resolution derivation in this record recomputes P-20's tail support for the retained comparisons from receipt.rs and trial_ledger.rs." \
    --family-description "Confirmatory stage of the whole-consumer comparison of public BitMatrix::matvec with the qualified external M4RI mzd_mul. Every cell is a fresh whole-consumer mid-range shape, both arms charging construction, conversion, execution and output, measured on fresh pairs at the resolution the committed pilot fixes. The family's only objective is the comparator gap: an outcome attributes an external difference and selects no production route."

target/e1f9a78f-arms/release/dense-campaign verify-confirmation \
    --family "${FAMILY}" --addendum "${OUTPUT}"
