#!/usr/bin/env python3
"""Emit the `zen3-benchmark-plan-v1` runner plan of one family (jit:07ca8585).

The addendum freezes what each cell is; the plan names the executables and
bundle directories that measure it on this host. Those are absolute host paths,
so the plan is generated beside its campaign stage and the receipt keeps its
exact bytes and digest.

Arms come from `arms.json`, which also names each family's baseline and
candidate arm and its campaign seed. The two gf2 generations are the same
harness built from two trees, so each arm names the generation whose target
directory holds its executable.

Usage:
  make-plan.py --family ID --label pilot|confirmation --addendum PATH
      --before-dir DIR --after-dir DIR --bundles-dir DIR --quality-dir DIR
      --campaign-id ID --output PLAN
"""

import argparse
import json
import os
import pathlib

LOCK = "/tmp/gf2-ccx1.lock"
WRAPPER = "dev/scripts/ccx1-bench-flock.sh"
RUSTFLAGS = "-C target-cpu=native"
CATALOGUE = json.loads((pathlib.Path(__file__).resolve().parent / "arms.json").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--family", required=True, choices=sorted(CATALOGUE["families"]))
    parser.add_argument("--label", required=True, choices=["pilot", "confirmation"])
    for name in ("--addendum", "--before-dir", "--after-dir", "--bundles-dir",
                 "--quality-dir", "--campaign-id", "--output"):
        parser.add_argument(name, required=True)
    parser.add_argument("--pilot-pairs", type=int, default=6)
    parser.add_argument("--max-cells-per-session", type=int, default=1)
    args = parser.parse_args()

    addendum = json.load(open(args.addendum, encoding="utf-8"))
    if addendum["family"]["id"] != args.family:
        raise SystemExit("the addendum belongs to another family")
    family = CATALOGUE["families"][args.family]
    generations = {"before": args.before_dir, "after": args.after_dir}

    arms, cells = {}, []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        code_key = next(key for key in CATALOGUE["codes"] if cell_id.startswith(key + "-"))
        code = CATALOGUE["codes"][code_key]
        names = []
        for arm in (family["baseline"], family["candidate"]):
            spec = CATALOGUE["arms"][arm]
            executable = os.path.join(generations[spec["generation"]], spec["executable"])
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
                    GF2_LDPC_QUALITY=os.path.join(
                        args.quality_dir, f"{spec['quality_arm']}-{code_key}.json"
                    ),
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
        "campaign_seed": family["seed"],
        "addendum": args.addendum,
        "producing_manifest": "dev/active/07ca8585/survey/producing-inputs.json",
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
