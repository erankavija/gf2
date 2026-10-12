#!/usr/bin/env bash
# Render the interval tables of the native M4RI comparator family's pilots
# (jit:63bad95d) with the canonical generator, `survey-analysis tables`.
# It reads committed bytes and times nothing; `repo_artifacts.py` locates the
# receipts, the generator and the output directory at run time.
#
# Usage: render-native-m4ri-tables.sh, from the worktree root
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

STORY="$(dirname "$(git -C "${REPO}" ls-files --full-name -- ':(glob)**/repo_artifacts.py' |
    grep -v '/inputs/')")"
locate() { python3 -B "${REPO}/${STORY}/repo_artifacts.py" "$@"; }
FAMILY=63bad95d-native-matvec-vs-m4ri
RESULTS="$(dirname "$(locate ledger "v4-r1-${FAMILY}")")"
ANALYSIS="$(locate containing Cargo.toml 'name = "byte-field-analysis"')"
TARGET="${REPO}/target/18a87159-analysis"

CARGO_TARGET_DIR="${TARGET}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${ANALYSIS}"
"${TARGET}/release/survey-analysis" tables "${RESULTS}/native-m4ri-tables.md" \
    "$(locate receipt "v4-r1-${FAMILY}")" "$(locate receipt "v4-r2-${FAMILY}")"
