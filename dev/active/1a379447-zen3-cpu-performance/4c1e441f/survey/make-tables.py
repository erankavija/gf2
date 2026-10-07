#!/usr/bin/env python3
"""Generate this family's result tables (jit:4c1e441f).

Usage: make-tables.py [output]
       (default tables.md beside the family launcher)

The rendering is `campaign_tables.py`, shared with every other
lane-comparison family; this file declares this family's stages, ledger, receipt
pin and the paragraph that says what the pinned receipt is cited for.
"""

import os
import sys

import locate
import campaign_tables  # noqa: E402

ISSUE = "4c1e441f"
RESULTS = locate.RESULTS
GENERATOR = os.path.relpath(os.path.abspath(__file__), locate.ROOT)
LEDGER = f"{RESULTS}/dense-product-family-ledger.jsonl"
PIN = f"{locate.ISSUE}/pinned-matrix-confirmation.json"
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
    root = locate.ROOT
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
