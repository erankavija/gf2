#!/usr/bin/env python3
"""Project the runner plan of one entry-check cost campaign (jit:bfc2ceab).

Usage: make-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                    --lock PATH --executable BIN --producing-manifest M
                    --max-cells-per-session N [--pilot-pairs P] --output PLAN

The argument surface is the one the shared `smoke-campaign-arms.sh` and the
shared `campaign_plan.py` use. This family projects its own plan because its
two arms are two executables: `--executable` names the baseline build, and
the candidate is the same binary under the sibling target directory whose name
carries the `-checked` suffix, built with the candidate's RUSTFLAGS. Each case
is built from the frozen addendum's own workload identity, size and seed.
"""

import argparse
import json
import os
import subprocess

BUILD = "conservative-portable"
FORM_VAR = "GF2_ENTRY_FORM"
CANDIDATE_RUSTFLAGS = "--cfg gf2_entry_checked"
CANDIDATE_SUFFIX = "-checked"
CONSUMERS = ("sparse-matmat", "permanent-f3", "ple", "gemm")


def lock_wrapper(root):
    """The shared host lock wrapper, repository-relative as a plan names it."""
    listing = subprocess.run(
        ["git", "-C", root, "ls-files", "--", ":(glob)**/ccx1-bench-flock.sh"],
        check=True, capture_output=True, text=True,
    ).stdout.split()
    live = [path for path in listing if "inputs" not in path.split("/")[:-1]]
    if len(live) != 1:
        raise SystemExit(f"{len(live)} live lock wrappers; exactly one must exist")
    return live[0]


def candidate_executable(baseline):
    """The candidate build's binary: the baseline path under the `-checked` target."""
    release = os.path.dirname(baseline)
    target = os.path.dirname(release)
    return os.path.join(target + CANDIDATE_SUFFIX, os.path.basename(release),
                        os.path.basename(baseline))


def arm(executable, form, rustflags, text):
    return {
        "build": BUILD,
        "description": f"entry-check-arm, {BUILD} release build {text}",
        "executable": executable,
        "arguments": [],
        "environment": {FORM_VAR: form},
        "rustflags": rustflags,
        "tuning_profile": None,
    }


def case_for(declared):
    identity = declared["workload"]["identity"]
    consumer = next((name for name in CONSUMERS if identity.startswith(name + "-")), None)
    if consumer is None:
        raise SystemExit(f"cell {declared['cell_id']}: workload {identity!r} names no consumer")
    return {"consumer": consumer} | declared["workload"]["size"] | {
        "seed": declared["workload"]["seed"]}


def main():
    parser = argparse.ArgumentParser()
    for flag in ("addendum", "label", "campaign-id", "campaign-seed", "lock", "executable",
                 "producing-manifest", "max-cells-per-session", "output"):
        parser.add_argument(f"--{flag}", required=True)
    parser.add_argument("--pilot-pairs", type=int)
    args = parser.parse_args()
    root = subprocess.run(
        ["git", "-C", os.path.dirname(os.path.abspath(__file__)), "rev-parse",
         "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    with open(args.addendum) as handle:
        addendum = json.load(handle)
    arms = {
        "current-entry": arm(
            args.executable, "current", None,
            "without a flag: the kernel crate publishes the entry points it publishes today"),
        "checked-entry": arm(
            candidate_executable(args.executable), "checked", CANDIDATE_RUSTFLAGS,
            f"with RUSTFLAGS `{CANDIDATE_RUSTFLAGS}`: the kernel crate publishes the checked "
            "entry points"),
    }
    cells = []
    for declared in addendum["cells"]:
        if declared["builds"] != {"baseline": BUILD, "candidate": BUILD}:
            raise SystemExit(f"cell {declared['cell_id']}: both arms are {BUILD} builds")
        if not (declared["metric_kind"] == "whole-consumer"
                and declared["conversion_costs_included"]):
            raise SystemExit(f"cell {declared['cell_id']}: every cell is whole-consumer")
        cells.append({
            "cell_id": declared["cell_id"],
            "baseline_arm": "current-entry",
            "candidate_arm": "checked-entry",
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
        "wrapper": f"{lock_wrapper(root)} --full-host",
        "timing_override": None,
        "arms": arms,
        "cells": cells,
        "max_cells_per_session": int(args.max_cells_per_session),
    }
    with open(args.output, "w") as output:
        json.dump(plan, output, indent=2)
        output.write("\n")
    print(f"plan: {len(cells)} cells, {len(arms)} arms -> {args.output}")


if __name__ == "__main__":
    main()
