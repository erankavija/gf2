#!/usr/bin/env python3
"""Freeze one baseline of the safety-contract change by content (jit:7d44b71f).

Records the selected source-file digests and source revision as the rebuild pointer.

Usage: freeze-anchor.py STAGE COMMIT
"""

import sys

from locate import BASELINES, ISSUE, PACKAGE, ROOT


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in BASELINES:
        raise SystemExit(__doc__)
    baseline = BASELINES[sys.argv[1]]
    count = baseline.freeze(sys.argv[2], [PACKAGE], "safety-contract-anchor-v1", ISSUE)
    print(f"{baseline.baseline.relative_to(ROOT)}: {count} files")


if __name__ == "__main__":
    main()
