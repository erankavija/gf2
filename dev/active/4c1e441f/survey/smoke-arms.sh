#!/usr/bin/env bash
# Non-timed harness smoke of every arm and cell of the frozen dense-product
# family (jit:4c1e441f), writing <stage>-smoke.json beside this script.
#
# Usage (from the worker worktree root):
#   smoke-arms.sh [pilot|confirmation]...
#
# Each argument names a stage whose frozen addendum the smoke drives; with none
# it drives both. The three untimed steps are the shared
# `smoke-campaign-arms.sh`, whose record is the one `benchmark-ab-runner smoke`
# writes; this script carries this family's own constants: its frozen addenda,
# its arm workspace and binary, its throwaway campaign identities and seeds, and
# each stage's session cell budget and pilot pair count.
#
# Every path is repository-relative, as the plan and the record name them: this
# family's files from this script's directory, the shared script by file name
# among the files git lists outside receipt input snapshots.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT=$(git -C "${HERE}" rev-parse --show-toplevel)
SURVEY=$(realpath --relative-to="${ROOT}" "${HERE}")
FAMILY=$(dirname "${SURVEY}")
mapfile -t SHARED < <(git -C "${ROOT}" ls-files -- ':(glob)**/smoke-campaign-arms.sh' |
    grep -v '/inputs/')
[[ ${#SHARED[@]} -eq 1 ]] || {
    echo "${#SHARED[@]} live smoke-campaign-arms.sh files; exactly one must exist" >&2
    exit 2
}

[[ $# -gt 0 ]] || set -- pilot confirmation
for STAGE in "$@"; do
    case "${STAGE}" in
        pilot) SEED=20260918 MAX_CELLS=4 PILOT_PAIRS=(--pilot-pairs 12) ;;
        confirmation) SEED=20260919 MAX_CELLS=3 PILOT_PAIRS=() ;;
        *) echo "stage must be pilot or confirmation" >&2; exit 2 ;;
    esac
    "${SHARED[0]}" \
        --issue 4c1e441f \
        --addendum "${FAMILY}/addendum-v4-dense-product-${STAGE}.json" \
        --arm-manifest "${SURVEY}/gemm-arm/Cargo.toml" \
        --arm-bin gf256-gemm-arm \
        --plan-tool "${SURVEY}/make-plan.py" \
        --producing "${SURVEY}/producing-inputs.json" \
        --record "${SURVEY}/${STAGE}-smoke.json" \
        --campaign-id "4c1e441f-dense-product-${STAGE}-arms-smoke" \
        --seed "${SEED}" \
        --max-cells "${MAX_CELLS}" \
        "${PILOT_PAIRS[@]}"
done
