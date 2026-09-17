#!/usr/bin/env python3
"""Pin the prior vector-family confirmation this family agrees with (jit:ad2a6a58).

Usage: pin-prior-receipt.py [output]
       (default dev/active/ad2a6a58/pinned-vector-confirmation.json)

The published outcome states whether this family's result agrees in direction
with the vector-family confirmation of issue 19513245. That receipt is context,
not evidence this family inherits: it measured a prototype that converted its
operands and did not write results in place, while the shipped path writes in
place through the consumer's own hook, so the two need not agree in size. The
pin fixes which bytes the agreement statement is about. Every field below is
read from the pinned files at run time; nothing is typed here.
"""

import hashlib
import json
import os
import subprocess
import sys

RECEIPT = "dev/bench_results/19513245/r1-vector-confirmation/receipt.json"
SUMMARY = "dev/bench_results/19513245/r1-vector-confirmation/acceptance-summary.json"


def digest(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def main():
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        root, "dev/active/ad2a6a58/pinned-vector-confirmation.json")
    receipt_path = os.path.join(root, RECEIPT)
    summary_path = os.path.join(root, SUMMARY)
    with open(summary_path) as handle:
        summary = json.load(handle)
    if summary["receipt_sha256"] != digest(receipt_path):
        raise SystemExit("the acceptance summary evaluates other receipt bytes")
    pin = {
        "role": "direction agreement only; this family inherits no sample from it",
        "issue": "19513245",
        "family_id": summary["family"]["family_id"],
        "campaign_id": summary["campaign_id"],
        "label": summary["label"],
        "verdict": summary["verdict"],
        "receipt": RECEIPT,
        "receipt_sha256": digest(receipt_path),
        "acceptance_summary": SUMMARY,
        "acceptance_summary_sha256": digest(summary_path),
        "cells": sorted(cell["cell_id"] for cell in summary["cells"]),
    }
    with open(output, "w") as handle:
        json.dump(pin, handle, indent=2)
        handle.write("\n")
    print(f"{output}: {pin['campaign_id']} {pin['label']} {pin['verdict']}, "
          f"{len(pin['cells'])} cells")


if __name__ == "__main__":
    main()
