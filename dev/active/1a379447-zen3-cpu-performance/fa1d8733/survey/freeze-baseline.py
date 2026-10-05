#!/usr/bin/env python3
"""Freeze one tree of the unroll-body removal by content (jit:fa1d8733).

Writes `<stage>-baseline.json` beside itself: the SHA-256 of every file the
kernel package holds at COMMIT. For each path whose working-tree bytes differ,
the baseline bytes are snapshotted under `inputs/<stage>/`. The generators read
these records and their snapshots only. `locate.py` names the stages.

Usage: freeze-baseline.py STAGE COMMIT
"""

import sys

from locate import BASELINES, ISSUE, PACKAGE, ROOT


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in BASELINES:
        raise SystemExit(__doc__)
    baseline = BASELINES[sys.argv[1]]
    count = baseline.freeze(sys.argv[2], [PACKAGE], "unroll-removal-baseline-v1", ISSUE)
    print(f"{baseline.baseline.relative_to(ROOT)}: {count} files")


if __name__ == "__main__":
    main()
