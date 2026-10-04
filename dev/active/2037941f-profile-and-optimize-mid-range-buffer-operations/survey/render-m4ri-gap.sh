#!/usr/bin/env bash
# Render the M4RI comparator's interval tables and resolution record
# (jit:50f0bd42).
#
# The tables come from the canonical generator, `survey-analysis tables`, and
# the resolution record applies the frozen rule through `survey-analysis
# resolution`. This script only names the family's campaigns; `repo_artifacts.py`
# locates their receipts, the generator and the output directory at run time.
# It reads committed bytes and times nothing.
#
# Usage: render-m4ri-gap.sh, from the repository root
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

locate() { python3 -B "${HERE}/repo_artifacts.py" "$@"; }
PILOT=v4-r1-2037941f-dense-matvec-vs-m4ri
RESULTS="$(dirname "$(locate ledger "${PILOT}")")"
ANALYSIS="$(locate containing Cargo.toml 'name = "byte-field-analysis"')"
TARGET="${REPO}/target/18a87159-analysis"

pilot="$(locate receipt "${PILOT}")"
confirmation="$(locate receipt "v4-r1-confirmation-${PILOT#v4-r1-}")"

CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${ANALYSIS}"
"${TARGET}/release/survey-analysis" tables "${RESULTS}/m4ri-gap-tables.md" \
    "${pilot}" "${confirmation}"
python3 -B "${HERE}/render-m4ri-resolution.py" "${TARGET}/release/survey-analysis" \
    "${RESULTS}/m4ri-gap-resolution.md"
