#!/usr/bin/env bash
# Non-timed harness smoke of every arm and cell of the frozen dense-product
# family (jit:4c1e441f), writing dev/active/4c1e441f/survey/runner-smoke.txt.
#
# Usage (from the worker worktree root):
#   dev/active/4c1e441f/survey/smoke-arms.sh
#
# The three untimed steps are `dev/scripts/smoke-campaign-arms.sh`, shared with
# every other lane-comparison family; this script carries this family's own
# constants: its frozen addendum, its arm workspace, its binaries, its throwaway
# campaign identity and seed, and the session cell budget and pilot pair count
# the queued campaign uses.
set -euo pipefail

exec dev/scripts/smoke-campaign-arms.sh \
    --issue 4c1e441f \
    --addendum dev/active/4c1e441f/addendum-v4-dense-product-pilot.json \
    --arm-manifest dev/active/4c1e441f/survey/gemm-arm/Cargo.toml \
    --arm-bin gf256-gemm-arm \
    --smoke-bin gf256-gemm-smoke \
    --plan-tool dev/active/4c1e441f/survey/make-plan.py \
    --producing dev/active/4c1e441f/survey/producing-inputs.json \
    --record dev/active/4c1e441f/survey/runner-smoke.txt \
    --campaign-id 4c1e441f-dense-product-arms-smoke \
    --seed 20260918 \
    --max-cells 8 \
    --pilot-pairs 12
