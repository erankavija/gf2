#!/usr/bin/env python3
"""Generate the lane-threshold family's result tables (jit:4337c02e).

Usage: make-tables.py [output]
       (default tables.md beside the addendum document)

The rendering is `campaign_tables.py`, shared with every other
lane-comparison family; this file declares this family's stages, ledger, receipt
pin and the paragraph that says what the pinned receipt is cited for.
"""

import json
import sys

sys.dont_write_bytecode = True

from locate import CONFIRMATION, HERE, PILOT, ROOT, repo_artifacts  # noqa: E402

import campaign_tables  # noqa: E402

PIN = HERE.parent / "pilot-receipt-pin.json"

AGREEMENT_NOTE = [
    "The pinned receipt is this family's own pilot, whose cells are exploratory. Agreement is on",
    "the direction of each estimate alone: a speedup above one favours the scalar lane in both",
    "stages. The confirmation measures fresh pairs and inherits no pilot sample.",
]


def main():
    addendum = json.loads((ROOT / repo_artifacts.addendum(CONFIRMATION)).read_bytes())
    output = sys.argv[1] if len(sys.argv) > 1 else HERE.parent / "tables.md"
    campaign_tables.main(
        root=str(ROOT),
        output=str(output),
        title="Matvec lane-threshold tables (jit:4337c02e)",
        generator=str((HERE / "make-tables.py").relative_to(ROOT)),
        stages=[
            ("pilot", str(repo_artifacts.receipt(PILOT))),
            ("confirmation", str(repo_artifacts.receipt(CONFIRMATION))),
        ],
        ledger=addendum["family_wise"]["ledger_path"],
        pin_path=str(PIN.relative_to(ROOT)),
        agreement_heading="Direction agreement of the confirmation with the pilot",
        agreement_note=AGREEMENT_NOTE,
    )


if __name__ == "__main__":
    main()
