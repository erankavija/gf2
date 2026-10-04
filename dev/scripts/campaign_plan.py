#!/usr/bin/env python3
"""Shared projection of a `benchmark-ab-runner` plan from a frozen addendum.

A lane-comparison family supplies its lanes, its element representations, the
entry point they name and the case its cells decode; the argument surface, the
per-cell arm pairing, the build-identity check against the declaration and the
plan envelope are the same for every such family and live here. A family's own
generator puts this file's directory on `sys.path`, imports `campaign_plan` and
calls `campaign_plan.main(...)`.

Both arms of a pair are the one executable the projection names; the environment
selects the lane and the element representation, and the runner records that
environment beside the executable digest, so the two arms share a build identity
and differ only in the lane. Each case is built from the frozen addendum's own
workload identity, size, seed and metric kind, so the plan cannot disagree with
the declaration about what a cell measures.
"""

import argparse
import json
import os
import subprocess

# A conservative-portable build sets no target feature: the kernels a declining
# lane reaches are dispatched at run time and are present in such a build, and a
# table lane's byte loop gets exactly the auto-vectorisation a shipped library
# gets.
RUSTFLAGS = None

BUILD = "conservative-portable"

# The shipped GF(2^8) lane contract's environment names, as the arm library
# `gf256-axpy-arm` declares them.
LANE_VAR = "GF2_GF256_LANE"
REPR_VAR = "GF2_GF256_REPR"


def lock_wrapper():
    """The host lock wrapper beside this file, repository-relative as a plan names it."""
    here = os.path.dirname(os.path.abspath(__file__))
    root = subprocess.run(
        ["git", "-C", here, "rev-parse", "--show-toplevel"], check=True, capture_output=True,
        text=True,
    ).stdout.strip()
    return os.path.relpath(os.path.join(here, "ccx1-bench-flock.sh"), root)


def parse_arguments():
    """The argument surface every family's plan projection carries."""
    parser = argparse.ArgumentParser()
    for flag in ("addendum", "label", "campaign-id", "campaign-seed", "lock", "executable",
                 "producing-manifest", "max-cells-per-session", "output"):
        parser.add_argument(f"--{flag}", required=True)
    parser.add_argument("--pilot-pairs", type=int)
    return parser.parse_args()


def arm_catalogue(executable, lanes, representations, entry_point):
    """One arm per lane and representation, all the one executable."""
    catalogue = {}
    for lane, lane_text in lanes.items():
        for representation, representation_text in representations.items():
            catalogue[f"{lane}-{representation}"] = {
                "build": BUILD,
                "description": (
                    f"{representation_text}: {entry_point}, which {lane_text}; "
                    f"{BUILD} build"
                ),
                "executable": executable,
                "arguments": [],
                "environment": {
                    LANE_VAR: lane,
                    REPR_VAR: representation,
                },
                "rustflags": RUSTFLAGS,
                "tuning_profile": None,
            }
    return catalogue


def main(lanes, representations, entry_point, case_for):
    """Writes the plan the arguments name.

    `lanes` maps a lane key to the sentence the arm description gives it, the
    baseline lane first and the candidate lane second. `representations` maps a
    representation key, which is a cell identifier's last dash-separated segment,
    to the type the consumer holds. `case_for` receives one cell declaration and
    returns the case both arms of that cell decode.
    """
    args = parse_arguments()
    with open(args.addendum) as handle:
        addendum = json.load(handle)
    catalogue = arm_catalogue(args.executable, lanes, representations, entry_point)
    baseline_lane, candidate_lane = list(lanes)
    plan_arms = {}
    cells = []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        representation = cell_id.rsplit("-", 1)[-1]
        if representation not in representations:
            raise SystemExit(f"cell {cell_id} names no known representation")
        if (declared["metric_kind"] == "whole-consumer") != declared["conversion_costs_included"]:
            raise SystemExit(f"cell {cell_id}: conversion flag disagrees with metric")
        pair = []
        for role, lane in (("baseline", baseline_lane), ("candidate", candidate_lane)):
            name = f"{lane}-{representation}"
            arm = catalogue[name]
            if arm["build"] != declared["builds"][role]:
                raise SystemExit(
                    f"cell {cell_id}: {role} arm {name} is {arm['build']}, "
                    f"addendum declares {declared['builds'][role]}")
            plan_arms.setdefault(name, arm)
            pair.append(name)
        cells.append({
            "cell_id": cell_id,
            "baseline_arm": pair[0],
            "candidate_arm": pair[1],
            "case": case_for(declared),
            "pilot_pairs": args.pilot_pairs if declared["role"] == "exploratory" else None,
        })

    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id,
        "issue": addendum["family"]["issue"],
        "label": args.label,
        "campaign_seed": int(args.campaign_seed),
        "addendum": args.addendum,
        "producing_manifest": args.producing_manifest,
        "lock_path": args.lock,
        "wrapper": f"{lock_wrapper()} --full-host",
        "timing_override": None,
        "arms": plan_arms,
        "cells": cells,
        "max_cells_per_session": int(args.max_cells_per_session),
    }
    with open(args.output, "w") as output:
        json.dump(plan, output, indent=2)
        output.write("\n")
    print(f"plan: {len(cells)} cells, {len(plan_arms)} arms -> {args.output}")
