#!/usr/bin/env python3
"""Write the origin record of each protocol-v3 family ledger (jit:6c6b09b1).

Usage: dev/active/6c6b09b1/survey/make-ledger-origin-v3.py   (from the repo root)

Each v3 family ledger of this survey opens with one retrospective line,
written before the family's first v3 campaign by
`survey-analysis ledger-genesis` (survey/analysis/src/main.rs): it restates
the completed protocol-v1 exploratory pilot, which pooled every operation of
this survey in one family, as a zero-comparison reservation. This script
records where that line comes from, and the two earlier v1 pilot launches
that stopped before the runner opened a campaign and so left no ledger line.
Every digest below is computed from the committed bytes it names; the script
refuses to write a record whose ledger does not open with exactly that line.
"""

import hashlib
import json
import os
import subprocess

ISSUE = "6c6b09b1"
RESULTS = f"dev/bench_results/{ISSUE}"
V1_PILOT = f"{RESULTS}/2026-09-08-{ISSUE}-byte-field-pilot"
HISTORY = f"{RESULTS}/v1-launch-history"
FAMILIES = {
    "region-axpy": "byte-field-region-axpy",
    "matrix-product": "byte-field-matrix-product",
    "pairwise-control": "byte-field-pairwise-control",
}
INTERRUPTED = [
    "gf2-pilot-6c6b09b1-20260907t195857z",
    "gf2-pilot-6c6b09b1-20260908t090803z",
]


def sha256(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def main():
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], check=True,
                          capture_output=True, text=True).stdout.strip()
    os.chdir(root)
    with open(f"{V1_PILOT}/receipt.json") as handle:
        receipt = json.load(handle)
    with open(receipt["addendum"]["snapshot"] if os.path.isabs(receipt["addendum"]["snapshot"])
              else f"{V1_PILOT}/{receipt['addendum']['snapshot']}") as handle:
        v1_addendum = json.load(handle)
    non_exploratory = sum(1 for cell in v1_addendum["cells"] if cell["role"] != "exploratory")
    for short, family in FAMILIES.items():
        ledger = f"{RESULTS}/v3-{short}-family-ledger.jsonl"
        with open(ledger, "rb") as handle:
            first = handle.read().split(b"\n", 1)[0] + b"\n"
        line = json.loads(first)
        expected = {
            "sequence": 0,
            "predecessor": "0" * 64,
            "family": family,
            "campaign": receipt["campaign_id"],
            "addendum_sha256": receipt["addendum"]["sha256"],
            "comparisons": non_exploratory,
            "protocol_version": v1_addendum["protocol"]["version"],
            "candidates": [],
        }
        if line != expected:
            raise SystemExit(f"{ledger} does not open with the v1 pilot restatement")
        record = {
            "kind": "retrospective-v1-exploratory-history",
            "family": family,
            "ledger": ledger,
            "genesis_sha256": hashlib.sha256(first).hexdigest(),
            "derivation": {
                "implementation": f"dev/active/{ISSUE}/survey/analysis/src/main.rs",
                "method": (
                    "ledger-genesis restates the completed protocol-v1 pilot as the "
                    "reservation the v3 runner would have appended before measuring it: its "
                    "campaign ID and addendum digest from the receipt, its protocol version and "
                    "its count of non-exploratory cells from the receipt's addendum snapshot, "
                    "no candidate identities, encoded with the shared "
                    "tuning_campaign_support::trial_ledger::Reservation and decoded by "
                    "trial_ledger::decode before it is written. The family field names the "
                    "canonical v3 question; the source keeps its original v1 family."
                ),
                "retrospective": (
                    "This line is accounting, not a premeasurement reservation: protocol v1 "
                    "took none, and no lock acquisition is claimed for it. The v1 pilot "
                    "measured one cell for each of this survey's three operations, so the same "
                    "line opens all three family ledgers; it spends zero comparisons."
                ),
                "command": (f"survey-analysis ledger-genesis {family} {ledger} {V1_PILOT}"),
            },
            "sources": [
                {
                    "sequence": 0,
                    "campaign": receipt["campaign_id"],
                    "original_family": v1_addendum["family"]["id"],
                    "receipt": f"{V1_PILOT}/receipt.json",
                    "receipt_sha256": sha256(f"{V1_PILOT}/receipt.json"),
                    "plan": f"{V1_PILOT}/plan.json",
                    "plan_sha256": sha256(f"{V1_PILOT}/plan.json"),
                    "addendum": f"{V1_PILOT}/{receipt['addendum']['snapshot']}",
                    "addendum_sha256": receipt["addendum"]["sha256"],
                    "acceptance_summary": f"{V1_PILOT}/acceptance-summary.json",
                    "acceptance_summary_sha256": sha256(f"{V1_PILOT}/acceptance-summary.json"),
                    "non_exploratory_cells": non_exploratory,
                    "disposition": "completed v1 pilot; exploratory, zero comparisons",
                }
            ],
            "interrupted_launches_without_line": [
                {
                    "campaign": name.removeprefix("gf2-"),
                    "evidence": [
                        {"path": f"{HISTORY}/{name}{suffix}",
                         "sha256": sha256(f"{HISTORY}/{name}{suffix}")}
                        for suffix in (".launcher.log", ".plan.json")
                    ],
                    "disposition": (
                        "launcher stopped before the runner opened a stage or execution log; "
                        "no campaign, no samples and no reservation exist"
                    ),
                }
                for name in INTERRUPTED
            ],
            "unrun_v1_confirmation": {
                "addendum": f"dev/active/{ISSUE}/addendum-byte-field-arms.json",
                "addendum_sha256": sha256(f"dev/active/{ISSUE}/addendum-byte-field-arms.json"),
                "disposition": (
                    "frozen under protocol v1 and never launched: no stage, execution log, "
                    "receipt or reservation exists, so it spends no comparisons"
                ),
            },
        }
        path = f"{RESULTS}/v3-{short}-ledger-origin.json"
        with open(path, "w") as handle:
            json.dump(record, handle, indent=2)
            handle.write("\n")
        print(f"{path}: genesis {record['genesis_sha256']}")


if __name__ == "__main__":
    main()
