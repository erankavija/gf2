#!/usr/bin/env bash
# Non-timed harness smoke of every arm and cell of the frozen codeword-assembly
# family (jit:8e53235b), writing <stage>-plan-smoke.json beside this script.
#
# Usage (from the worker worktree root):
#   smoke-arms.sh [pilot|confirmation]...
#
# The three untimed steps are the shared `smoke-campaign-arms.sh`, whose record
# is the one `benchmark-ab-runner smoke` writes.
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
ARM_MANIFEST=$(python3 -B "$(dirname "${SHARED[0]}")/repository_files.py" \
    package-directory consumer-profile-gf2-side)/Cargo.toml

[[ $# -gt 0 ]] || set -- pilot
for STAGE in "$@"; do
    case "${STAGE}" in
        pilot) SEED=2026101001 MAX_CELLS=3 PILOT_PAIRS=(--pilot-pairs 24) ;;
        confirmation) SEED=2026101011 MAX_CELLS=3 PILOT_PAIRS=() ;;
        *) echo "stage must be pilot or confirmation" >&2; exit 2 ;;
    esac
    "${SHARED[0]}" \
        --issue "${ISSUE}" \
        --addendum "${FAMILY}/addendum-v4-codeword-assembly-${STAGE}.json" \
        --arm-manifest "${ARM_MANIFEST}" \
        --arm-bin consumer-arm \
        --plan-tool "${SURVEY}/make-plan.py" \
        --producing "${SURVEY}/producing-inputs.json" \
        --record "${SURVEY}/${STAGE}-plan-smoke.json" \
        --campaign-id "${ISSUE}-codeword-assembly-${STAGE}-arms-smoke" \
        --seed "${SEED}" \
        --max-cells "${MAX_CELLS}" \
        "${PILOT_PAIRS[@]}"
done
