#!/usr/bin/env python3
"""Derives a family's measurement resolution from its accepted v3 pilot.

Reads the pilot's acceptance summary, prints every cell's estimate, interval
and relative half-width at the corrected per-comparison confidence the
acceptance tool used, and reports the widest half-width rounded up to two
decimals. The confirmatory addendum freezes that rounded value as
`effect.measurement_resolution`; P-03 recomputes the same quantity from the raw
pairs and rejects a declared value below it.

Usage: pilot-resolution.py <pilot-receipt-dir>
"""

import hashlib
import json
import math
import pathlib
import sys


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    receipt_dir = pathlib.Path(sys.argv[1])
    summary = json.loads((receipt_dir / "acceptance-summary.json").read_text(encoding="utf-8"))
    receipt_sha = hashlib.sha256((receipt_dir / "receipt.json").read_bytes()).hexdigest()
    if summary["receipt_sha256"] != receipt_sha:
        sys.exit("acceptance summary does not describe this receipt")

    print(f"receipt {receipt_dir / 'receipt.json'}")
    print(f"receipt_sha256 {receipt_sha}")
    print(f"label {summary['label']} verdict {summary['verdict']} findings {len(summary['findings'])}")
    family = summary["family"]
    print(
        f"family {family['family_id']} comparisons {family['comparisons']} "
        f"family_alpha {family['family_alpha']} "
        f"per-comparison confidence {family['per_comparison_confidence']}"
    )
    print()
    print(f"{'cell_id':<52}{'estimate':>10}{'lower':>10}{'upper':>10}{'rel_half':>10}")
    widest = 0.0
    for cell in summary["cells"]:
        interval = cell.get("interval")
        if interval is None:
            print(f"{cell['cell_id']:<52}{'unavailable':>40}")
            continue
        estimate = interval["estimate"]
        lower = interval["lower"]
        upper = interval["upper"]
        rel_half = max(estimate - lower, upper - estimate) / estimate
        widest = max(widest, rel_half)
        print(f"{cell['cell_id']:<52}{estimate:>10.4f}{lower:>10.4f}{upper:>10.4f}{rel_half:>10.4f}")
    print()
    print(f"largest relative half-width: {widest:.6f}")
    print(f"rounded up to two decimals:  {math.ceil(widest * 100 - 1e-9) / 100:.2f}")


if __name__ == "__main__":
    main()
