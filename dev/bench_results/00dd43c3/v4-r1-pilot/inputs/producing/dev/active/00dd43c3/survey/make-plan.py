#!/usr/bin/env python3
"""Project a frozen residual-route addendum into a resumable runner plan."""

import argparse
import json

PILOT_ADDENDUM = "dev/active/00dd43c3/pilot-addendum.json"
PRODUCING = "dev/active/00dd43c3/survey/producing-inputs.json"


def arm(executable, mode, description):
    return {
        "build": "conservative-portable",
        "description": description,
        "executable": executable,
        "arguments": [],
        "environment": {"GF2_SHIFT_ARM": mode},
        "rustflags": None,
        "tuning_profile": None,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output")
    parser.add_argument("executable")
    parser.add_argument("lock")
    parser.add_argument("--addendum", default=PILOT_ADDENDUM)
    parser.add_argument("--producing-manifest", default=PRODUCING)
    parser.add_argument("--campaign-id", required=True)
    parser.add_argument("--label", choices=("pilot", "confirmation"), required=True)
    parser.add_argument("--campaign-seed", type=int, default=2026092600)
    args = parser.parse_args()

    with open(args.addendum) as source:
        addendum = json.load(source)
    cells = []
    for declared in addendum["cells"]:
        size = declared["workload"]["size"]
        cells.append({
            "cell_id": declared["cell_id"],
            "baseline_arm": "residual-scalar",
            "candidate_arm": "residual-gated",
            "case": {
                "direction": declared["cell_id"].split("-", 1)[0],
                "length_bits": size["length_bits"],
                "residual_offset": size["residual_offset"],
                "control_offset": size["control_offset"],
                "seed": declared["workload"]["seed"],
            },
            "pilot_pairs": None,
        })
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id,
        "issue": "00dd43c3",
        "label": args.label,
        "campaign_seed": args.campaign_seed,
        "addendum": args.addendum,
        "producing_manifest": args.producing_manifest,
        "lock_path": args.lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {
            "residual-scalar": arm(
                args.executable,
                "residual-scalar",
                "Public BitVec shift at the declared residual offset, held on the scalar funnel",
            ),
            "residual-gated": arm(
                args.executable,
                "residual-gated",
                "The same public shift and executable with the force switch clear",
            ),
        },
        "cells": cells,
        "max_cells_per_session": 2,
    }
    with open(args.output, "w") as destination:
        json.dump(plan, destination, indent=2)
        destination.write("\n")
    print(f"{len(cells)} cells -> {args.output}")


if __name__ == "__main__":
    main()
