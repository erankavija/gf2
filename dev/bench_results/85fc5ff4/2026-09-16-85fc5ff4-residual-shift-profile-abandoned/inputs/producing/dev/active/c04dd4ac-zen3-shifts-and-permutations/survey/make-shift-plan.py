#!/usr/bin/env python3
"""Write the resumable protocol-v4 runner plan for issue 85fc5ff4."""

import json
import sys

ADDENDUM = (
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/"
    "shift-profile-addendum.json"
)
PRODUCING = (
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/"
    "shift-profile-producing-inputs.json"
)


def arm(executable, name, description):
    return {
        "build": "conservative-portable",
        "description": description,
        "executable": executable,
        "arguments": [],
        "environment": {"GF2_SHIFT_ARM": name},
        "rustflags": None,
        "tuning_profile": None,
    }


def main():
    if len(sys.argv) != 4:
        raise SystemExit("usage: make-shift-plan.py <plan> <arm-executable> <absolute-lock>")
    output, executable, lock = sys.argv[1:]
    with open(ADDENDUM) as source:
        addendum = json.load(source)
    cells = []
    for declared in addendum["cells"]:
        size = declared["workload"]["size"]
        direction = declared["cell_id"].split("-", 1)[0]
        cells.append(
            {
                "cell_id": declared["cell_id"],
                "baseline_arm": "residual-production",
                "candidate_arm": "word-aligned-control",
                "case": {
                    "direction": direction,
                    "length_bits": size["length_bits"],
                    "residual_offset": size["residual_offset"],
                    "control_offset": size["control_offset"],
                    "seed": declared["workload"]["seed"],
                },
                "pilot_pairs": 6,
            }
        )
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": "residual-shift-profile-85fc5ff4-v4",
        "issue": "85fc5ff4",
        "label": "pilot",
        "campaign_seed": 2026091585,
        "addendum": ADDENDUM,
        "producing_manifest": PRODUCING,
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {
            "residual-production": arm(
                executable,
                "residual-production",
                "Current public BitVec zero-fill shift at the declared residual offset; "
                "the non-word-aligned scalar carry loop is selected.",
            ),
            "word-aligned-control": arm(
                executable,
                "word-aligned-control",
                "Current public BitVec zero-fill shift at offset 64; the canonical "
                "word-shift backend is a non-equivalent workload control only.",
            ),
        },
        "cells": cells,
        "max_cells_per_session": 2,
    }
    with open(output, "w") as destination:
        json.dump(plan, destination, indent=2)
        destination.write("\n")
    print(f"{len(cells)} cells -> {output}")


if __name__ == "__main__":
    main()
