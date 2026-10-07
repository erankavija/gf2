#!/usr/bin/env python3
"""Generate the result tables of the entry-check cost family (jit:bfc2ceab).

Usage: make-tables.py, with no arguments

Writes `tables.md` beside the family ledger through the shared
`campaign_tables.py`. The stages are those `locate.family_stages` finds from
the frozen confirmation addendum, and the ledger is the one the pilot receipt
records. The family pins no earlier campaign, so the tables carry no agreement
section.
"""

import json
import sys

import review
from locate import HERE, ISSUE, ROOT, family_stages

import campaign_tables  # noqa: E402


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    stages = family_stages(HERE.parent / review.CONFIRMATION_ADDENDUM)
    ledger = json.loads((stages["pilot"] / "receipt.json").read_text())["trial_ledger"]["path"]
    campaign_tables.main(
        root=str(ROOT),
        output=str((ROOT / ledger).parent / review.TABLES),
        title=f"Entry-check cost family tables (jit:{ISSUE})",
        generator=str((HERE / "make-tables.py").relative_to(ROOT)),
        stages=[(label, str(directory.relative_to(ROOT))) for label, directory in stages.items()],
        ledger=ledger,
    )


if __name__ == "__main__":
    main()
