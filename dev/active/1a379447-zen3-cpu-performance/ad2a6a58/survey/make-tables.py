#!/usr/bin/env python3
"""Generate this family's result tables (jit:ad2a6a58).

Usage: make-tables.py [output]
       (default tables.md beside the launcher)

The rendering is the shared `campaign_tables.py`, which `locate.py` beside this
file puts on the import path; this file declares this family's stages, ledger,
receipt pin and the paragraph that says what the pinned receipt is cited for.
"""

import os
import sys

import locate
import campaign_tables  # noqa: E402

ISSUE = "ad2a6a58"
RESULTS = locate.RESULTS
GENERATOR = os.path.relpath(os.path.abspath(__file__), locate.ROOT)
LEDGER = f"{RESULTS}/axpy-family-ledger.jsonl"
PIN = f"{locate.ISSUE}/pinned-vector-confirmation.json"
STAGES = [
    ("pilot", f"{RESULTS}/r1-axpy-pilot"),
    ("confirmation", f"{RESULTS}/r1-axpy-confirmation"),
]

AGREEMENT_NOTE = [
    "Agreement is on the direction alone. That campaign measured a prototype that converted",
    "its operands and did not write results in place; the shipped path writes in place",
    "through the consumer's own hook, so the sizes need not match.",
]


def main():
    root = locate.ROOT
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, RESULTS, "tables.md")
    campaign_tables.main(
        root=root,
        output=output,
        title=f"GF(2^8) axpy lane confirmation tables (jit:{ISSUE})",
        generator=GENERATOR,
        stages=STAGES,
        ledger=LEDGER,
        pin_path=PIN,
        agreement_heading="Direction agreement with the pinned vector-family confirmation",
        agreement_note=AGREEMENT_NOTE,
    )


if __name__ == "__main__":
    main()
