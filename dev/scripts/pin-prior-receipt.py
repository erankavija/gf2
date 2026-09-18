#!/usr/bin/env python3
"""Pin a prior accepted receipt a family states its agreement with.

Usage:
  pin-prior-receipt.py --receipt-dir <dir> --issue <short-id> --role <text>
                       --output <json>

A family whose published outcome states whether its result agrees in direction
with an earlier campaign pins which bytes that statement is about. The pinned
receipt is context, not evidence the family inherits, which is what `--role`
records. Every field written is read from the pinned files at run time; the tool
types none of them, and it refuses a receipt whose acceptance summary evaluates
other bytes.
"""

import argparse
import hashlib
import json
import os
import subprocess


def digest(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    for flag in ("receipt-dir", "issue", "role", "output"):
        parser.add_argument(f"--{flag}", required=True)
    args = parser.parse_args()
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()

    receipt = os.path.join(args.receipt_dir, "receipt.json")
    summary_path = os.path.join(args.receipt_dir, "acceptance-summary.json")
    with open(os.path.join(root, summary_path)) as handle:
        summary = json.load(handle)
    if summary["receipt_sha256"] != digest(os.path.join(root, receipt)):
        raise SystemExit("the acceptance summary evaluates other receipt bytes")
    pin = {
        "role": args.role,
        "issue": args.issue,
        "family_id": summary["family"]["family_id"],
        "campaign_id": summary["campaign_id"],
        "label": summary["label"],
        "verdict": summary["verdict"],
        "receipt": receipt,
        "receipt_sha256": digest(os.path.join(root, receipt)),
        "acceptance_summary": summary_path,
        "acceptance_summary_sha256": digest(os.path.join(root, summary_path)),
        "cells": sorted(cell["cell_id"] for cell in summary["cells"]),
    }
    with open(args.output, "w") as handle:
        json.dump(pin, handle, indent=2)
        handle.write("\n")
    print(f"{args.output}: {pin['campaign_id']} {pin['label']} {pin['verdict']}, "
          f"{len(pin['cells'])} cells")


if __name__ == "__main__":
    main()
