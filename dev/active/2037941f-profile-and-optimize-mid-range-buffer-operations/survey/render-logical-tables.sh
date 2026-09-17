#!/usr/bin/env bash
# Render the three logical baselines' interval tables (jit:18a87159).
#
# The tables come from the canonical generator, `survey-analysis tables`
# (dev/active/6c6b09b1/survey/analysis), which reads committed receipt bytes
# and re-derives every interval through the shared campaign tooling. This
# script only names this issue's receipts, so no private table variant exists.
#
# It reads receipts and times nothing; it refuses until the window has written
# them rather than rendering a partial document.
#
# Usage: dev/active/2037941f-.../survey/render-logical-tables.sh [<run-id>]
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

RUN_ID="${1:-v4-r1}"
RESULTS=dev/bench_results/2037941f
OUTPUT="${RESULTS}/logical-tables.md"
ANALYSIS=dev/active/6c6b09b1/survey/analysis/Cargo.toml
TARGET="${REPO}/target/18a87159-analysis"

receipts=()
for family in 2037941f-logical-isolated-xor 2037941f-logical-public-row-xor \
    2037941f-logical-nr-construction; do
    receipt="${RESULTS}/${family}/${RUN_ID}-pilot"
    [[ -f "${receipt}/receipt.json" ]] || {
        echo "${receipt}/receipt.json is absent; the benchmark window has not written it" >&2
        exit 2
    }
    receipts+=("${receipt}")
done

CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${ANALYSIS}"
"${TARGET}/release/survey-analysis" tables "${OUTPUT}" "${receipts[@]}"
echo "tables written to ${OUTPUT}" >&2
