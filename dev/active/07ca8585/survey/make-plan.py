#!/usr/bin/env python3
"""Emit the `zen3-benchmark-plan-v1` runner plan of one family (jit:07ca8585).

The addendum freezes what each cell is; the plan names the executables and
bundle directories that measure it on this host. Those are absolute host paths,
so the plan is generated beside its campaign stage and the receipt keeps its
exact bytes and digest.

Arms come from `arms.json`, which also names each family's baseline and
candidate arm and its campaign seed. The two gf2 generations are the same
harness built from two trees, and the isolated check-node arms are a third
build, so each arm names the generation whose target directory holds its
executable: `before`, `after` or `kernel`.

A cell's declared `metric_kind` decides the case the arms receive. A
whole-consumer cell receives the decoder case the steady-state arms decode, and
each arm receives the prepared quality of its own quality arm. A kernel-isolated
cell receives the prepared-array case the check-node arms read, including the
checksums the addendum's workload identity freezes, and no prepared quality:
an isolated check-node pass decodes no frame.

Usage:
  make-plan.py --family ID --label pilot|confirmation --addendum PATH
      --before-dir DIR --after-dir DIR --bundles-dir DIR --quality-dir DIR
      --campaign-id ID --output PLAN [--kernel-dir DIR]
      [--producing-manifest PATH]
"""

import argparse
import json
import os
import pathlib
import re

LOCK = "/tmp/gf2-ccx1.lock"
# The update rule of every cell this issue measures; the isolated arms take it
# from here because their cells declare no decoder to read it from.
NORMALIZATION_FACTOR = 0.75
WRAPPER = "dev/scripts/ccx1-bench-flock.sh"
RUSTFLAGS = "-C target-cpu=native"
CATALOGUE = json.loads((pathlib.Path(__file__).resolve().parent / "arms.json").read_text())
# The two checksums a kernel cell's frozen workload identity carries.
CHECKSUMS = re.compile(r"input ([0-9a-f]{16}), output ([0-9a-f]{16})")


def kernel_case(declared, bundle, code):
    """The prepared-array case both isolated check-node arms read."""
    identity = declared["workload"]["identity"]
    found = CHECKSUMS.search(identity)
    if not found:
        raise SystemExit(f"cell {declared['cell_id']} freezes no prepared-array checksums")
    size = declared["workload"]["size"]
    return {
        "bundle": bundle,
        "code": code,
        "normalization_factor": NORMALIZATION_FACTOR,
        "warmup_rounds": size["warmup_rounds"],
        "frames": size["timed_batch"],
        "input_checksum": found.group(1),
        "output_checksum": found.group(2),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--family", required=True, choices=sorted(CATALOGUE["families"]))
    parser.add_argument("--label", required=True, choices=["pilot", "confirmation"])
    for name in ("--addendum", "--before-dir", "--after-dir", "--bundles-dir",
                 "--quality-dir", "--campaign-id", "--output"):
        parser.add_argument(name, required=True)
    parser.add_argument("--kernel-dir")
    parser.add_argument(
        "--producing-manifest", default="dev/active/07ca8585/survey/producing-inputs.json"
    )
    parser.add_argument("--pilot-pairs", type=int, default=6)
    parser.add_argument("--max-cells-per-session", type=int, default=1)
    args = parser.parse_args()

    addendum = json.load(open(args.addendum, encoding="utf-8"))
    if addendum["family"]["id"] != args.family:
        raise SystemExit("the addendum belongs to another family")
    family = CATALOGUE["families"][args.family]
    generations = {
        "before": args.before_dir,
        "after": args.after_dir,
        "kernel": args.kernel_dir,
    }

    arms, cells = {}, []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        code_key = next(key for key in CATALOGUE["codes"] if cell_id.startswith(key + "-"))
        code = CATALOGUE["codes"][code_key]
        names = []
        for arm in (family["baseline"], family["candidate"]):
            spec = CATALOGUE["arms"][arm]
            directory = generations[spec["generation"]]
            if directory is None:
                raise SystemExit(f"arm {arm} needs the {spec['generation']} target directory")
            executable = os.path.join(directory, spec["executable"])
            if not os.path.isfile(executable):
                raise SystemExit("missing executable " + executable)
            key = f"{arm}-{code_key}"
            environment = dict(spec["environment"])
            if "quality_arm" in spec:
                environment["GF2_LDPC_QUALITY"] = os.path.join(
                    args.quality_dir, f"{spec['quality_arm']}-{code_key}.json"
                )
            arms[key] = {
                "build": spec["build"],
                "description": spec["description"],
                "executable": executable,
                "arguments": [],
                "environment": environment,
                "rustflags": RUSTFLAGS,
                "tuning_profile": None,
            }
            names.append(key)
        bundle = os.path.join(args.bundles_dir, code["bundle"])
        if declared["metric_kind"] == "kernel-isolated":
            case = kernel_case(declared, bundle, code["code"])
        else:
            decoder = declared["decoder"]
            case = {
                "bundle": bundle,
                "code": code["code"],
                "iteration_cap": decoder["iteration_cap"],
                "normalization_factor": decoder["normalization"]["factor"],
                "syndrome_stopping": decoder["stopping"]["kind"] == "syndrome",
                "batch_size": declared["workload"]["size"]["timed_batch"],
                "quality_frames": decoder["input"]["frames"],
            }
        cells.append({
            "cell_id": cell_id,
            "baseline_arm": names[0],
            "candidate_arm": names[1],
            "case": case,
            "pilot_pairs": args.pilot_pairs if args.label == "pilot" else None,
        })
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id,
        "issue": addendum["family"]["issue"],
        "label": args.label,
        "campaign_seed": family["seed"],
        "addendum": args.addendum,
        "producing_manifest": args.producing_manifest,
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
