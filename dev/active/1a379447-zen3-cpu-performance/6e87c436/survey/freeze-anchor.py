#!/usr/bin/env python3
"""Freeze the task anchor of the dense-parity outcome by content (jit:6e87c436).

Writes `anchor-baseline.json` beside itself: the SHA-256 of every file the
measured production packages hold at COMMIT. For each path whose working-tree
bytes differ, the anchor bytes are snapshotted under `inputs/anchor/`, as a
receipt snapshots its producing inputs. The generators read this record and
its snapshots only; the commit id is kept as an informational field.

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
