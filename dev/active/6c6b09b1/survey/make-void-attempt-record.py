#!/usr/bin/env python3
"""Record the family-ledger attempts that measured no cell (jit:6c6b09b1).

Usage: make-void-attempt-record.py CAMPAIGN_PREFIX STAGE_DIR OUTPUT

A ledger line carries a reservation, not an outcome: `comparisons` counts the
confirmatory comparisons the addendum predeclared, and a pilot addendum
declares none, so a pilot attempt that measured every cell and one that died
on its first arm write the same line. This record closes that gap for the
attempts that produced nothing, so a reader can tell a void attempt from a
falsified one from the committed artifacts alone.

For each family it reads the ledger, locates the attempt whose campaign
identity starts with CAMPAIGN_PREFIX, and derives its outcome from that
campaign's execution log under STAGE_DIR: the journal's terminal event, the
cells it completed, and the first child diagnostic, which is the only place
the cause survives. Every field is read from an artifact; none is
transcribed.
"""

import hashlib
import json
import os
import sys

LEDGERS = "dev/bench_results/6c6b09b1"
FAMILIES = {
    "region-axpy": "byte-field-region-axpy",
    "matrix-product": "byte-field-matrix-product",
    "pairwise-control": "byte-field-pairwise-control",
}
# Journal events that end a campaign, and what each one means for its data.
TERMINAL = {
    "complete": "the campaign measured every cell of its plan",
    "failed": "the campaign stopped on an error and measured no further cell",
    "interrupted": "the session stopped at its budget and the campaign resumes",
}


def digest(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def journal(stage, campaign):
    """Terminal event, completed cells and first diagnostic of one campaign."""
    path = os.path.join(stage, campaign, "execution.log")
    events = [json.loads(line) for line in open(path)]
    completed = [e["case"]["cell_id"] for e in events if e["event"] == "cell-complete"]
    diagnostics = [e for e in events if e["event"] == "child-diagnostic"]
    last = events[-1]
    return {
        "execution_log_events": len(events),
        "terminal_event": last["event"],
        "terminal_detail": last["details"],
        "cells_completed": completed,
        "first_child_diagnostic": {
            "arm": diagnostics[0]["case"]["arm"],
            "cell_id": diagnostics[0]["case"]["cell_id"],
            "stderr": diagnostics[0]["details"]["stderr"],
        }
        if diagnostics
        else None,
    }


def main():
    if len(sys.argv) != 4:
        raise SystemExit(__doc__.strip())
    prefix, stage, output = sys.argv[1:]
    attempts = []
    for short, family in FAMILIES.items():
        ledger = f"{LEDGERS}/v3-{short}-family-ledger.jsonl"
        entries = [json.loads(line) for line in open(ledger)]
        matching = [e for e in entries if e["campaign"].startswith(prefix)]
        if len(matching) != 1:
            raise SystemExit(f"{ledger}: {len(matching)} attempts match {prefix!r}")
        entry = matching[0]
        addendum = (
            f"dev/active/6c6b09b1/addendum-v{entry['protocol_version']}-{short}-pilot.json"
        )
        if digest(addendum) != entry["addendum_sha256"]:
            raise SystemExit(f"{addendum} is not what {ledger} pins")
        record = journal(stage, entry["campaign"])
        attempts.append(
            {
                "family": family,
                "ledger": ledger,
                "ledger_sequence": entry["sequence"],
                "campaign": entry["campaign"],
                "protocol_version": entry["protocol_version"],
                "addendum": addendum,
                "addendum_sha256": entry["addendum_sha256"],
                "reserved_comparisons": entry["comparisons"],
                "reserved_candidates": entry["candidates"],
                "cells_measured": len(record["cells_completed"]),
                "outcome": TERMINAL.get(record["terminal_event"], "unrecognised"),
                **record,
            }
        )
    void = [a for a in attempts if a["cells_measured"] == 0]
    manifest = {
        "kind": "void-family-ledger-attempts",
        "summary": (
            "Every attempt listed here measured no cell. A reservation spending "
            "zero comparisons and naming no candidate is what a pilot addendum "
            "always reserves, because a pilot declares only exploratory cells, "
            "so the ledger line alone cannot say whether the attempt produced "
            "data. These attempts produced none: they falsify nothing, support "
            "no claim, and consume neither a confirmatory comparison nor any "
            "candidate's confirmatory attempt. The family keeps them because "
            "the ledger chain rejects a removed attempt."
        ),
        "attempts": void,
    }
    with open(output, "w") as handle:
        json.dump(manifest, handle, indent=2)
        handle.write("\n")
    print(f"{output}: {len(void)} void attempts of {len(attempts)} matching")


if __name__ == "__main__":
    main()
