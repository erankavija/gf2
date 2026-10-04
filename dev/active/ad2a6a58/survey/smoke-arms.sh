#!/usr/bin/env bash
# Non-timed harness smoke of every arm and cell of the frozen axpy family
# (jit:ad2a6a58), writing dev/active/ad2a6a58/survey/<stage>-smoke.json.
#
# Usage (from the worker worktree root):
#   dev/active/ad2a6a58/survey/smoke-arms.sh [pilot|confirmation]...
#
# Each argument names a stage whose frozen addendum the smoke drives; with none
# it drives both. The three untimed steps are `dev/scripts/smoke-campaign-arms.sh`,
# shared with every other lane-comparison family, whose record is the one
# `benchmark-ab-runner smoke` writes; this script carries this family's own
# constants: its frozen addenda, its arm workspace and binary, its throwaway
# campaign identities and seeds, and the session cell budget and pilot pair
# count each queued campaign uses.
set -euo pipefail

[[ $# -gt 0 ]] || set -- pilot confirmation
for STAGE in "$@"; do
    case "${STAGE}" in
        pilot) SEED=20260918 MAX_CELLS=5 PILOT_PAIRS=(--pilot-pairs 12) ;;
        confirmation) SEED=20260919 MAX_CELLS=3 PILOT_PAIRS=() ;;
        *) echo "stage must be pilot or confirmation" >&2; exit 2 ;;
    esac
    dev/scripts/smoke-campaign-arms.sh \
        --issue ad2a6a58 \
        --addendum "dev/active/ad2a6a58/addendum-v4-axpy-${STAGE}.json" \
        --arm-manifest dev/active/ad2a6a58/survey/axpy-arm/Cargo.toml \
        --arm-bin gf256-axpy-arm \
        --plan-tool dev/active/ad2a6a58/survey/make-plan.py \
        --producing dev/active/ad2a6a58/survey/producing-inputs.json \
        --record "dev/active/ad2a6a58/survey/${STAGE}-smoke.json" \
        --campaign-id "ad2a6a58-axpy-${STAGE}-arms-smoke" \
        --seed "${SEED}" \
        --max-cells "${MAX_CELLS}" \
        "${PILOT_PAIRS[@]}"
done
