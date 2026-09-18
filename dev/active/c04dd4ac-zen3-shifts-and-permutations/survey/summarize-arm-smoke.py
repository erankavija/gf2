#!/usr/bin/env python3
"""Render a non-timed arm smoke's observations as its committed record.

The smoke driver writes one observation per dispatch; this renderer aggregates
them per arm and per cell and states nothing it did not read. Both committed
formats carry the same facts: plan and arm identity, the route each arm
selected, the result lines parsed and the timing-window count, which is zero
because the driver refuses an arm that reports one. No value is a clock reading,
so a rerun on the same executables reproduces the record byte for byte.

Usage:
  summarize-arm-smoke.py --observations <json> --format text|json \\
      --output <path> --command <invocation> [--title <line>] [--check]
"""

import argparse
import collections
import json
import os
from pathlib import Path

SCOPE = (
    "non-timed validation-role dispatch of every arm of the plan's cells, through the "
    "runner's own request framing and child environment; the smoke opens no campaign, "
    "takes no lock, reserves nothing in a family ledger, finalizes no receipt and "
    "records no timing window"
)


def aggregate(observations):
    dispatches = observations["dispatches"]
    per_arm = collections.defaultdict(list)
    per_cell = collections.defaultdict(list)
    for dispatch in dispatches:
        per_arm[dispatch["arm"]].append(dispatch)
        per_cell[dispatch["cell_id"]].append(dispatch)
    arms = []
    for identity in observations["arms"]:
        name = identity["arm"]
        own = per_arm[name]
        arms.append(
            {
                **identity,
                "dispatches": len(own),
                "result_lines": sum(dispatch["result_lines"] for dispatch in own),
                "timing_windows": sum(dispatch["timing_windows"] for dispatch in own),
                "workers_observed": sorted({dispatch["workers_observed"] for dispatch in own}),
                "routes": sorted({dispatch["selected_path"] for dispatch in own}),
            }
        )
    cells = [
        {
            "cell_id": cell_id,
            "cache_state": own[0]["cache_state_applied"],
            "arms": [dispatch["arm"] for dispatch in own],
            "result_lines": sum(dispatch["result_lines"] for dispatch in own),
            "timing_windows": sum(dispatch["timing_windows"] for dispatch in own),
        }
        for cell_id, own in per_cell.items()
    ]
    return arms, cells


def render_json(observations, command):
    arms, cells = aggregate(observations)
    return json.dumps(
        {
            "schema": "zen3-arm-smoke-record-v1",
            "issue": observations["plan"]["issue"],
            "scope": SCOPE,
            "command": command,
            "plan": observations["plan"],
            "arms": arms,
            "cells": cells,
            "dispatches": len(observations["dispatches"]),
            "result_lines": sum(d["result_lines"] for d in observations["dispatches"]),
            "timing_windows": sum(d["timing_windows"] for d in observations["dispatches"]),
        },
        indent=2,
    ) + "\n"


def render_text(observations, command, title):
    arms, cells = aggregate(observations)
    plan = observations["plan"]
    lines = [
        f"# {title} (jit:{plan['issue']})",
        f"# command: {command}",
        f"# scope: {SCOPE}",
        f"# plan: {plan['path']} sha256 {plan['sha256']}",
        f"# addendum: {plan['addendum']} sha256 {plan['addendum_sha256']}",
    ]
    for executable, digest in sorted({(a["executable"], a["sha256"]) for a in arms}):
        lines.append(f"# {os.path.basename(executable)} sha256: {digest}")
    for cell in cells:
        lines.append(
            f"PASS cell {cell['cell_id']}: {' and '.join(cell['arms'])} validated, "
            f"{cell['result_lines']} result lines, {cell['timing_windows']} timing windows"
        )
    for arm in arms:
        lines.append(
            f"PASS arm {arm['arm']}: {arm['dispatches']} dispatches, "
            f"{arm['result_lines']} result lines, {arm['timing_windows']} timing windows, "
            f"workers observed {','.join(str(count) for count in arm['workers_observed'])}"
        )
        for route in arm["routes"]:
            lines.append(f"PASS route {arm['arm']}: {route}")
    lines.append(
        f"PASS smoke: {len(observations['dispatches'])} dispatches, "
        f"{sum(d['result_lines'] for d in observations['dispatches'])} result lines, "
        f"{sum(d['timing_windows'] for d in observations['dispatches'])} timing windows"
    )
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--observations", required=True)
    parser.add_argument("--format", required=True, choices=("text", "json"))
    parser.add_argument("--output", required=True)
    parser.add_argument("--command", required=True)
    parser.add_argument("--title", default="Non-timed arm smoke")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    observations = json.loads(Path(args.observations).read_text())
    if args.format == "json":
        rendered = render_json(observations, args.command)
    else:
        rendered = render_text(observations, args.command, args.title)
    if args.check:
        if Path(args.output).read_text() != rendered:
            raise SystemExit(f"{args.output} differs from the observed smoke")
        print(f"{args.output}: unchanged by this smoke")
        return
    Path(args.output).write_text(rendered)
    print(f"{args.output}: {len(observations['dispatches'])} dispatches recorded")


if __name__ == "__main__":
    main()
