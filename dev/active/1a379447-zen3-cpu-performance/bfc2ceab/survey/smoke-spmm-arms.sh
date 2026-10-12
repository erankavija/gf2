#!/usr/bin/env bash
# Non-timed harness smoke of both arms and every cell of the frozen sparse
# product check-cost family (jit:bfc2ceab) through the shared
# `smoke-campaign-arms.sh`, writing spmm-<stage>-smoke.json beside this script.
# `pilot2` is the pilot addendum at the second pilot trial's pair count.
#
# Usage (from the worker worktree root):
#   smoke-spmm-arms.sh [pilot|pilot2|confirmation]...
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
    # arm takes a name of its own beside the entry-check family's.
    "${SHARED}" \
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
