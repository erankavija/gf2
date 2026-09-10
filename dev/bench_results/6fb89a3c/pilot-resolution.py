#!/usr/bin/env python3
"""Derive a family's measurement resolution from its accepted v3 pilot.

Usage: dev/bench_results/6fb89a3c/pilot-resolution.py <pilot-dir>

Protocol v3 (P-03) recomputes, from the pilot receipt's raw pairs at the
pilot's own ledger-derived corrected alpha, the widest relative half-width
max(|s - l|, |u - s|) / s over the pilot's measured cells, and rejects a
declared `effect.measurement_resolution` below it. The acceptance summary
carries exactly those recomputed intervals, so this script reads the summary
(after checking that it accepts the exact committed receipt bytes) and
prints the per-cell half-widths, the widest one, and its two-decimal
round-up, which is the value frozen in the confirmation addendum.
"""

import hashlib
import json
import math
import pathlib
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    pilot = pathlib.Path(sys.argv[1])
    receipt_bytes = (pilot / "receipt.json").read_bytes()
    summary = json.loads((pilot / "acceptance-summary.json").read_text())
    digest = hashlib.sha256(receipt_bytes).hexdigest()
    if summary["receipt_sha256"] != digest or summary["verdict"] != "accepted" or summary["label"] != "pilot":
        raise SystemExit("the acceptance summary must accept the exact committed pilot receipt bytes")
    receipt = json.loads(receipt_bytes)
    print(f"receipt {pilot / 'receipt.json'}")
    print(f"receipt_sha256 {digest}")
    print(f"family {receipt['family_id']} label {summary['label']} verdict {summary['verdict']}")
    print(f"ledger-derived per-comparison confidence {summary['family']['per_comparison_confidence']}")
    print()
    print(f"{'cell_id':<52}{'pairs':>6}{'estimate':>10}{'lower':>10}{'upper':>10}{'rel_half':>10}")
    worst = 0.0
    for cell in summary["cells"]:
        interval = cell.get("interval")
        if interval is None:
            print(f"{cell['cell_id']:<52}{'-':>6}{'-':>10}{'-':>10}{'-':>10}{'-':>10}")
            continue
        estimate = interval["estimate"]
        relative = max(abs(estimate - interval["lower"]), abs(interval["upper"] - estimate)) / estimate
        worst = max(worst, relative)
        print(
            f"{cell['cell_id']:<52}{cell['pairs']:>6}{estimate:>10.4f}{interval['lower']:>10.4f}"
            f"{interval['upper']:>10.4f}{relative:>10.4f}"
        )
    print()
    print(f"largest relative half-width: {worst:.6f}")
    print(f"rounded up to two decimals:  {math.ceil(worst * 100.0 - 1e-9) / 100.0:.2f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
