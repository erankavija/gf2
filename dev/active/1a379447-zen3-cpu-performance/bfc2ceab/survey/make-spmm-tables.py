#!/usr/bin/env python3
"""Generate the result tables of the sparse product check-cost family (jit:bfc2ceab).

Usage: make-spmm-tables.py, with no arguments

Writes the family's tables beside its ledger through the shared
`campaign_tables.py`. The stages are the committed receipts of the ledger's
campaigns in ledger order, which `spmm_family.stages` finds. The family pins no
earlier campaign, so the tables carry no agreement section.
"""

import sys

import review
import spmm_family
from locate import HERE, ISSUE, ROOT

import campaign_tables  # noqa: E402


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    ledger = spmm_family.ledger()[0]
    campaign_tables.main(
        root=str(ROOT),
        output=str(ledger.parent / review.GRAIN_FAMILY_TABLES),
        title=f"Sparse product check-cost family tables (jit:{ISSUE})",
        generator=str((HERE / "make-spmm-tables.py").relative_to(ROOT)),
        stages=[(label, str(directory.relative_to(ROOT)))
                for label, directory, _ in spmm_family.stages()],
        ledger=str(ledger.relative_to(ROOT)),
    )


if __name__ == "__main__":
    main()
