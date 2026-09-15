#!/usr/bin/env python3
"""Write the runner plan for one 53c5a8c0 campaign.

Usage: make-plan.py <plan.json> <campaign-id> <family> <label> <addendum>
                    <seed> <arm-directory> <lock> [holdout-declaration]

The plan's cells come from the same `cells.py` grid the addendum generator
reads, and the holdout cells come from the committed holdout declaration, so no
cell, size, seed or arm assignment is written twice. Both arms of a crossover
cell are the same executable and differ only in `GF2_CROSSOVER_PATH`, so a
measured ratio attributes to the entry point rather than to two builds; the
polynomial family's two arms are two executables, one of which links the pinned
gf2x build.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cells as grid  # noqa: E402


def arm(executable, build, description, environment=None):
    return {
        "build": build,
        "description": description,
        "executable": executable,
        "arguments": [],
        "environment": environment or {},
        "rustflags": None,
        "tuning_profile": None,
    }


def arms_for(family, arm_dir):
    if family == grid.CROSSOVER_FAMILY:
        executable = os.path.join(arm_dir, "crossover-arm")
        return {
            grid.ELEMENT_ARM: arm(
                executable,
                "conservative-portable",
                "gf2 portable build timing the per-element path: the bundle's "
                "single carry-less multiply in a loop, FieldVec::dot_product, "
                "and Gf2mElement multiplication one pair at a time.",
                {"GF2_CROSSOVER_PATH": "element"},
            ),
            grid.BATCH_ARM: arm(
                executable,
                "conservative-portable",
                "gf2 portable build timing the batched path: the bundle's "
                "raw-batch kernel, FieldVec::simd_dot_product, and "
                "gf2m::batch::batch_mul.",
                {"GF2_CROSSOVER_PATH": "batch"},
            ),
        }
    return {
        grid.GF2_ARM: arm(
            os.path.join(arm_dir, "poly-arm"),
            "native",
            "gf2 host-targeted build timing the public clmul_wide_slice long "
            "product and the whole-consumer Gf2mWide::mul_ref field product.",
        ),
        grid.GF2X_ARM: arm(
            os.path.join(arm_dir, "gf2x-poly-arm"),
            "external",
            "the pinned gf2x 1.3.0 host-targeted build timing gf2x_mul_r, "
            "composed with gf2's own BarrettReducerWide where the cell is a "
            "field product.",
        ),
    }


def plan_cell(cell, pilot_pairs):
    return {
        "cell_id": cell["cell_id"],
        "baseline_arm": cell["baseline_arm"],
        "candidate_arm": cell["candidate_arm"],
        "case": cell["case"],
        "pilot_pairs": pilot_pairs,
    }


def main():
    (
        plan_path,
        campaign,
        family,
        label,
        addendum,
        seed,
        arm_dir,
        lock,
    ) = sys.argv[1:9]
    holdout_declaration = sys.argv[9] if len(sys.argv) > 9 else None
    pilot_pairs = 6 if label == "pilot" else None

    declared = list(grid.cells_of(family))
    if label != "pilot":
        # A confirmation runs exactly the cells its frozen addendum declares,
        # which is a selection of the pilot cells plus any holdout cells.
        with open(addendum) as handle:
            frozen = json.load(handle)
        wanted = [cell["cell_id"] for cell in frozen["cells"]]
        by_id = {cell["cell_id"]: cell for cell in declared}
        if holdout_declaration:
            with open(holdout_declaration) as handle:
                for cell in json.load(handle)["cells"]:
                    by_id[cell["cell_id"]] = cell
        missing = [cell_id for cell_id in wanted if cell_id not in by_id]
        if missing:
            raise SystemExit(f"the addendum declares cells the plan cannot run: {missing}")
        declared = [by_id[cell_id] for cell_id in wanted]

    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": "53c5a8c0",
        "label": label,
        "campaign_seed": int(seed),
        "addendum": addendum,
        "producing_manifest": "dev/active/53c5a8c0/survey/producing-inputs.json",
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": arms_for(family, arm_dir),
        "cells": [plan_cell(cell, pilot_pairs) for cell in declared],
        # Bounded sessions: the exclusive mutex is released mid-campaign so
        # sibling workers are not starved, and the runner resumes from its
        # checkpoints.
        "max_cells_per_session": 4,
    }
    with open(plan_path, "w") as handle:
        json.dump(plan, handle, indent=2)
        handle.write("\n")
    print(f"plan ({len(plan['cells'])} cells) -> {plan_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
