#!/usr/bin/env python3
"""Project the runner plan of one GF(2^8) axpy campaign (jit:ad2a6a58).

Usage: make-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                    --lock PATH --executable BIN --producing-manifest M
                    --max-cells-per-session N [--pilot-pairs P] --output PLAN

Both arms of every pair are the one executable this projection names; the
environment selects the lane and the element representation, and the runner
records that environment beside the executable digest, so the two arms of a
pair share a build identity and differ only in the lane. The case both arms
decode is built from the frozen addendum's own workload identity, size, seed
and metric kind, so the plan cannot disagree with the declaration about what a
cell measures.
"""

import argparse
import json

POLY = 0x11D
# A conservative-portable build sets no target feature: the carry-less-multiply
# kernels the scalar element lane reaches are dispatched at run time and are
# present in such a build, and the table lane's byte loop gets exactly the
# auto-vectorisation a shipped library gets.
RUSTFLAGS = None

WORKLOAD_PREFIX = "gf256-0x11d-vector-axpy"

LANES = {
    "scalar": "holds every GF(2^8) call on the scalar element lane through the shipped lane "
              "switch, so FieldVec::axpy runs the element loop it runs when the hook declines",
    "table": "leaves the shipped lane switch clear, so gf256_table_dispatch selects the cached "
             "product table",
}

REPRESENTATIONS = {
    "element": "FieldVec<Gf2mElement> over Gf2mField::gf256()",
    "wide": "FieldVec<Gf2mWide<1,Gf256x11d>>",
}


def arms(executable):
    catalogue = {}
    for lane, lane_text in LANES.items():
        for representation, representation_text in REPRESENTATIONS.items():
            catalogue[f"{lane}-{representation}"] = {
                "build": "conservative-portable",
                "description": (
                    f"{representation_text}: the shipped gf2-core axpy path, which {lane_text}; "
                    "conservative-portable build"
                ),
                "executable": executable,
                "arguments": [],
                "environment": {
                    "GF2_GF256_LANE": lane,
                    "GF2_GF256_REPR": representation,
                },
                "rustflags": RUSTFLAGS,
                "tuning_profile": None,
            }
    return catalogue


def case_for(declared):
    identity = declared["workload"]["identity"]
    if not identity.startswith(WORKLOAD_PREFIX):
        raise SystemExit(
            f"cell {declared['cell_id']}: this family measures FieldVec::axpy; the workload "
            f"identity is {identity!r}")
    metric = declared["metric_kind"]
    if (metric == "whole-consumer") != declared["conversion_costs_included"]:
        raise SystemExit(f"cell {declared['cell_id']}: conversion flag disagrees with metric")
    return {
        "operation": "axpy",
        "bytes": declared["workload"]["size"]["bytes"],
        "n": 0,
        "k": 0,
        "rows": 0,
        "poly": POLY,
        "metric": metric,
        "seed": declared["workload"]["seed"],
        "workers": declared["workers"]["declared"],
    }


def main():
    parser = argparse.ArgumentParser()
    for flag in ("addendum", "label", "campaign-id", "campaign-seed", "lock", "executable",
                 "producing-manifest", "max-cells-per-session", "output"):
        parser.add_argument(f"--{flag}", required=True)
    parser.add_argument("--pilot-pairs", type=int)
    args = parser.parse_args()

    with open(args.addendum) as handle:
        addendum = json.load(handle)
    catalogue = arms(args.executable)
    plan_arms = {}
    cells = []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        representation = cell_id.rsplit("-", 1)[-1]
        if representation not in REPRESENTATIONS:
            raise SystemExit(f"cell {cell_id} names no known representation")
        pair = []
        for role, lane in (("baseline", "scalar"), ("candidate", "table")):
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
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": plan_arms,
        "cells": cells,
        "max_cells_per_session": int(args.max_cells_per_session),
    }
    with open(args.output, "w") as output:
        json.dump(plan, output, indent=2)
        output.write("\n")
    print(f"plan: {len(cells)} cells, {len(plan_arms)} arms -> {args.output}")


if __name__ == "__main__":
    main()
