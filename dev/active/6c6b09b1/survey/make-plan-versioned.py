#!/usr/bin/env python3
"""Project the runner plan of one versioned-protocol byte-field campaign (jit:6c6b09b1).

Usage: make-plan-versioned.py --addendum A --label L --campaign-id ID
                       --campaign-seed S --lock PATH --target DIR
                       --max-cells-per-session N --producing-manifest M
                       [--pilot-pairs P] --output PLAN

The plan schema is independent of the protocol version, so this projector
serves every versioned addendum; the caller names the producing-input
manifest of the campaign generation it is projecting.

Every cell identifier ends in `-<baseline>-vs-<candidate>`, naming the two
arms it compares; the case both arms decode is built from the frozen
addendum's own workload identity, sizes, seed and metric kind, so the plan
cannot disagree with the declaration about what a cell measures. Every cell
works in GF(2^8) modulo 0x11D, the one field all compared arms share.

An arm is a fresh process per execution. The gf2 arms share one executable
and the three external arms share another; the environment is the only thing
that distinguishes arms sharing a binary, and the runner records it with the
executable digest.
"""

import argparse
import json
import os

POLY = 0x11D
RUSTFLAGS = "-C target-cpu=native"

# Workload identity prefix -> the case operation both arms perform.
OPERATIONS = {
    "gf256-0x11d-region-axpy": "axpy",
    "gf256-0x11d-pairwise-multiply": "pairwise",
    "gf256-0x11d-square-product": "matmul",
    "gf256-0x11d-generator-encode": "encode",
}


def arms(target):
    gf2 = os.path.join(target, "release", "gf2-arm")
    ext = os.path.join(target, "release", "ext-arm")

    def arm(build, executable, environment, description):
        return {
            "build": build,
            "description": description,
            "executable": executable,
            "arguments": [],
            "environment": environment,
            "rustflags": RUSTFLAGS,
            "tuning_profile": None,
        }

    return {
        "element": ("gf2-element", arm(
            "native", gf2, {"GF2_SURVEY_GF2_REPR": "element"},
            "gf2-core FieldVec/FieldMatrix<Gf2mElement> over Gf2mField::gf256(), "
            "built with -C target-cpu=native")),
        "batch": ("gf2-batch", arm(
            "native", gf2, {"GF2_SURVEY_GF2_REPR": "element"},
            "gf2-core gf2m::batch::batch_mul on u64 lanes over Gf2mField::gf256(), "
            "built with -C target-cpu=native")),
        "wide": ("gf2-wide", arm(
            "native", gf2, {"GF2_SURVEY_GF2_REPR": "wide"},
            "gf2-core FieldVec/FieldMatrix<Gf2mWide<1,Gf256x11d>>, the survey's 0x11D "
            "Gf2mWideConfig, built with -C target-cpu=native")),
        "isal": ("isal", arm(
            "external", ext, {"GF2_SURVEY_BACKEND": "isa-l", "GF2_SURVEY_VARIANT": "default"},
            "ISA-L v2.32.1 (7c3479e0) runtime-dispatched kernels through the C shim, "
            "gcc -O3 -march=native")),
        "gfcomplete": ("gfcomplete", arm(
            "external", ext,
            {"GF2_SURVEY_BACKEND": "gf-complete", "GF2_SURVEY_VARIANT": "default"},
            "GF-Complete ceph/gf-complete@a6862d10 w=8 default configuration through the "
            "C shim, gcc -O3 -march=native")),
        "m4rie": ("m4rie", arm(
            "external", ext, {"GF2_SURVEY_BACKEND": "m4rie", "GF2_SURVEY_VARIANT": "default"},
            "M4RIE 20250128 over M4RI 20260122 through the C shim, gcc -O3 -march=native")),
    }


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
        head, _, candidate = cell_id.rpartition("-vs-")
        baseline = head.rsplit("-", 1)[-1]
        if baseline not in catalogue or candidate not in catalogue:
            raise SystemExit(f"cell {cell_id} does not name two known arms")
        pair = []
        for role, short in (("baseline", baseline), ("candidate", candidate)):
            name, arm = catalogue[short]
            if arm["build"] != declared["builds"][role]:
                raise SystemExit(
                    f"cell {cell_id}: {role} arm {name} is {arm['build']}, "
                    f"addendum declares {declared['builds'][role]}")
            if plan_arms.setdefault(name, arm) != arm:
                raise SystemExit(f"arm {name} is declared twice with different settings")
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
