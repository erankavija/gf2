#!/usr/bin/env python3
"""Generate this family's result tables (jit:4c1e441f).

Usage: make-tables.py [output]
       (default dev/bench_results/4c1e441f/tables.md)

The rendering is `dev/scripts/campaign_tables.py`, shared with every other
lane-comparison family; this file declares this family's stages, ledger, receipt
pin and the paragraph that says what the pinned receipt is cited for.
"""

import os
import subprocess
import sys

sys.path.insert(0, os.path.join(subprocess.run(
    ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
).stdout.strip(), "dev/scripts"))
import campaign_tables  # noqa: E402

ISSUE = "4c1e441f"
RESULTS = f"dev/bench_results/{ISSUE}"
GENERATOR = f"dev/active/{ISSUE}/survey/make-tables.py"
LEDGER = f"{RESULTS}/dense-product-family-ledger.jsonl"
PIN = f"dev/active/{ISSUE}/pinned-matrix-confirmation.json"
STAGES = [
    ("pilot", f"{RESULTS}/r1-dense-product-pilot"),
    ("confirmation", f"{RESULTS}/r1-dense-product-confirmation"),
]

AGREEMENT_NOTE = [
    "Agreement is on the direction alone. That campaign measured a prototype that rebuilt its",
    "table per product, converted its operands and did not write results in place; the shipped",
    "path caches the table for the life of the process and writes in place through the consumer's",
    "own hook, so the sizes need not match.",
]


def main():
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, RESULTS, "tables.md")
    campaign_tables.main(
        root=root,
        output=output,
        title=f"GF(2^8) dense product confirmation tables (jit:{ISSUE})",
        generator=GENERATOR,
        stages=STAGES,
        ledger=LEDGER,
        pin_path=PIN,
        agreement_heading="Direction agreement with the pinned matrix-family confirmation",
        agreement_note=AGREEMENT_NOTE,
    )


if __name__ == "__main__":
    main()
