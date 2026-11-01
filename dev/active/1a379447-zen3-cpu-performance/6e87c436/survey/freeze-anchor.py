#!/usr/bin/env python3
"""Freeze the task anchor of the dense-parity outcome by content (jit:6e87c436).

Records the selected source-file digests and source revision as the rebuild pointer.

Usage: freeze-anchor.py COMMIT
"""

import sys

from locate import ANCHOR, PACKAGES, ROOT, repository_files


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    directories = [repository_files.package_directory(ROOT, name) for name in PACKAGES]
    count = ANCHOR.freeze(sys.argv[1], directories, "dense-outcome-anchor-v1", "6e87c436")
    print(f"{ANCHOR.baseline.relative_to(ROOT)}: {count} anchor files")


if __name__ == "__main__":
    main()
