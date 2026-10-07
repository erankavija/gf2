#!/usr/bin/env python3
"""Project the runner plan of one byte-field consumer campaign (jit:19513245).

Usage: make-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                    --lock PATH --target DIR --max-cells-per-session N
                    --producing-manifest M [--pilot-pairs P] --output PLAN

Every cell identifier ends in the representation the consumer holds, which
selects the pair: the baseline arm is that representation's current gf2-core
route and the candidate arm is the byte-field prototype over the same field.
The case both arms decode is built from the frozen addendum's own workload
identity, sizes, seed and metric kind, so the plan cannot disagree with the
declaration about what a cell measures.

An arm is a fresh process per execution. Every arm shares one executable; the
environment is the only thing that distinguishes them, and the runner records
it with the executable digest.
"""

import argparse
import json
import os

POLY = 0x11D
# A conservative-portable build sets no target feature: the carry-less-multiply
# kernels the current routes reach are dispatched at run time and are present
# in such a build, and the prototype's scalar loop gets exactly the
# auto-vectorisation a shipped library gets.
RUSTFLAGS = None

# Workload identity prefix -> the case operation both arms perform.
OPERATIONS = {
    "gf256-0x11d-vector-axpy": "axpy",
    "gf256-0x11d-region-axpy": "axpy",
    "gf256-0x11d-square-product": "matmul",
    "gf256-0x11d-pairwise-multiply": "pairwise",
}

ROUTE_DESCRIPTIONS = {
    ("current", "element"): (
        "gf2-core FieldVec/FieldMatrix<Gf2mElement> over Gf2mField::gf256() through the entry "
        "point itself, conservative-portable build"),
    ("prototype", "element"): (
        "the byte-field coefficient-table prototype over Gf2mField::gf256(), writing back through "
        "Gf2mElement, conservative-portable build"),
    ("current", "wide"): (
        "gf2-core FieldVec/FieldMatrix<Gf2mWide<1,Gf256x11d>> through the entry point itself, "
        "conservative-portable build"),
    ("prototype", "wide"): (
        "the byte-field coefficient-table prototype over Gf2mWide<1,Gf256x11d>, "
        "conservative-portable build"),
    ("current", "batch"): (
        "gf2-core gf2m::batch::batch_mul on u64 lanes over Gf2mField::gf256(), "
        "conservative-portable build"),
    ("prototype", "batch"): (
        "the byte-field full-table prototype on byte regions over the same field, "
        "conservative-portable build"),
}


def arms(target):
    executable = os.path.join(target, "release", "consumer-arm")
    catalogue = {}
    for (route, representation), description in ROUTE_DESCRIPTIONS.items():
        catalogue[f"{route}-{representation}"] = {
            "build": "conservative-portable",
            "description": description,
            "executable": executable,
            "arguments": [],
            "environment": {
                "GF2_BYTEFIELD_ROUTE": route,
                "GF2_BYTEFIELD_REPR": representation,
            },
            "rustflags": RUSTFLAGS,
            "tuning_profile": None,
        }
    return catalogue


def case_for(declared):
    identity = declared["workload"]["identity"]
    operation = next(
        (op for prefix, op in OPERATIONS.items() if identity.startswith(prefix)), None)
    if operation is None:
        raise SystemExit(f"cell {declared['cell_id']}: unknown workload identity {identity!r}")
    size = declared["workload"]["size"]
    metric = declared["metric_kind"]
    if (metric == "whole-consumer") != declared["conversion_costs_included"]:
        raise SystemExit(f"cell {declared['cell_id']}: conversion flag disagrees with metric")
    return {
        "operation": operation,
        "bytes": size.get("bytes", 0),
        "n": size.get("n", 0),
        "k": size.get("k", 0),
        "rows": size.get("rows", 0),
        "poly": POLY,
        "metric": metric,
        "seed": declared["workload"]["seed"],
        "workers": declared["workers"]["declared"],
    }


def main():
    parser = argparse.ArgumentParser()
    for flag in ("addendum", "label", "campaign-id", "campaign-seed", "lock", "target",
                 "max-cells-per-session", "producing-manifest", "output"):
        parser.add_argument(f"--{flag}", required=True)
    parser.add_argument("--pilot-pairs", type=int)
    args = parser.parse_args()

    with open(args.addendum) as handle:
        addendum = json.load(handle)
    catalogue = arms(args.target)
    plan_arms = {}
    cells = []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        representation = cell_id.rsplit("-", 1)[-1]
        pair = []
        for role, route in (("baseline", "current"), ("candidate", "prototype")):
            name = f"{route}-{representation}"
            arm = catalogue.get(name)
            if arm is None:
                raise SystemExit(f"cell {cell_id} names no known representation")
            if arm["build"] != declared["builds"][role]:
                raise SystemExit(
                    f"cell {cell_id}: {role} arm {name} is {arm['build']}, "
                    f"addendum declares {declared['builds'][role]}")
            plan_arms.setdefault(name, arm)
            pair.append(name)
        exploratory = declared["role"] == "exploratory"
        cells.append({
            "cell_id": cell_id,
            "baseline_arm": pair[0],
            "candidate_arm": pair[1],
            "case": case_for(declared),
            "pilot_pairs": args.pilot_pairs if exploratory else None,
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
