#!/usr/bin/env python3
"""Freeze the task anchor of the safety-contract change by content (jit:7d44b71f).

Writes `anchor-baseline.json` beside itself: the SHA-256 of every file the
kernel package holds at COMMIT, the commit whose listings
`regen-asm-listings.py` regenerated from the unedited sources. For each path
whose working-tree bytes differ, the anchor bytes are snapshotted under
`inputs/anchor/`. The generators read this record and its snapshots only.

Usage: freeze-anchor.py COMMIT
"""

import sys

from locate import ANCHOR, ISSUE, PACKAGE, ROOT


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    count = ANCHOR.freeze(sys.argv[1], [PACKAGE], "safety-contract-anchor-v1", ISSUE)
    print(f"{ANCHOR.baseline.relative_to(ROOT)}: {count} anchor files")


if __name__ == "__main__":
    main()
