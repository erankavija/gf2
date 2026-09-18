#!/usr/bin/env bash
# Non-timed harness smoke of every arm and cell of the frozen axpy family
# (jit:ad2a6a58), writing dev/active/ad2a6a58/survey/runner-smoke.txt.
#
# Usage (from the worker worktree root):
#   dev/active/ad2a6a58/survey/smoke-arms.sh
#
# The three untimed steps are `dev/scripts/smoke-campaign-arms.sh`, shared with
# every other lane-comparison family; this script carries this family's own
# constants: its frozen addendum, its arm workspace, its binaries, its throwaway
# campaign identity and seed, and the session cell budget and pilot pair count
# the queued campaign uses.
set -euo pipefail

exec dev/scripts/smoke-campaign-arms.sh \
    --issue ad2a6a58 \
    --addendum dev/active/ad2a6a58/addendum-v4-axpy-pilot.json \
    --arm-manifest dev/active/ad2a6a58/survey/axpy-arm/Cargo.toml \
    --arm-bin gf256-axpy-arm \
    --smoke-bin gf256-axpy-smoke \
    --plan-tool dev/active/ad2a6a58/survey/make-plan.py \
    --producing dev/active/ad2a6a58/survey/producing-inputs.json \
    --record dev/active/ad2a6a58/survey/runner-smoke.txt \
    --campaign-id ad2a6a58-axpy-arms-smoke \
    --seed 20260918 \
    --max-cells 10 \
    --pilot-pairs 12
