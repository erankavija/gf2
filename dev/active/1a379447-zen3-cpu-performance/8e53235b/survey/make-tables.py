#!/usr/bin/env python3
"""Generate the result tables of the codeword-assembly family (jit:8e53235b).

Usage: make-tables.py, with no arguments

Writes `tables.md` beside the family launcher through the shared
`campaign_tables.py`. The stages are the pilot and confirmation receipt
directories the launcher publishes; a stage without a committed receipt is
reported as absent. The ledger is the one the pilot receipt records. The
family pins no earlier campaign, so the tables carry no agreement section.
"""

import json
import os
import sys

import locate
import campaign_tables  # noqa: E402

STAGES = [(label, f"{locate.RESULTS}/r1-codeword-assembly-{label}")
          for label in ("pilot", "confirmation")]


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    root = str(locate.ROOT)
    with open(os.path.join(root, STAGES[0][1], "receipt.json")) as handle:
        ledger = json.load(handle)["trial_ledger"]["path"]
    campaign_tables.main(
        root=root,
        output=os.path.join(root, locate.RESULTS, "tables.md"),
        title=f"BCH codeword-assembly family tables (jit:{locate.ISSUE})",
        generator=f"{locate.SURVEY}/make-tables.py",
        stages=STAGES,
        ledger=ledger,
    )


if __name__ == "__main__":
    main()
