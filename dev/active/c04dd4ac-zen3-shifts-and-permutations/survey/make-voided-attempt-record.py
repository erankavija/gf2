#!/usr/bin/env python3
"""Write the voided-attempt record for the residual-shift profile (jit:85fc5ff4).

Every count, identity and digest comes from the preserved stage: the execution
log, its addendum snapshot and its reservation snapshot. The record's prose
states the defect and the executor's decision; its figures are read, never
typed.
"""

import argparse
import collections
import json
from pathlib import Path

STAGE = "dev/bench_results/85fc5ff4/2026-09-16-85fc5ff4-residual-shift-profile-abandoned"
OUTPUT = "dev/bench_results/85fc5ff4/v4-voided-launch-attempt.json"
LEDGER = (
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/"
    "shift-profile-trial-ledger.jsonl"
)
SUMMARY = (
    "The executor aborted this exploratory attempt for a procedural defect in its own "
    "launch, before reading any of its results, and voided it under the protocol's "
    "voided-attempt rule. Its reservation does not enter the chain the replacement "
    "attempt reserves on, so it is never interior to that chain and it spends no "
    "comparison and no candidate attempt. The aborted stage is preserved whole beside "
    "the published outcomes. The rule reaches only an attempt whose results were "
    "unread; an attempt whose results were read spends its reservation like any other."
)
DEFECT = (
    "The arm executable this attempt launched could not complete the canonical "
    "child-v2 handshake. The runner forwards a plan cell's case as a serde_json "
    "Value, whose object keys re-encode in sorted order; the arm deserialized the "
    "request into a type whose case was the typed ShiftCase, which re-encodes in "
    "declaration order. transport::decode_canonical accepts a request only when the "
    "child re-encodes the runner's bytes exactly, so it rejected the first request "
    "and the child exited 2 before any timing began. A second defect stood behind "
    "it: the arm required the request's role field to read \"exploratory\", while the "
    "wire carries the arm's position in the pair. The defect is in the launched arm, "
    "not in the runner, the transport or the plan."
)
RESOLUTION = (
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/shift-profile-smoke.json records "
    "the same executable, rebuilt from the corrected arm, completing the handshake and "
    "the result framing through the same runner on a throwaway plan."
)
RULE = (
    "dev/active/f547c394/protocol.md, the voided-attempt paragraph of the "
    "family-ledger section; dev/active/f547c394/amendment-v4.md records the entry."
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", default=STAGE)
    parser.add_argument("--output", default=OUTPUT)
    args = parser.parse_args()

    stage = Path(args.stage)
    events = [
        json.loads(line)
        for line in (stage / "execution.log").read_text().splitlines()
        if line
    ]
    counts = collections.Counter(event["event"] for event in events)
    start = events[0]
    addendum = json.loads((stage / "inputs/family-addendum.json").read_text())
    reservation = [
        json.loads(line)
        for line in (stage / "inputs/trial-ledger.jsonl").read_text().splitlines()
        if line
    ]
    declared = [cell["cell_id"] for cell in addendum["cells"]]
    measured = sorted(
        {
            event["case"]["cell_id"]
            for event in events
            if event["event"] == "cell-complete"
        }
    )
    diagnostics = [
        event["details"].get("stderr", "")
        for event in events
        if event["event"] == "child-diagnostic"
    ]
    terminal = [
        event for event in events if event["event"] in ("complete", "failed", "paused")
    ]
    units = sorted((stage / "checkpoints/units").glob("*.json"))

    record = {
        "kind": "voided-family-ledger-attempt",
        "summary": SUMMARY,
        "attempt": {
            "family": addendum["family"]["id"],
            "campaign": start["campaign_id"],
            "protocol_version": addendum["protocol"]["version"],
            "rule": RULE,
            "rule_reading": (
                "The family is exploratory-only, so its reservation spends zero "
                "comparisons under the ledger section's ordinary rule as well. "
                "Voiding changes only the chain the replacement attempt reserves on."
            ),
            "stage": str(stage),
            "addendum": start["details"]["addendum"]["path"],
            "addendum_sha256": start["details"]["addendum"]["sha256"],
            "defect": DEFECT,
            "defect_diagnostics": diagnostics,
            "resolution": RESOLUTION,
            "results_read": False,
            "results_read_detail": (
                "No cell reached a checkpoint and no execution produced a window: the "
                "first child of the first cell failed the handshake. No interval, "
                "decision or outcome of this attempt exists to be read, and none is "
                "quoted anywhere in this profile."
            ),
            "abort": {
                "actor": "executor",
                "launcher_exit_code": 1,
                "receipt_written": False,
                "terminal_event": terminal[0]["event"] if terminal else False,
                "terminal_error": (
                    terminal[0]["details"].get("error") if terminal else None
                ),
            },
            "reservation": {
                "comparisons": reservation[0]["comparisons"],
                "candidates": reservation[0]["candidates"],
                "in_family_ledger": False,
                "family_ledger": LEDGER,
                "snapshot": str(stage / "inputs/trial-ledger.jsonl"),
                "spends": (
                    "nothing: no comparison and no candidate attempt. Every cell of "
                    "this family is exploratory, so the family's confirmatory budget "
                    "is untouched and the replacement attempt reserves at sequence 0 "
                    "on an empty chain."
                ),
            },
            "cells_declared": len(declared),
            "cells_measured": len(measured),
            "cells_measured_ids": measured,
            "cells_unmeasured_ids": [
                cell for cell in declared if cell not in set(measured)
            ],
            "checkpoints_accepted": len(units),
            "execution_log": str(stage / "execution.log"),
            "execution_log_events": dict(sorted(counts.items())),
            "measured_data_location": (
                "None. The stage carries no checkpointed unit and no window, so this "
                "attempt contributes no measurement to any record."
            ),
        },
    }
    Path(args.output).write_text(json.dumps(record, indent=2) + "\n")
    print(
        f"{args.output}: {record['attempt']['cells_measured']} of "
        f"{record['attempt']['cells_declared']} cells measured, "
        f"{len(diagnostics)} child diagnostics"
    )


if __name__ == "__main__":
    main()
