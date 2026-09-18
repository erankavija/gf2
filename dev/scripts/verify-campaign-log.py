#!/usr/bin/env python3
"""Verify a finished campaign from its own execution log.

Usage: verify-campaign-log.py --log execution.log --receipt receipt.json
                              --plan plan.json
       verify-campaign-log.py --log execution.log --stage-complete

`--stage-complete` answers the one question a launcher's session loop asks — has
this stage reached its terminal `complete` record — and exits non-zero while it
has not, including when the log does not exist yet.

A campaign whose every cell failed can still exit cleanly, so a launcher judges
its campaign from the journal rather than from an exit code. Every declared cell
must start once, be checkpointed once and complete once with status `measured` at
its declared pair count, and no undeclared cell may complete. A restarted attempt
is abandoned exactly once by a later session (protocol v4), so a cell's start
count exceeds one only by the abandonments the journal records.

A campaign is a resumable sequence of bounded sessions, so its journal carries
one session-terminal record per session: `complete` once and last, `failed`
never, and `paused`, `budget-exhausted` or `interrupted` for each session that
resumed. A launcher that demanded a single `complete` record would reject a
campaign that paused at its session cell budget and then finished.

The declared pair count comes from the plan: an exploratory cell names its own
count and a confirmatory one takes the protocol's frozen confirmatory pair count,
which the receipt carries in its settings. Nothing here is a protocol check; the
acceptance tool recomputes the verdict.
"""

import argparse
import collections
import json
import os

# Session-terminal journal events: one closes each session.
TERMINAL = ("complete", "failed", "paused", "budget-exhausted", "interrupted")


def verify(log_path, receipt_path, plan_path):
    """Returns the one-line conclusion, or raises SystemExit naming the problems."""
    with open(log_path) as handle:
        events = [json.loads(line) for line in handle]
    with open(receipt_path) as handle:
        receipt = json.load(handle)
    with open(plan_path) as handle:
        plan = json.load(handle)

    confirmatory = receipt["settings"]["confirmatory_pairs"]
    declared = {cell["cell_id"]: cell["pilot_pairs"] or confirmatory for cell in plan["cells"]}

    terminal = [event["event"] for event in events if event["event"] in TERMINAL]
    if terminal[-1:] != ["complete"] or terminal.count("complete") != 1:
        raise SystemExit(
            f"the campaign's session-terminal records are {terminal} rather than resumable "
            "sessions closed by one final 'complete'")
    if "failed" in terminal:
        raise SystemExit("a failed session ends its campaign; the journal records one")

    def cells_of(name):
        return [event for event in events
                if event["event"] == name and event.get("case") is not None]

    starts = collections.Counter(event["case"]["cell_id"] for event in cells_of("cell-start"))
    checkpoints = collections.Counter(event["case"]["cell_id"]
                                      for event in cells_of("checkpoint-accepted"))
    completes = {event["case"]["cell_id"]: event["details"] for event in cells_of("cell-complete")}
    abandoned = collections.Counter(event["case"]["cell_id"]
                                    for event in cells_of("cell-abandoned"))

    problems = []
    for cell_id, pairs in sorted(declared.items()):
        if starts[cell_id] != 1 + abandoned[cell_id]:
            problems.append(f"{cell_id}: {starts[cell_id]} starts, {abandoned[cell_id]} abandoned")
        if checkpoints[cell_id] != 1:
            problems.append(f"{cell_id}: {checkpoints[cell_id]} checkpoints accepted")
        detail = completes.get(cell_id)
        if detail is None:
            problems.append(f"{cell_id}: never completed")
            continue
        if detail.get("status") != "measured":
            problems.append(f"{cell_id}: completed with status {detail.get('status')!r}")
        if detail.get("pairs") != pairs:
            problems.append(f"{cell_id}: {detail.get('pairs')} pairs, {pairs} declared")
    extra = sorted(set(completes) - set(declared))
    if extra:
        problems.append(f"the log completes undeclared cells: {', '.join(extra)}")
    if problems:
        raise SystemExit("execution log: " + "; ".join(problems))
    return (f"execution log: {len(declared)} declared cells, each started once, checkpointed once "
            f"and completed once at its declared pairs, terminal record 'complete'")


def stage_complete(log_path):
    """Whether the stage has reached its terminal `complete` record."""
    if not os.path.isfile(log_path):
        return False
    with open(log_path) as handle:
        terminal = [json.loads(line)["event"] for line in handle
                    if json.loads(line)["event"] in TERMINAL]
    return terminal[-1:] == ["complete"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", required=True)
    parser.add_argument("--receipt")
    parser.add_argument("--plan")
    parser.add_argument("--stage-complete", action="store_true")
    args = parser.parse_args()
    if args.stage_complete:
        raise SystemExit(0 if stage_complete(args.log) else 1)
    if not (args.receipt and args.plan):
        raise SystemExit("a full verification needs --receipt and --plan")
    print(verify(args.log, args.receipt, args.plan))


if __name__ == "__main__":
    main()
