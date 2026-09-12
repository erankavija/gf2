#!/usr/bin/env python3
"""Derive the family measurement resolution from a pilot acceptance summary.

Usage: dev/bench_results/eda07788/pilot-resolution.py <acceptance-summary.json>

Protocol v3 defines the resolution of a percentile-bootstrap interval [l, u]
around an estimate s as max(abs(s - l), abs(u - s)) / s. The asymmetric,
conservative half-width prevents a skewed interval from understating what the
pilot can distinguish. The family resolution is the largest such value over
the pilot's measured cells.

This is the construction the protocol smoke used for `f547c394`; it is scripted
here so the number frozen in the confirmatory addendum is reproducible from the
committed pilot receipt rather than transcribed by hand.
"""

import json
import math
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    with open(sys.argv[1]) as handle:
        summary = json.load(handle)
    print(f"receipt_sha256 {summary['receipt_sha256']}")
    print(f"label {summary['label']} verdict {summary['verdict']}")
    print(f"per-comparison confidence {summary['family']['per_comparison_confidence']}")
    print()
    print(f"{'cell_id':<56}{'estimate':>10}{'lower':>10}{'upper':>10}{'rel_half':>10}")
    worst = 0.0
    for cell in summary["cells"]:
        interval = cell.get("interval")
        if interval is None:
            print(f"{cell['cell_id']:<56}{'-':>10}{'-':>10}{'-':>10}{'-':>10}")
            continue
        estimate = interval["estimate"]
        relative = max(
            abs(estimate - interval["lower"]),
            abs(interval["upper"] - estimate),
        ) / estimate
        worst = max(worst, relative)
        print(
            f"{cell['cell_id']:<56}{estimate:>10.4f}{interval['lower']:>10.4f}"
            f"{interval['upper']:>10.4f}{relative:>10.4f}"
        )
    print()
    print(f"largest relative half-width: {worst:.6f}")
    print(f"rounded up to two decimals:  {math.ceil(worst * 100.0) / 100.0:.2f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
