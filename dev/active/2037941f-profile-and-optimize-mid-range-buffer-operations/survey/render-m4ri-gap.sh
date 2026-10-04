#!/usr/bin/env bash
# Render the M4RI comparator's interval tables and resolution record
# (jit:50f0bd42).
#
# The tables come from the canonical generator, `survey-analysis tables`
# (dev/active/6c6b09b1/survey/analysis), and the resolution record applies the
# frozen rule through `survey-analysis resolution`. This script only names the
# family's receipts; it reads committed bytes and times nothing.
#
# Usage: dev/active/2037941f-.../survey/render-m4ri-gap.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

RESULTS=dev/bench_results/2037941f
FAMILY="${RESULTS}/2037941f-dense-matvec-vs-m4ri"
SURVEY=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey
ANALYSIS=dev/active/6c6b09b1/survey/analysis/Cargo.toml
TARGET="${REPO}/target/18a87159-analysis"

# The pilot always; the confirmation once the window has written it.
receipts=("${FAMILY}/v4-r1-pilot")
[[ -f "${FAMILY}/v4-r1-confirmation/receipt.json" ]] &&
    receipts+=("${FAMILY}/v4-r1-confirmation")

CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${ANALYSIS}"
"${TARGET}/release/survey-analysis" tables "${RESULTS}/m4ri-gap-tables.md" "${receipts[@]}"
python3 -B "${SURVEY}/render-m4ri-resolution.py" "${TARGET}/release/survey-analysis" \
    "${RESULTS}/m4ri-gap-resolution.md"
