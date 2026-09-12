#!/usr/bin/env python3
"""Emit the `zen3-benchmark-plan-v1` runner plan of one steady-state family (jit:3be770d5).

The addendum freezes what each cell is; the plan names the executables and
bundle directories that measure it on this host. Those are absolute host
paths, so the plan is generated beside its campaign stage and the receipt
keeps its exact bytes and digest. Arms come from `arms.json`; the baseline of
every cell is the gf2 arm, and the candidate follows from the cell id.

Usage:
  make-plan.py --family ID --label pilot|confirmation --addendum PATH
      --arms-dir DIR --bundles-dir DIR --quality-dir DIR --campaign-id ID
      --output PLAN
"""

import argparse
import json
import os
import pathlib

LOCK = "/tmp/gf2-ccx1.lock"
WRAPPER = "dev/scripts/ccx1-bench-flock.sh"
# One seed per family, fixed here so a resumed campaign reproduces its pair
# orders and bootstrap draws from the plan alone.
SEEDS = {
    "ldpc-steady-matched-single-worker-v1": 0x3BE7_70D5_0001_5EED,
    "ldpc-steady-matched-multicore-v1": 0x3BE7_70D5_0002_5EED,
    "ldpc-steady-fastest-compatible-v1": 0x3BE7_70D5_0003_5EED,
}
CATALOGUE = json.loads((pathlib.Path(__file__).resolve().parent / "arms.json").read_text())
RUSTFLAGS = "-C target-cpu=native"


def candidate_of(cell_id):
    for name in ("layered-i16-inter", "layered-f32-inter", "layered-f32"):
        if f"-{name}-" in cell_id:
            return f"aff3ct-{name.replace('layered-', 'layered-nms-')}"
    if "-matched-" in cell_id:
        return "aff3ct-flooding-nms-f32"
    raise SystemExit(f"no candidate arm for cell {cell_id}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--family", required=True, choices=sorted(SEEDS))
    parser.add_argument("--label", required=True, choices=["pilot", "confirmation"])
    for name in ("--addendum", "--arms-dir", "--bundles-dir", "--quality-dir",
                 "--campaign-id", "--output"):
        parser.add_argument(name, required=True)
    parser.add_argument("--pilot-pairs", type=int, default=6)
    parser.add_argument("--max-cells-per-session", type=int, default=1)
    args = parser.parse_args()

    addendum = json.load(open(args.addendum, encoding="utf-8"))
    if addendum["family"]["id"] != args.family:
        raise SystemExit("the addendum belongs to another family")
    arms, cells = {}, []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        code_key = next(key for key in CATALOGUE["codes"] if cell_id.startswith(key + "-"))
        code = CATALOGUE["codes"][code_key]
        names = []
        for arm in ("gf2-nms-f32", candidate_of(cell_id)):
            spec = CATALOGUE["arms"][arm]
            executable = os.path.join(args.arms_dir, spec["executable"])
            if not os.path.isfile(executable):
                raise SystemExit("missing executable " + executable)
            key = f"{arm}-{code_key}"
            arms[key] = {
                "build": spec["build"],
                "description": spec["description"],
                "executable": executable,
                "arguments": [],
                "environment": dict(
                    spec["environment"],
                    GF2_LDPC_QUALITY=os.path.join(args.quality_dir, key + ".json"),
                ),
                "rustflags": RUSTFLAGS,
                "tuning_profile": None,
            }
            names.append(key)
        decoder = declared["decoder"]
        cells.append({
            "cell_id": cell_id,
            "baseline_arm": names[0],
            "candidate_arm": names[1],
            "case": {
                "bundle": os.path.join(args.bundles_dir, code["bundle"]),
                "code": code["code"],
                "iteration_cap": decoder["iteration_cap"],
                "normalization_factor": decoder["normalization"]["factor"],
                "syndrome_stopping": decoder["stopping"]["kind"] == "syndrome",
                "batch_size": declared["workload"]["size"]["timed_batch"],
                "quality_frames": decoder["input"]["frames"],
            },
            "pilot_pairs": args.pilot_pairs if args.label == "pilot" else None,
        })
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id,
        "issue": addendum["family"]["issue"],
        "label": args.label,
        "campaign_seed": SEEDS[args.family],
        "addendum": args.addendum,
        "producing_manifest": "dev/active/3be770d5/survey/producing-inputs.json",
        "lock_path": LOCK,
        "wrapper": WRAPPER,
        "timing_override": None,
        "arms": arms,
        "cells": cells,
        "max_cells_per_session": args.max_cells_per_session,
    }
    with open(args.output, "x", encoding="utf-8") as handle:
        json.dump(plan, handle, indent=2)
        handle.write("\n")
    print(args.output)


if __name__ == "__main__":
    main()
