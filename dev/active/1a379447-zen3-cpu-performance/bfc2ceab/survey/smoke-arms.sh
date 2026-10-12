#!/usr/bin/env bash
# Non-timed harness smoke of both arms and every cell of the frozen entry-check
# cost family (jit:bfc2ceab) through the shared `smoke-campaign-arms.sh`,
# writing <stage>-smoke.json beside this script. The candidate arm is a second
# build of the arm package under `--cfg gf2_entry_checked`, which this script
# tests and builds first, into the sibling target directory `make-plan.py`
# names.
#
# Usage (from the worker worktree root):
#   smoke-arms.sh [pilot|confirmation]...
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT=$(git -C "${HERE}" rev-parse --show-toplevel)
[[ "$(pwd -P)" == "$(cd "${ROOT}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
SURVEY=$(realpath --relative-to="${ROOT}" "${HERE}")
FAMILY=$(dirname "${SURVEY}")
ISSUE=$(basename "${FAMILY}")
# The shared locator, found once, names the shared smoke script.
LOCATOR=$(git ls-files --cached --others --exclude-standard -- \
    ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')
SHARED=$(python3 -B "${LOCATOR}" file smoke-campaign-arms.sh)

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
MANIFEST="${SURVEY}/entry-check-arm/Cargo.toml"
CHECKED_TARGET="${ROOT}/target/${ISSUE}-arm-checked"
CARGO_TARGET_DIR="${CHECKED_TARGET}" RUSTFLAGS='--cfg gf2_entry_checked' \
    ./scripts/cargo-budget.sh --test cargo test --locked --manifest-path "${MANIFEST}"
CARGO_TARGET_DIR="${CHECKED_TARGET}" RUSTFLAGS='--cfg gf2_entry_checked' \
    ./scripts/cargo-budget.sh cargo build --release --locked --manifest-path "${MANIFEST}"

[[ $# -gt 0 ]] || set -- pilot
for STAGE in "$@"; do
    case "${STAGE}" in
        pilot) SEED=20261005 MAX_CELLS=3 PILOT_PAIRS=(--pilot-pairs 12) ;;
        confirmation) SEED=20261006 MAX_CELLS=3 PILOT_PAIRS=() ;;
        *) echo "stage must be pilot or confirmation" >&2; exit 2 ;;
    esac
    "${SHARED}" \
        --issue "${ISSUE}" \
        --addendum "${FAMILY}/addendum-v4-entry-check-${STAGE}.json" \
        --arm-manifest "${MANIFEST}" \
        --arm-bin entry-check-arm \
        --plan-tool "${SURVEY}/make-plan.py" \
        --producing "${SURVEY}/producing-inputs.json" \
        --record "${SURVEY}/${STAGE}-smoke.json" \
        --campaign-id "${ISSUE}-entry-check-${STAGE}-arms-smoke" \
        --seed "${SEED}" \
        --max-cells "${MAX_CELLS}" \
        "${PILOT_PAIRS[@]}"
done
