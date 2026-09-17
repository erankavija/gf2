#!/usr/bin/env python3
"""Write a resumable protocol-v4 runner plan for issue 85fc5ff4.

The same projection serves the queued profile and the throwaway wire smoke:
both name the residual and word-aligned arms of one executable and both derive
every cell from a frozen addendum, so the smoke exercises the case shape the
profile sends.
"""

import argparse
import json

ADDENDUM = (
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/"
    "shift-profile-addendum.json"
)
PRODUCING = (
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/"
    "shift-profile-producing-inputs.json"
)
SEED = 2026091585


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


def cell(declared):
    size = declared["workload"]["size"]
    return {
        "cell_id": declared["cell_id"],
        "baseline_arm": "residual-production",
        "candidate_arm": "word-aligned-control",
        "case": {
            "direction": declared["cell_id"].split("-", 1)[0],
            "length_bits": size["length_bits"],
            "residual_offset": size["residual_offset"],
            "control_offset": size["control_offset"],
            "seed": declared["workload"]["seed"],
        },
        "pilot_pairs": 6,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output")
    parser.add_argument("executable")
    parser.add_argument("lock")
    parser.add_argument("--addendum", default=ADDENDUM)
    parser.add_argument("--producing-manifest", default=PRODUCING)
    parser.add_argument("--campaign-id", required=True)
    parser.add_argument("--campaign-seed", type=int, default=SEED)
    parser.add_argument("--label", default="pilot")
    parser.add_argument("--max-cells-per-session", type=int, default=2)
    args = parser.parse_args()

    with open(args.addendum) as source:
        addendum = json.load(source)
    cells = [cell(declared) for declared in addendum["cells"]]
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id,
        "issue": "85fc5ff4",
        "label": args.label,
        "campaign_seed": args.campaign_seed,
        "addendum": args.addendum,
        "producing_manifest": args.producing_manifest,
        "lock_path": args.lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {
            "residual-production": arm(
                args.executable,
                "residual-production",
                "Current public BitVec zero-fill shift at the declared residual offset; "
                "the non-word-aligned scalar carry loop is selected.",
            ),
            "word-aligned-control": arm(
                args.executable,
                "word-aligned-control",
                "Current public BitVec zero-fill shift at offset 64; the canonical "
                "word-shift backend is a non-equivalent workload control only.",
            ),
        },
        "cells": cells,
        "max_cells_per_session": args.max_cells_per_session,
    }
    with open(args.output, "w") as destination:
        json.dump(plan, destination, indent=2)
        destination.write("\n")
    print(f"{len(cells)} cells -> {args.output}")


if __name__ == "__main__":
    main()
