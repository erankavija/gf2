#!/usr/bin/env bash
# Non-timed harness smoke of both arms and every cell of the frozen sparse
# product check-cost family (jit:bfc2ceab), writing spmm-<stage>-smoke.json
# beside this script.
#
# Usage (from the worker worktree root):
#   smoke-spmm-arms.sh [pilot|pilot2|confirmation]...
#
# `pilot2` is the pilot addendum at the second pilot trial's pair count.
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

[[ $# -gt 0 ]] || set -- pilot
for STAGE in "$@"; do
    case "${STAGE}" in
        pilot) SEED=20261007 MAX_CELLS=4 PILOT_PAIRS=(--pilot-pairs 12) ;;
        pilot2)
            SEED=20261009 MAX_CELLS=4
            PILOT_PAIRS=(--pilot-pairs "$(python3 -B -c \
                'import json,sys; print(json.load(open(sys.argv[1]))["second_pilot"]["pairs"])' \
                "${SURVEY}/spmm-second-pilot-pairs.json")") ;;
        confirmation) SEED=20261008 MAX_CELLS=4 PILOT_PAIRS=() ;;
        *) echo "stage must be pilot, pilot2 or confirmation" >&2; exit 2 ;;
    esac
    ADDENDUM_STAGE=${STAGE%2}
    # The shared script names its arm target after the issue; this family's
    # arm takes a name of its own beside the earlier family's.
    "${SHARED[0]}" \
        --issue "${ISSUE}-spmm" \
        --addendum "${FAMILY}/addendum-v4-spmm-check-${ADDENDUM_STAGE}.json" \
        --arm-manifest "${SURVEY}/spmm-check-arm/Cargo.toml" \
        --arm-bin spmm-check-arm \
        --plan-tool "${SURVEY}/make-spmm-plan.py" \
        --producing "${SURVEY}/spmm-producing-inputs.json" \
        --record "${SURVEY}/spmm-${STAGE}-smoke.json" \
        --campaign-id "${ISSUE}-spmm-check-${STAGE}-arms-smoke" \
        --seed "${SEED}" \
        --max-cells "${MAX_CELLS}" \
        "${PILOT_PAIRS[@]}"
done
