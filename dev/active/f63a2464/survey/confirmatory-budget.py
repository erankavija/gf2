#!/usr/bin/env python3
"""Recompute each family's confirmatory cell budget from its own ledger.

The protocol caps a confirmatory attempt's cell count through two rules: the
sequential attempt budget `alpha / (t (t + 1))`, where `t` counts the ledger
entries that reserve a comparison, and P-20's requirement that a cell's
corrected level leave at least twenty expected draws in each bootstrap tail.
Both constants are read from the protocol's source of truth and the attempt
index from the family's append-only ledger, so this records the arithmetic
rather than a remembered number.

Usage (from the worktree root):
  confirmatory-budget.py LEDGER... > dev/bench_results/f63a2464/confirmatory-budget.json
"""

import json
import pathlib
import sys

PROTOCOL = pathlib.Path("dev/tools/tuning-campaign-support/src/protocol.rs")
RECEIPT = pathlib.Path("dev/tools/tuning-campaign-support/src/receipt.rs")
TAIL_RULE = "|| f64::from(settings.bootstrap_resamples) * corrected_alpha / 2.0 < 20.0;"
MIN_TAIL_DRAWS = 20.0


def frozen(name):
    for line in PROTOCOL.read_text(encoding="utf-8").splitlines():
        text = line.strip()
        if text.startswith(f"{name}:") and text.endswith(","):
            return float(text.split(":", 1)[1].strip().rstrip(",").replace("_", ""))
    sys.exit(f"{PROTOCOL} does not carry {name}")


def main():
    ledgers = sys.argv[1:]
    if not ledgers:
        sys.exit("usage: confirmatory-budget.py LEDGER...")
    if TAIL_RULE not in RECEIPT.read_text(encoding="utf-8"):
        sys.exit(f"{RECEIPT} no longer carries the tail-support rule this derivation reads")

    alpha = frozen("family_alpha")
    resamples = frozen("bootstrap_resamples")

    families = []
    for name in ledgers:
        path = pathlib.Path(name)
        entries = [
            json.loads(line)
            for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        spending = sum(1 for entry in entries if entry.get("comparisons", 0) > 0)
        # The next confirmation is attempt `spending + 1`, and `attempt_alpha`
        # floors the count at one, so an untouched chain yields attempt one.
        attempt = max(1, spending + 1)
        attempt_alpha = alpha / (attempt * (attempt + 1))
        admissible = [
            cells
            for cells in range(1, 64)
            if resamples * (attempt_alpha / cells) / 2.0 >= MIN_TAIL_DRAWS
        ]
        families.append({
            "ledger": path.as_posix(),
            "entries": len(entries),
            "reservations_spending_a_comparison": spending,
            "next_attempt_index": attempt,
            "attempt_alpha": attempt_alpha,
            "max_confirmatory_cells": max(admissible) if admissible else 0,
        })

    print(json.dumps({
        "schema": "ldpc-candidate-confirmatory-budget-v1",
        "frozen": {
            "family_alpha": alpha,
            "bootstrap_resamples": resamples,
            "min_tail_draws": MIN_TAIL_DRAWS,
        },
        "families": families,
    }, indent=2))


if __name__ == "__main__":
    main()
