#!/usr/bin/env python3
"""Summarize the residual-shift wire smoke as a committed structural artifact.

The smoke's durations are throwaway, so this summary records only what the
receipt and the execution log state about the handshake: which executables
spoke, how many children the runner spawned and reaped cleanly, whether any
child wrote a diagnostic, and which fields of each arm's one result line the
runner parsed. Every value is read from those two files at run time.
"""

import argparse
import collections
import hashlib
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
# Fields a parsed arm result line contributes to a pair record. Their presence
# is what distinguishes a decoded result from a child that only exited zero.
RESULT_FIELDS = ("windows", "cache_state_applied", "workers_observed", "selected_path")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def relative(path):
    return os.path.relpath(Path(path).resolve(), ROOT)


def summarize(receipt_path, log_path, arm, runner):
    receipt = json.loads(Path(receipt_path).read_text())
    events = [json.loads(line) for line in Path(log_path).read_text().splitlines() if line]
    counts = collections.Counter(event["event"] for event in events)
    diagnostics = [
        event["details"].get("stderr", "")
        for event in events
        if event["event"] == "child-diagnostic"
    ]
    exits = [
        event["details"]["outcome"]
        for event in events
        if event["event"] == "child-exit"
    ]

    arms = collections.Counter()
    fields = collections.Counter()
    paths = collections.defaultdict(set)
    cells = []
    for cell in receipt["cells"]:
        pairs = cell.get("pairs") or []
        for pair in pairs:
            for role in ("baseline", "candidate"):
                record = pair[role]
                arms[record["arm"]] += 1
                paths[record["arm"]].add(record["selected_path"])
                for field in RESULT_FIELDS:
                    if record.get(field) is not None:
                        fields[record["arm"], field] += 1
        cells.append(
            {
                "cell_id": cell["cell_id"],
                "role": cell["role"],
                "status": cell["status"],
                "baseline_arm": cell["baseline_arm"],
                "candidate_arm": cell["candidate_arm"],
                "pairs": len(pairs),
            }
        )

    arm_records = [
        {
            "arm": name,
            "result_lines_parsed": arms[name],
            "selected_paths": sorted(paths[name]),
            "result_fields_present": sorted(
                field for (owner, field) in fields if owner == name
            ),
            "complete_result_lines": min(
                fields[name, field] for field in RESULT_FIELDS
            ),
        }
        for name in sorted(arms)
    ]

    failures = []
    if diagnostics:
        failures.append(f"{len(diagnostics)} child diagnostics")
    if any(exit_["exit_code"] != 0 for exit_ in exits):
        failures.append("a child exited nonzero")
    if counts["complete"] != 1:
        failures.append("the campaign did not reach a terminal complete record")
    if counts["cell-start"] != counts["cell-complete"]:
        failures.append("cell-start and cell-complete disagree")
    if not cells or any(cell["pairs"] == 0 for cell in cells):
        failures.append("a cell carries no paired execution")
    for record in arm_records:
        if record["result_lines_parsed"] != record["complete_result_lines"]:
            failures.append(f"{record['arm']} wrote an incomplete result line")
        if sorted(RESULT_FIELDS) != record["result_fields_present"]:
            failures.append(f"{record['arm']} omitted a result field")

    return {
        "schema": "bitvec-residual-shift-wire-smoke-v1",
        "issue": "85fc5ff4",
        "scope": (
            "throwaway protocol-v4 run of the queued arm executable through the "
            "real benchmark-ab-runner; the handshake and the result framing are "
            "the observation, the durations are discarded"
        ),
        "campaign_id": receipt["campaign_id"],
        "family_id": receipt["family_id"],
        "runner": {"path": relative(runner), "sha256": digest(runner)},
        "arm_executable": {"path": relative(arm), "sha256": digest(arm)},
        "children_spawned": counts["child-spawn"],
        "children_exited_zero": sum(1 for exit_ in exits if exit_["exit_code"] == 0),
        "children_reaped": sum(1 for exit_ in exits if exit_["all_descendants_reaped"]),
        "child_diagnostics": diagnostics,
        "terminal_events": sorted(
            event for event in ("complete", "failed", "paused") if counts[event]
        ),
        "cells": cells,
        "arms": arm_records,
        "passed": not failures,
        "failures": failures,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--log", required=True)
    parser.add_argument("--arm", required=True)
    parser.add_argument("--runner", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    report = summarize(args.receipt, args.log, args.arm, args.runner)
    encoded = json.dumps(report, indent=2) + "\n"
    if args.check:
        observed = Path(args.output).read_text()
        if observed != encoded:
            raise SystemExit(f"{args.output} differs from the observed smoke")
    else:
        Path(args.output).write_text(encoded)
    if not report["passed"]:
        raise SystemExit("; ".join(report["failures"]))
    print(
        f"{args.output}: {len(report['cells'])} cells, "
        f"{report['children_spawned']} children, "
        f"{len(report['child_diagnostics'])} diagnostics"
    )


if __name__ == "__main__":
    main()
