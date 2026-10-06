#!/usr/bin/env python3
"""Render a `benchmark-ab-runner smoke` record as a committed text record.

Every line projects the record and states nothing else; the smoke's contract is
stated once, at `tuning_campaign_support::arm::smoke`.

Usage:
  summarize-arm-smoke.py --record <json> --output <path> --command <invocation> \\
      [--title <line>] [--check]
"""

import argparse
import json
import os
from pathlib import Path


def render(record, command, title):
    lines = [
        f"# {title} (jit:{record['issue']})",
        f"# command: {command}",
        "# contract: benchmark-ab-runner smoke (tuning_campaign_support::arm::smoke)",
        f"# campaign: {record['campaign_id']} plan sha256 {record['plan_sha256']}",
        f"# addendum: {record['addendum']} sha256 {record['addendum_sha256']}",
    ]
    dispatches = [arm for cell in record["cells"] for arm in cell["arms"]]
    for executable, digest in sorted(
        {(arm["executable"], arm["executable_sha256"]) for arm in dispatches}
    ):
        lines.append(f"# {os.path.basename(executable)} sha256: {digest}")
    for cell in record["cells"]:
        names = " and ".join(arm["arm"] for arm in cell["arms"])
        windows = sum(arm["windows"] for arm in cell["arms"])
        lines.append(
            f"PASS cell {cell['cell_id']}: {names} validated, "
            f"cache {cell['cache_state']}, {windows} timing windows"
        )
    for name in sorted({arm["arm"] for arm in dispatches}):
        own = [arm for arm in dispatches if arm["arm"] == name]
        lines.append(
            f"PASS arm {name}: {len(own)} dispatches, "
            f"{sum(arm['windows'] for arm in own)} timing windows"
        )
        for route in sorted({arm["selected_path"] for arm in own}):
            lines.append(f"PASS route {name}: {route}")
    lines.append(
        f"PASS smoke: {len(dispatches)} dispatches over {len(record['cells'])} cells, "
        f"{sum(arm['windows'] for arm in dispatches)} timing windows"
    )
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--record", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--command", required=True)
    parser.add_argument("--title", default="Non-timed arm smoke")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    record = json.loads(Path(args.record).read_text())
    rendered = render(record, args.command, args.title)
    if args.check:
        if Path(args.output).read_text() != rendered:
            raise SystemExit(f"{args.output} differs from the observed smoke")
        print(f"{args.output}: unchanged by this smoke")
        return
    Path(args.output).write_text(rendered)
    print(f"{args.output}: {len(record['cells'])} cells recorded")


if __name__ == "__main__":
    main()
