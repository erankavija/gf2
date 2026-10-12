#!/usr/bin/env python3
"""Generate the result tables of the entry-check cost family (jit:bfc2ceab).

Usage: make-tables.py, with no arguments

Writes `tables.md` beside the family ledger through the shared
`campaign_tables.py`. The stages are the committed receipts of the ledger's
campaigns in ledger order, which `locate.family_stages` finds. The family pins
no earlier campaign, so the tables carry no agreement section.
"""

import sys

import review
from locate import HERE, ISSUE, ROOT, family_ledger, family_stages

import campaign_tables  # noqa: E402


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    addendum = HERE.parent / review.PILOT_ADDENDUM
    ledger = family_ledger(addendum)[0]
    campaign_tables.main(
        root=str(ROOT),
        output=str(ledger.parent / review.TABLES),
        title=f"Entry-check cost family tables (jit:{ISSUE})",
        generator=str((HERE / "make-tables.py").relative_to(ROOT)),
        stages=[(label, str(directory.relative_to(ROOT)))
                for label, directory, _ in family_stages(addendum)],
        ledger=str(ledger.relative_to(ROOT)),
    )


if __name__ == "__main__":
    main()
