#!/usr/bin/env python3
"""Derive the eBCH(128,64) OSD campaign comparison against the pinned reference.

Reads:
  - ``ebch_osd_awgn.json``  (schema-2 campaign receipt beside this script)
  - ``dev/reference_data/osd_ebch_128_64_fossorier1994.csv``  (digitized series)

Produces, beside this script:
  - ``comparison.json``        (the machine-readable comparison, with the
    acceptance verdict for every order-2 reproduction target)
  - ``order2_ber_comparison``  (order-2 BER vs published order-2 series)
  - ``order1_controls``        (order-1 internal-control BER/BLER series
    beside the published order-1 BER as a cross-check)

Each figure is written as both PNG and SVG. ``comparison.json`` is the single
derivation point for the campaign's provenance record: the prose tables are
projections of it rather than independent transcriptions.

Every plotted simulation value is a projection of the receipt: BER point
estimates with their ``block_ratio_product_interval`` endpoints, BLER point
estimates with their ``negative_binomial_clopper_pearson`` endpoints.
Published points carry their recorded digitization uncertainty (log10
decades) as multiplicative error bars. The script draws only completed cells;
the receipt records intervals for completed cells alone.

Usage (from repo root)::

    python3 dev/simulation_results/osd-ebch-128-64/plot.py

Optional flags override the input receipt, reference CSV, and output
directory. Non-interactive; safe for headless environments.
"""

import argparse
import csv
import json
import math
import re
import sys
from pathlib import Path

try:
    import matplotlib

    matplotlib.use("Agg")
    # A fixed hash salt makes SVG element identifiers reproducible; without it
    # matplotlib salts them per process and no two runs agree byte for byte.
    matplotlib.rcParams["svg.hashsalt"] = "osd-ebch-128-64"
    import matplotlib.pyplot as plt
except ImportError:
    print("Error: matplotlib is required. Install with: pip install matplotlib", file=sys.stderr)
    sys.exit(1)

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--receipt", type=Path, default=HERE / "ebch_osd_awgn.json")
    parser.add_argument(
        "--reference-csv",
        type=Path,
        default=REPO / "dev/reference_data/osd_ebch_128_64_fossorier1994.csv",
    )
    parser.add_argument("--output-dir", type=Path, default=HERE)
    return parser.parse_args()


def completed_cells(receipt):
    """Latest completed result per cell id, keyed by (order, eb_n0_db)."""
    cells = {}
    for result in receipt["cell_results"]:
        if result["termination"].get("state") != "completed":
            continue
        cell = result["cell"]
        cells[(cell["osd_order"], cell["eb_n0_db"])] = result
    return cells


def series(cells, order):
    rows = sorted((k[1], v) for k, v in cells.items() if k[0] == order)
    ebn0 = [e for e, _ in rows]
    ber = [v["ber"] for _, v in rows]
    ber_lo = [v["ber_confidence_interval"]["lower"] for _, v in rows]
    ber_hi = [v["ber_confidence_interval"]["upper"] for _, v in rows]
    bler = [v["bler"] for _, v in rows]
    bler_lo = [v["bler_confidence_interval"]["lower"] for _, v in rows]
    bler_hi = [v["bler_confidence_interval"]["upper"] for _, v in rows]
    return ebn0, ber, ber_lo, ber_hi, bler, bler_lo, bler_hi


def published_series(csv_path, osd_order):
    """Returns the published series and the source locators it was read from.

    The locators are collected so the legend names every source actually
    plotted; this series mixes printed-table transcriptions with
    axis-calibrated figure reads.
    """
    ebn0, value, lo, hi = [], [], [], []
    sources = set()
    with open(csv_path, newline="") as handle:
        for row in csv.DictReader(handle):
            if row["metric"] != "BER" or int(row["osd_order"]) != osd_order:
                continue
            v = float(row["value"])
            delta = float(row["digitization_uncertainty_log10"])
            ebn0.append(float(row["eb_n0_db"]))
            value.append(v)
            lo.append(v * (1 - 10.0**-delta) if delta else 0.0)
            hi.append(v * (10.0**delta - 1) if delta else 0.0)
            match = re.search(r"(Table|Fig\.)\s*[\d.]+", row["source_locator"])
            if match:
                sources.add(match.group(0))
    return ebn0, value, lo, hi, sorted(sources)


def published_rows(csv_path):
    """Published BER rows keyed by (osd_order, eb_n0_db)."""
    rows = {}
    with open(csv_path, newline="") as handle:
        for row in csv.DictReader(handle):
            if row["metric"] != "BER":
                continue
            rows[(int(row["osd_order"]), float(row["eb_n0_db"]))] = row
    return rows


def comparison(cells, csv_path, receipt):
    """Derives the acceptance comparison this campaign's record is read from.

    One machine-readable artifact carries every compared quantity, so the
    receipt, the reference dataset, and the prose record have a single
    derivation point rather than three independent transcriptions. Order-1
    cells are emitted as controls and carry no verdict, matching the issue's
    contract that only order-2 points are reproduction targets.
    """
    published = published_rows(csv_path)
    entries = []
    for (order, ebn0), result in sorted(cells.items(), key=lambda kv: (kv[0][0], kv[0][1])):
        interval = result["ber_confidence_interval"]
        lower, upper = interval["lower"], interval["upper"]
        entry = {
            "cell_id": result["cell"]["id"],
            "osd_order": order,
            "eb_n0_db": ebn0,
            "role": "reproduction_target" if order == 2 else "internal_control",
            "samples": result["samples"],
            "block_errors": result["block_errors"],
            "ber": result["ber"],
            "ber_interval": {
                "lower": lower,
                "upper": upper,
                "estimator": interval["estimator"],
                "level": interval["level"],
            },
        }
        row = published.get((order, ebn0))
        if row is not None:
            value = float(row["value"])
            delta = float(row["digitization_uncertainty_log10"])
            window = {"lower": lower * 10.0**-delta, "upper": upper * 10.0**delta}
            entry["published"] = {
                "value": value,
                "digitization_precision_log10_decades": delta,
                "value_kind": row["value_kind"],
                "ordinate_source": row["ordinate_source"],
                "source_locator": row["source_locator"],
            }
            entry["accept_window"] = window
            entry["log10_gap"] = math.log10(result["ber"] / value)
            # Only order-2 points are reproduction targets; the predicate is
            # not evaluated for controls.
            if order == 2:
                entry["accepts"] = window["lower"] <= value <= window["upper"]
        entries.append(entry)
    return {
        "schema_version": receipt["schema_version"],
        "campaign_seed": receipt["campaign_seed"],
        "target_block_errors": receipt["target_block_errors"],
        "predicate": "p in [L * 10**-delta, U * 10**+delta]",
        "cells": entries,
    }


def error_bars(points, lower, upper):
    return [
        [p - l for p, l in zip(points, lower)],
        [u - p for p, u in zip(points, upper)],
    ]


def save(fig, output_dir, stem):
    """Writes the figure as PNG and SVG, and returns the paths written.

    The SVG carries no creation date, so a regeneration from an unchanged
    receipt reproduces both files byte for byte; the SVG is the form the
    issue tracker accepts as a linked document.
    """
    written = []
    for suffix, metadata in ((".png", None), (".svg", {"Date": None})):
        path = output_dir / f"{stem}{suffix}"
        fig.savefig(path, dpi=150, metadata=metadata)
        written.append(path)
    return written


def style(ax, title):
    ax.set_yscale("log")
    ax.set_xlabel(r"$E_b/N_0$ (dB)")
    ax.grid(True, which="both", alpha=0.3)
    ax.set_title(title)
    ax.legend()


def main():
    args = parse_args()
    receipt = json.loads(args.receipt.read_text())
    cells = completed_cells(receipt)
    seed = receipt["campaign_seed"]
    schema = receipt["schema_version"]

    # Order-2 comparison figure.
    ebn0, ber, lo, hi, _, _, _ = series(cells, 2)
    p_ebn0, p_val, p_lo, p_hi, p_sources = published_series(args.reference_csv, 2)
    fig, ax = plt.subplots(figsize=(7, 5))
    ax.errorbar(
        ebn0, ber, yerr=error_bars(ber, lo, hi), fmt="o-", capsize=3,
        label=f"simulation (schema {schema}, seed {seed:#x}, 95% product interval)",
    )
    ax.errorbar(
        p_ebn0, p_val, yerr=[p_lo, p_hi], fmt="s--", capsize=3,
        label=f"Fossorier1994 {', '.join(p_sources)}" + r" ($\pm\delta$ decades)",
    )
    ax.set_ylabel("BER")
    style(ax, "eBCH(128,64) OSD order-2: simulation vs published BER")
    fig.tight_layout()
    for out in save(fig, args.output_dir, "order2_ber_comparison"):
        print(f"wrote {out}")

    # Order-1 internal-control figure.
    ebn0, ber, lo, hi, bler, blo, bhi = series(cells, 1)
    c_ebn0, c_val, c_lo, c_hi, c_sources = published_series(args.reference_csv, 1)
    fig, ax = plt.subplots(figsize=(7, 5))
    ax.errorbar(
        ebn0, ber, yerr=error_bars(ber, lo, hi), fmt="o-", capsize=3,
        label="control BER (95% product interval)",
    )
    ax.errorbar(
        ebn0, bler, yerr=error_bars(bler, blo, bhi), fmt="^-", capsize=3,
        label="control BLER (97.5% negative-binomial interval)",
    )
    # The published order-1 BER is drawn as a cross-check on the campaign's
    # machinery, not as a reproduction target: no acceptance verdict is
    # computed for any order-1 cell.
    ax.errorbar(
        c_ebn0, c_val, yerr=[c_lo, c_hi], fmt="s--", capsize=3,
        label=f"Fossorier1994 {', '.join(c_sources)} BER" + r" ($\pm\delta$, cross-check only)",
    )
    ax.set_ylabel("error rate")
    style(ax, "eBCH(128,64) OSD order-1 internal controls (not a reproduction target)")
    fig.tight_layout()
    for out in save(fig, args.output_dir, "order1_controls"):
        print(f"wrote {out}")

    # Machine-readable comparison: the single derivation point the prose
    # record and both figures are read from.
    out = args.output_dir / "comparison.json"
    out.write_text(
        json.dumps(comparison(cells, args.reference_csv, receipt), indent=1) + "\n"
    )
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
