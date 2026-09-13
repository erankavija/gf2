#!/usr/bin/env python3
"""Write the runner plan for one count-optimization campaign (jit:5cbb6545).

Usage: make-plan.py <plan.json> <campaign-id> <label> <addendum> <seed> <arm> <lock>

The plan names the addendum's cells, the arms those cells reference and the
producing-input manifest. Every arm is the same executable and differs only in
`GF2_COUNT_ARM`, which selects the route the arm times, so a measured ratio
attributes to that route rather than to two builds. The cell table comes from
`families.py`, the same module the addendum generator reads; the plan refuses
to run if the addendum it is paired with names different cells.
"""

import json
import sys

import families

STAGES = {
    "popcount-sweep": families.popcount_sweep,
    "fused-sweep": families.fused_sweep,
    "popcount-sweep2": families.popcount_sweep2,
    "fused-sweep2": families.fused_sweep2,
    "popcount-pilot": families.popcount_pilot,
    "fused-pilot": families.fused_pilot,
    "smoke-smoke": families.smoke,
}

ARM_DESCRIPTIONS = {
    "legacy-dispatch": (
        "gf2 portable build timing the route ops::popcount took before this "
        "issue: the scalar backend below eight words, the AVX2 nibble-lookup "
        "kernel of the detected bundle at or above."
    ),
    "resolved-dispatch": (
        "gf2 portable build timing ops::popcount as this issue leaves it: one "
        "route resolved for the buffer's width from the compile-time "
        "bit-backend boundaries."
    ),
    "nibble-lut": (
        "The detected bundle's avx2_popcnt called directly: one VPSHUFB "
        "nibble lookup per vector summed by VPSADBW, with no size threshold."
    ),
    "scalar-popcnt": (
        "The detected bundle's popcnt_words called directly: one POPCNT "
        "instruction per word, with no vector state."
    ),
    "csa": (
        "The detected bundle's avx2_popcnt_csa called directly: sixteen "
        "vectors folded through fifteen carry-save adders per 512-byte block "
        "before one nibble lookup."
    ),
    "compiler-count-ones": (
        "internal control: the portable u64::count_ones loop as this "
        "toolchain lowers it."
    ),
    "libpopcnt": (
        "libpopcnt v4.2 popcnt() compiled -O3 as its README recommends, with "
        "its own CPUID dispatch."
    ),
    "mula-avx2-harley-seal": (
        "Mula's popcnt_AVX2_harley_seal compiled with the sse-popcount "
        "Makefile's AVX2 flags; it loads through const __m256i* and therefore "
        "takes only vector-aligned windows."
    ),
    "and-legacy-fused": (
        "The detected bundle's avx2_and_popcnt called directly, as "
        "BitMatrix::matvec reached it before this issue."
    ),
    "and-resolved-fused": (
        "gf2 portable build timing ops::and_popcount as this issue leaves it: "
        "one fused route resolved for the width."
    ),
    "and-csa-fused": (
        "The detected bundle's avx2_and_popcnt_csa called directly: the same "
        "carry-save block with each bit-plane ANDed from the two operands as "
        "it is loaded."
    ),
    "and-scalar-control": (
        "internal control: a single-pass portable (a & b).count_ones() loop."
    ),
    "and-two-pass": (
        "The whole route a gf2-core consumer has without a fused kernel: a "
        "temporary copy of the left operand, ops::and_inplace and "
        "ops::popcount, all inside every timed call."
    ),
    "matvec-legacy": (
        "The row loop BitMatrix::matvec ran before this issue: the bundle's "
        "fused nibble-lookup kernel fetched per product and called per row, "
        "into a freshly allocated output vector."
    ),
    "matvec-resolved": (
        "BitMatrix::matvec as this issue leaves it: one fused route resolved "
        "for the stride, called per row, into a freshly allocated output "
        "vector."
    ),
}

EXTERNAL_ARMS = ("libpopcnt", "mula-avx2-harley-seal")


def arm(executable, name):
    return {
        # No target-cpu flag reaches this build, so it is the portable one a
        # consumer gets by default; gf2 reaches AVX2 through runtime
        # detection, which is what the dispatched arms time.
        "build": "external" if name in EXTERNAL_ARMS else "conservative-portable",
        "description": ARM_DESCRIPTIONS[name],
        "executable": executable,
        "arguments": [],
        "environment": {"GF2_COUNT_ARM": name},
        "rustflags": None,
        "tuning_profile": None,
    }


def main():
    plan_path, campaign, label, addendum, seed, executable, lock = sys.argv[1:8]
    stage = addendum.rsplit("/", 1)[-1].removeprefix("addendum-").removesuffix(".json")
    key = stage.replace("-v4-", "-")
    if key not in STAGES:
        raise SystemExit(f"no cell table for addendum stage {key!r}")
    cells = STAGES[key]()
    declared = json.load(open(addendum))
    declared_ids = [cell["cell_id"] for cell in declared["cells"]]
    if declared_ids != [cell["cell_id"] for cell in cells]:
        raise SystemExit(f"{addendum} names different cells than the {key} table")
    pilot_pairs = 6 if label == "pilot" else None
    arms = {}
    for cell in cells:
        for name in (cell["baseline_arm"], cell["candidate_arm"]):
            arms.setdefault(name, arm(executable, name))
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": "5cbb6545",
        "label": label,
        "campaign_seed": int(seed),
        "addendum": addendum,
        "producing_manifest": "dev/active/5cbb6545/producing-inputs.json",
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": arms,
        "cells": [
            {
                "cell_id": cell["cell_id"],
                "baseline_arm": cell["baseline_arm"],
                "candidate_arm": cell["candidate_arm"],
                "case": cell["case"],
                "pilot_pairs": pilot_pairs,
            }
            for cell in cells
        ],
        # Bounded sessions: the exclusive mutex is released between blocks so
        # sibling workers are not starved, and the runner resumes from its
        # checkpoints.
        "max_cells_per_session": 8,
    }
    with open(plan_path, "w") as handle:
        json.dump(plan, handle, indent=2)
        handle.write("\n")
    print(f"{len(plan['cells'])} cells, {len(arms)} arms -> {plan_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
