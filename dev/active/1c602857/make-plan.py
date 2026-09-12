#!/usr/bin/env python3
"""Write the runner plan for one public-clmul campaign (jit:1c602857).

Usage: make-plan.py <plan.json> <campaign-id> <label> <addendum> <seed> <arm> <lock>

The plan names the addendum's six cells, both arms and the producing-input
manifest. Both arms are the same executable: they differ only in
`GF2_CLMUL_PATH`, which selects the entry point the arm times, so a measured
ratio attributes to that entry point rather than to two builds.
"""

import json
import sys

# A logical call is this many products. Per-call harness cost is then a small
# fraction of a measured window at every width, and both arms pay it alike.
INNER = 64


def arm(executable, path, description):
    return {
        # No target-cpu flag reaches this build, so it is the portable one a
        # consumer gets by default. Reaching the PCLMULQDQ kernels is a runtime
        # capability decision, which is exactly what the candidate arm times.
        "build": "conservative-portable",
        "description": description,
        "executable": executable,
        "arguments": [],
        "environment": {"GF2_CLMUL_PATH": path},
        "rustflags": None,
        "tuning_profile": None,
    }


def owned(words, seed):
    return {"kind": "clmul-wide", "words": words, "inner": INNER, "seed": seed}


def accumulate(words, seed):
    return {
        "kind": "clmul-wide-accumulate",
        "words": words,
        "inner": INNER,
        "seed": seed,
    }


def cell(cell_id, case, pilot_pairs):
    return {
        "cell_id": cell_id,
        "baseline_arm": "portable-path",
        "candidate_arm": "public-path",
        "case": case,
        "pilot_pairs": pilot_pairs,
    }


def main():
    plan_path, campaign, label, addendum, seed, executable, lock = sys.argv[1:8]
    pilot_pairs = 6 if label == "pilot" else None
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": "1c602857",
        "label": label,
        "campaign_seed": int(seed),
        "addendum": addendum,
        "producing_manifest": "dev/active/1c602857/producing-inputs.json",
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {
            "portable-path": arm(
                executable,
                "portable",
                "gf2 portable build timing clmul_wide_slice_portable: the "
                "bit-by-bit schoolbook that reaches no capability dispatch, "
                "which is the public path before this issue routed it.",
            ),
            "public-path": arm(
                executable,
                "public",
                "gf2 portable build timing the public clmul_wide and "
                "clmul_wide_slice long-product API, routed through the "
                "capability dispatch this issue gives it.",
            ),
        },
        "cells": [
            cell("clmul-wide-4w-owned", owned(4, 1001), pilot_pairs),
            cell("clmul-wide-9w-owned", owned(9, 1002), pilot_pairs),
            cell("clmul-wide-4w-accumulate", accumulate(4, 1003), pilot_pairs),
            cell("clmul-wide-1w-dispatch-overhead", owned(1, 1004), pilot_pairs),
            cell("clmul-wide-2w-dispatch-overhead", owned(2, 1005), pilot_pairs),
            cell("clmul-wide-16w-dispatch-overhead", owned(16, 1006), pilot_pairs),
        ],
        # Three bounded sessions: the exclusive mutex is released twice
        # mid-campaign so sibling workers are not starved, and the runner
        # resumes from its checkpoints.
        "max_cells_per_session": 2,
    }
    with open(plan_path, "w") as handle:
        json.dump(plan, handle, indent=2)
        handle.write("\n")
    print(f"plan -> {plan_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
