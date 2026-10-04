#!/usr/bin/env bash
# Non-timed harness smoke of every arm and cell of the frozen dense-product
# family (jit:4c1e441f), writing dev/active/4c1e441f/survey/runner-smoke.txt.
#
# Usage (from the worker worktree root):
#   dev/active/4c1e441f/survey/smoke-arms.sh [pilot|confirmation]
#
# The argument names the stage whose frozen addendum the smoke drives and
# defaults to the confirmation. The record the launcher's `window` stage reads
# is the same file either way, so the committed record describes the stage
# smoked last.
#
# The three untimed steps are `dev/scripts/smoke-campaign-arms.sh`, shared with
# every other lane-comparison family; this script carries this family's own
# constants: its frozen addendum, its arm workspace, its binaries, its throwaway
# campaign identity and seed, and each stage's session cell budget and pilot
# pair count.
set -euo pipefail

STAGE=${1:-confirmation}
case "${STAGE}" in
    pilot)
        ADDENDUM=dev/active/4c1e441f/addendum-v4-dense-product-pilot.json
        # The pilot's throwaway identity and seed are the ones its committed
        # record names, so that record stays reproducible byte for byte.
        CAMPAIGN_ID=4c1e441f-dense-product-arms-smoke
        SEED=20260918 MAX_CELLS=8 PILOT_PAIRS=(--pilot-pairs 12) ;;
    confirmation)
        ADDENDUM=dev/active/4c1e441f/addendum-v4-dense-product-confirmation.json
        CAMPAIGN_ID=4c1e441f-dense-product-arms-smoke-confirmation
        SEED=20260919 MAX_CELLS=3 PILOT_PAIRS=() ;;
    *) echo "stage must be pilot or confirmation" >&2; exit 2 ;;
esac

exec dev/scripts/smoke-campaign-arms.sh \
    --issue 4c1e441f \
    --addendum "${ADDENDUM}" \
    --arm-manifest dev/active/4c1e441f/survey/gemm-arm/Cargo.toml \
    --arm-bin gf256-gemm-arm \
    --smoke-bin gf256-gemm-smoke \
    --plan-tool dev/active/4c1e441f/survey/make-plan.py \
    --producing dev/active/4c1e441f/survey/producing-inputs.json \
    --record dev/active/4c1e441f/survey/runner-smoke.txt \
    --campaign-id "${CAMPAIGN_ID}" \
    --seed "${SEED}" \
    --max-cells "${MAX_CELLS}" \
    "${PILOT_PAIRS[@]}"
