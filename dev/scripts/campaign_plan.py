#!/usr/bin/env python3
"""Shared projection of a `benchmark-ab-runner` plan from a frozen addendum.

The argument surface, the per-cell arm pairing, the build-identity check
against the declaration and the plan envelope are the same for every family
and live here. A family's own generator puts this file's directory on
`sys.path`, imports `campaign_plan` and calls one of two entry points.

`project` is the general form: the family supplies the two arms of each cell,
which may be one executable under two environments or two executables built
with different flags, and the case the cell's arms decode.

`main` is the lane-comparison form over `project`: the family supplies its
lanes, its element representations and the entry point they name. Both arms of
a pair are then the one executable the projection names; the environment
selects the lane and the element representation, and the runner records that
environment beside the executable digest, so the two arms share a build
identity and differ only in the lane.

Each case is built from the frozen addendum's own workload identity, size, seed
and metric kind, so the plan cannot disagree with the declaration about what a
cell measures.
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


def arm(build, description, executable, environment, rustflags=RUSTFLAGS):
    """One plan arm: an executable, its environment and the flags it was built with."""
    return {
        "build": build,
        "description": description,
        "executable": executable,
        "arguments": [],
        "environment": environment,
        "rustflags": rustflags,
        "tuning_profile": None,
    }


def sibling_executable(executable, suffix):
    """The same binary under the target directory whose name carries `suffix`.

    A family whose candidate is a second build of the baseline's package keeps
    the two builds in sibling target directories, `<target>` and
    `<target><suffix>`.
    """
    release = os.path.dirname(executable)
    target = os.path.dirname(release)
    return os.path.join(target + suffix, os.path.basename(release),
                        os.path.basename(executable))


def arm_catalogue(executable, lanes, representations, entry_point, lane_var=LANE_VAR,
                  representation_var=REPR_VAR):
    """One arm per lane and representation, all the one executable."""
    catalogue = {}
    for lane, lane_text in lanes.items():
        for representation, representation_text in representations.items():
            catalogue[f"{lane}-{representation}"] = arm(
                BUILD,
                f"{representation_text}: {entry_point}, which {lane_text}; "
                f"{BUILD} build",
                executable,
                {lane_var: lane, representation_var: representation},
            )
    return catalogue


def project(arms_for, case_for):
    """Writes the plan the arguments name.

    `arms_for` receives one cell declaration and the `--executable` argument and
    returns the cell's baseline and candidate arms, each as a plan name and an
    `arm` record. `case_for` receives one cell declaration and returns the
    case both arms of that cell decode.
    """
    args = parse_arguments()
    with open(args.addendum) as handle:
        addendum = json.load(handle)
    plan_arms = {}
    cells = []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        if (declared["metric_kind"] == "whole-consumer") != declared["conversion_costs_included"]:
            raise SystemExit(f"cell {cell_id}: conversion flag disagrees with metric")
        pair = []
        for role, (name, record) in zip(("baseline", "candidate"),
                                        arms_for(declared, args.executable)):
            if record["build"] != declared["builds"][role]:
                raise SystemExit(
                    f"cell {cell_id}: {role} arm {name} is {record['build']}, "
                    f"addendum declares {declared['builds'][role]}")
            plan_arms.setdefault(name, record)
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


def main(lanes, representations, entry_point, case_for, lane_var=LANE_VAR,
         representation_var=REPR_VAR):
    """Writes the plan of a lane-comparison family.

    `lanes` maps a lane key to the sentence the arm description gives it, the
    baseline lane first and the candidate lane second. `representations` maps a
    representation key, which is a cell identifier's last dash-separated segment,
    to the type the consumer holds. `lane_var` and `representation_var` name the
    environment variables the family's arm reads them from. `case_for` receives
    one cell declaration and returns the case both arms of that cell decode.
    """
    baseline_lane, candidate_lane = list(lanes)

    def arms_for(declared, executable):
        catalogue = arm_catalogue(executable, lanes, representations, entry_point, lane_var,
                                  representation_var)
        representation = declared["cell_id"].rsplit("-", 1)[-1]
        if representation not in representations:
            raise SystemExit(f"cell {declared['cell_id']} names no known representation")
        return [(f"{lane}-{representation}", catalogue[f"{lane}-{representation}"])
                for lane in (baseline_lane, candidate_lane)]

    project(arms_for, case_for)
