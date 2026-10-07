#!/usr/bin/env bash
# Non-timed harness smoke of both arms and every cell of the frozen entry-check
# cost family (jit:bfc2ceab), writing <stage>-smoke.json beside this script.
#
# Usage (from the worker worktree root):
#   smoke-arms.sh [pilot|confirmation]...
#
# The three untimed steps are the shared `smoke-campaign-arms.sh`, whose record
# is the one `benchmark-ab-runner smoke` writes. That script builds the arm
# once; this family's candidate arm is a second build of the same package
# under `--cfg gf2_entry_checked`, so this script tests and builds it first,
# into the sibling target directory `make-plan.py` names.
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
mapfile -t SHARED < <(git -C "${ROOT}" ls-files -- ':(glob)**/smoke-campaign-arms.sh' |
    grep -v '/inputs/')
[[ ${#SHARED[@]} -eq 1 ]] || {
    echo "${#SHARED[@]} live smoke-campaign-arms.sh files; exactly one must exist" >&2
    exit 2
}

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
    "${SHARED[0]}" \
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
