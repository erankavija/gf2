#!/usr/bin/env python3
"""Project the protocol runner plan for the frozen DVB profile addendum."""

import argparse
import json
import os


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--addendum", required=True)
    parser.add_argument("--campaign-id", required=True)
    parser.add_argument("--campaign-seed", required=True, type=int)
    parser.add_argument("--lock", required=True)
    parser.add_argument("--executable", required=True)
    parser.add_argument("--producing-manifest", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--max-cells-per-session", type=int, default=2)
    parser.add_argument("--pilot-pairs", type=int, default=6)
    args = parser.parse_args()

    with open(args.addendum) as handle:
        addendum = json.load(handle)
    executable = os.path.realpath(args.executable)
    arms = {
        "gf2-direct-a": {
            "build": "native",
            "description": (
                "First isolated null arm: current gf2-coding "
                "DvbT2BitInterleaver::interleave on one packed BitVec, including output allocation"
            ),
            "executable": executable,
            "arguments": [],
            "environment": {"GF2_DVB_PROFILE_ROUTE": "gf2-direct-a"},
            "rustflags": "-C target-cpu=native",
            "tuning_profile": None,
        },
        "gf2-direct-b": {
            "build": "native",
            "description": (
                "Second isolated null arm: byte-identical executable and current direct gf2 "
                "interleave route, independently launched for same-path noise"
            ),
            "executable": executable,
            "arguments": [],
            "environment": {"GF2_DVB_PROFILE_ROUTE": "gf2-direct-b"},
            "rustflags": "-C target-cpu=native",
            "tuning_profile": None,
        },
        "gf2-stage": {
            "build": "native",
            "description": (
                "Production gf2-sim BitInterleave::process on a one-frame BitPackedBatch, which "
                "calls the current gf2-coding packed scatter and returns a BitPackedBatch"
            ),
            "executable": executable,
            "arguments": [],
            "environment": {"GF2_DVB_PROFILE_ROUTE": "gf2-stage"},
            "rustflags": "-C target-cpu=native",
            "tuning_profile": None,
        },
        "xdsopl-stage": {
            "build": "external",
            "description": (
                "Operation-equivalent xdsopl PCTITL at the same one-frame BitPackedBatch boundary; "
                "unpack, destructive-input copy, output allocation and pack remain inside each call"
            ),
            "executable": executable,
            "arguments": [],
            "environment": {"GF2_DVB_PROFILE_ROUTE": "xdsopl-stage"},
            "rustflags": "-C target-cpu=native; C++ -O3 -march=native -std=c++17",
            "tuning_profile": None,
        },
    }

    cells = []
    used = set()
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        isolated = cell_id.endswith("-isolated-null")
        pair = ("gf2-direct-a", "gf2-direct-b") if isolated else ("gf2-stage", "xdsopl-stage")
        expected = (declared["builds"]["baseline"], declared["builds"]["candidate"])
        actual = (arms[pair[0]]["build"], arms[pair[1]]["build"])
        if expected != actual:
            raise SystemExit(f"{cell_id}: declared builds {expected} differ from projected {actual}")
        identity = declared["workload"]["identity"]
        prefix = "dvb-t2-bit-interleave-"
        suffix = "-isolated" if isolated else "-sim-stage"
        if not identity.startswith(prefix) or not identity.endswith(suffix):
            raise SystemExit(f"{cell_id}: workload identity does not encode its boundary")
        modcod = identity[len(prefix) : -len(suffix)]
        cells.append(
            {
                "cell_id": cell_id,
                "baseline_arm": pair[0],
                "candidate_arm": pair[1],
                "case": {"modcod": modcod, "seed": declared["workload"]["seed"]},
                "pilot_pairs": args.pilot_pairs,
            }
        )
        used.update(pair)

    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id,
        "issue": addendum["family"]["issue"],
        "label": "pilot",
        "campaign_seed": args.campaign_seed,
        "addendum": args.addendum,
        "producing_manifest": args.producing_manifest,
        "lock_path": os.path.realpath(args.lock),
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {name: arms[name] for name in sorted(used)},
        "cells": cells,
        "max_cells_per_session": args.max_cells_per_session,
    }
    with open(args.output, "w") as output:
        json.dump(plan, output, indent=2)
        output.write("\n")
    print(f"{args.output}: {len(cells)} cells, {len(used)} arms")


if __name__ == "__main__":
    main()
