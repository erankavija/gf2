#!/usr/bin/env bash
# Freeze the confirmation of one M4RI comparator family of this issue
# (jit:63bad95d).
#
# Usage: freeze-m4ri-confirmation.sh <level> <frozen-utc>, from the worktree root
#
# <level> is the build level in the family identity
# `63bad95d-<level>-matvec-vs-m4ri`; the family's prose addendum is
# `<level>-m4ri-addendum.md` beside this directory and its margin row is
# `<level> comparator`. The mid-range story's resolution renderer applies the
# frozen rule to the committed pilot and writes `<level>-m4ri-pilot-resolution.md`;
# the canonical freezer pins that resolution with the pilot receipt, and
# `dense-campaign verify-confirmation` holds the result to the frozen addendum.
# A `resolution-insufficient` record ends the family: no addendum is written.
# This script writes no number of its own.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
LEVEL="${1:?usage: freeze-m4ri-confirmation.sh <level> <YYYY-MM-DDTHH:MM:SSZ>}"
FROZEN="${2:?usage: freeze-m4ri-confirmation.sh <level> <YYYY-MM-DDTHH:MM:SSZ>}"

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

STORY="$(dirname "$(git -C "${REPO}" ls-files --full-name -- ':(glob)**/render-m4ri-resolution.py' |
    grep -v '/inputs/')")"
locate() { python3 -B "${REPO}/${STORY}/repo_artifacts.py" "$@"; }
FAMILY="63bad95d-${LEVEL}-matvec-vs-m4ri"
PILOT="v4-r1-${FAMILY}"
PILOT_ADDENDUM="$(locate addendum "${PILOT}")"
ISSUE_DIR="$(dirname "${HERE#"${REPO}/"}")"
RECORD="${ISSUE_DIR}/${LEVEL}-m4ri-pilot-resolution.md"
OUTPUT="${PILOT_ADDENDUM%.json}-confirmation.json"
ANALYSIS="$(locate containing Cargo.toml 'name = "byte-field-analysis"')"
TARGET="${REPO}/target/18a87159-analysis"

CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${ANALYSIS}"
python3 -B "${REPO}/${STORY}/render-m4ri-resolution.py" "${TARGET}/release/survey-analysis" \
    "${RECORD}" "${PILOT}" "${LEVEL}-m4ri-addendum.md" "${LEVEL} comparator"
mapfile -t rule < <(python3 - "${RECORD}" <<'PY'
import re, sys
text = open(sys.argv[1]).read()
print(re.search(r"\*\*Outcome: `([a-z-]+)`", text).group(1))
print(re.search(r"\$r=([0-9.]+)\$", text).group(1))
PY
)
[[ "${rule[0]}" == eligible ]] || {
    echo "${RECORD} records ${rule[0]}; the family runs no confirmation" >&2
    exit 2
}

python3 -B "$(locate containing freeze-confirmation.py --resolution-decimals)" \
    --pilot-addendum "${PILOT_ADDENDUM}" \
    --pilot "$(locate receipt "${PILOT}")" \
    --frozen-utc "${FROZEN}" \
    --output "${OUTPUT}" \
    --record "${OUTPUT%.json}-derivation.txt" \
    --resolution "${rule[1]}" --resolution-derivation "${RECORD}" --resolution-decimals 3 \
    --family-description "Confirmatory stage of the whole-consumer comparison of public BitMatrix::matvec from the ${LEVEL} gf2 build with the external M4RI mzd_mul built at the equivalent targeting level. Every pilot cell is confirmatory: the fresh whole-consumer and retained-state mid-range shapes and the small inputs, both arms charging construction, conversion, execution and output, measured on fresh pairs at the resolution the committed pilot fixes. The family's only objective is the comparator gap: an outcome attributes an external difference and selects no production route."

"${REPO}/target/e1f9a78f-arms/release/dense-campaign" verify-confirmation \
    --family "${FAMILY}" --addendum "${OUTPUT}"
