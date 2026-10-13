#!/usr/bin/env python3
"""Project the runner plan of one profile-family campaign (jit:63bad95d).

Usage: make-family-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                           --lock PATH --arm BUILD=EXECUTABLE [--arm ...]
                           --producing-manifest M --max-cells-per-session N --output PLAN

An arm is one build level of the one harness executable. Its RUSTFLAGS come
from the build table of `planned-families.json`, and a profile level names the
measured core owner of this issue's campaign, pinned by digest, in its
environment and as its tuning profile. Each cell's case and pilot pair count
come from the plan record and must agree with the frozen addendum's workload.
"""

import argparse
import hashlib
import json

from locate import HERE, ROOT, repository_files, tracked

BUILD_VAR = "GF2_PROFILE_BUILD"
OWNER_VAR = "GF2_PROFILE_OWNER"
OWNER_SHA256_VAR = "GF2_PROFILE_OWNER_SHA256"


def measured_owner(issue):
    """Root-relative path, profile identity and digest of the issue's measured core owner."""
    owners = tracked(f"**/gf2-core/**/tuning-profiles/gf2-{issue}-*.json")
    if len(owners) != 1:
        raise SystemExit(f"{len(owners)} measured core owners of issue {issue}; exactly one must exist")
    content = (ROOT / owners[0]).read_bytes()
    return owners[0], json.loads(content)["profile_id"], hashlib.sha256(content).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    for flag in ("addendum", "label", "campaign-id", "campaign-seed", "lock",
                 "producing-manifest", "max-cells-per-session", "output"):
        parser.add_argument(f"--{flag}", required=True)
    parser.add_argument("--arm", action="append", required=True, metavar="BUILD=EXECUTABLE")
    args = parser.parse_args()
    executables = dict(entry.split("=", 1) for entry in args.arm)

    record = json.loads((HERE / "planned-families.json").read_text())
    addendum = json.loads((ROOT / args.addendum).read_text())
    families = [family for family in record["families"] if family["id"] == addendum["family"]["id"]]
    if len(families) != 1:
        raise SystemExit(f"{addendum['family']['id']} is not a planned family")
    planned = {cell["cell_id"]: cell for cell in families[0]["cells"]}
    owner, profile_id, owner_sha256 = measured_owner(record["issue"])

    arms, cells = {}, []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        if cell_id not in planned:
            raise SystemExit(f"cell {cell_id} is not in the plan record")
        case = planned[cell_id]["case"]
        workload = dict(declared["workload"]["size"], seed=declared["workload"]["seed"])
        if {name: value for name, value in case.items() if name != "operation"} != workload:
            raise SystemExit(f"cell {cell_id}: the plan record's case disagrees with the addendum")
        for build in declared["builds"].values():
            if build in arms:
                continue
            if build not in executables:
                raise SystemExit(f"cell {cell_id} needs a {build} arm; pass --arm {build}=...")
            level = record["builds"][build]
            profiled = level["rustflags"] is not None
            environment = {BUILD_VAR: build}
            if profiled:
                environment[OWNER_VAR] = str(ROOT / owner)
                environment[OWNER_SHA256_VAR] = owner_sha256
            arms[build] = {
                "build": build,
                "description": f"profile-arm, {build}: {level['selectors']}",
                "executable": executables[build],
                "arguments": [],
                "environment": environment,
                "rustflags": level["rustflags"],
                "tuning_profile":
                    {"id": profile_id, "sha256": owner_sha256} if profiled else None,
            }
        cells.append({
            "cell_id": cell_id,
            "baseline_arm": declared["builds"]["baseline"],
            "candidate_arm": declared["builds"]["candidate"],
            "case": case,
            "pilot_pairs": record["pilot_pairs"] if declared["role"] == "exploratory" else None,
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
        "wrapper": f"{repository_files.live_file(ROOT, 'ccx1-bench-flock.sh')} --full-host",
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
