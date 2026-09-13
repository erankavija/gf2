#!/usr/bin/env python3
"""Render one byte-field receipt as the tables findings.md carries.

Every number in the findings document is produced here from a committed
acceptance summary and receipt, so the prose cannot drift from the evidence
it cites.

Usage: summarize-receipt.py <receipt-dir> [--conversion]
"""

import argparse
import json
import os
import statistics


def median_ns(cell, arm):
    values = [pair[arm]["ns_per_call"] for pair in cell.get("pairs", [])]
    return statistics.median(values) if values else None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("receipt")
    parser.add_argument("--conversion", action="store_true")
    args = parser.parse_args()

    with open(os.path.join(args.receipt, "acceptance-summary.json")) as handle:
        summary = json.load(handle)
    with open(os.path.join(args.receipt, "receipt.json")) as handle:
        receipt = json.load(handle)

    cells = {cell["cell_id"]: cell for cell in receipt["cells"]}
    print(f"verdict: {summary['verdict']}  qualifies: {summary['qualifies']}  "
          f"label: {summary['label']}  sessions: {summary['sessions']}")
    print(f"receipt sha256: {summary['receipt_sha256']}")
    print()
    confidences = {
        entry["interval"]["confidence"]
        for entry in summary["cells"]
        if entry.get("interval")
    }
    # Bonferroni sets one per-comparison level for the whole family, so the
    # column header states the level the receipt actually used.
    level = f"{100.0 * max(confidences):.4g}%" if confidences else "family"
    print(
        f"| Cell | gf2 ns/call | comparator ns/call | ratio of medians | "
        f"{level} interval | outcome |"
    )
    print("|---|---|---|---|---|---|")
    for entry in summary["cells"]:
        cell = cells.get(entry["cell_id"], {})
        baseline = median_ns(cell, "baseline")
        candidate = median_ns(cell, "candidate")
        interval = entry.get("interval")
        if interval is None:
            print(f"| `{entry['cell_id']}` | | | | | {entry['outcome']} "
                  f"({entry.get('reason') or 'no interval'}) |")
            continue
        print(
            f"| `{entry['cell_id']}` | {baseline:,.0f} | {candidate:,.0f} | "
            f"{interval['estimate']:.2f} | "
            f"[{interval['lower']:.2f}, {interval['upper']:.2f}] | "
            f"{entry['outcome']} |"
        )
    print()
    widths = {
        entry["cell_id"]: (entry["interval"]["upper"] - entry["interval"]["lower"])
        / (2.0 * entry["interval"]["estimate"])
        for entry in summary["cells"]
        if entry.get("interval")
    }
    if widths:
        print("relative confidence-interval half-widths:")
        for cell_id, width in sorted(widths.items()):
            print(f"  {cell_id}: {width:.4f}")
        print(f"  maximum: {max(widths.values()):.4f}")

    if args.conversion:
        print()
        print("| Cell | Arm | setup ns | pack ns | unpack ns | table ns |")
        print("|---|---|---|---|---|---|")
        for cell in receipt["cells"]:
            for pair in cell.get("pairs", [])[:1]:
                for arm in ("baseline", "candidate"):
                    conversion = pair[arm].get("conversion")
                    if conversion is None:
                        continue
                    print(
                        f"| `{cell['cell_id']}` | {pair[arm]['arm']} | "
                        f"{conversion['setup_ns']:,} | {conversion['pack_ns']:,} | "
                        f"{conversion['unpack_ns']:,} | {conversion['batch_fill_ns']:,} |"
                    )
        print()
        print("| Cell | Arm | selected path |")
        print("|---|---|---|")
        for cell in receipt["cells"]:
            for pair in cell.get("pairs", [])[:1]:
                for arm in ("baseline", "candidate"):
                    path = pair[arm].get("selected_path")
                    if path:
                        print(f"| `{cell['cell_id']}` | {pair[arm]['arm']} | `{path}` |")


if __name__ == "__main__":
    main()
