#!/usr/bin/env python3
"""Plot the eBCH(128,64) OSD campaign receipt against the pinned reference series.

Reads:
  - ``ebch_osd_awgn.json``  (schema-2 campaign receipt beside this script)
  - ``dev/reference_data/osd_ebch_128_64_fossorier1994.csv``  (digitized series)

Produces, beside this script:
  - ``order2_ber_comparison.png``  (order-2 BER vs published order-2 series)
  - ``order1_controls.png``        (order-1 internal-control BER/BLER series)

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
import sys
from pathlib import Path

try:
    import matplotlib

    matplotlib.use("Agg")
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
    ebn0, value, lo, hi = [], [], [], []
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
    return ebn0, value, lo, hi


def error_bars(points, lower, upper):
    return [
        [p - l for p, l in zip(points, lower)],
        [u - p for p, u in zip(points, upper)],
    ]


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
    p_ebn0, p_val, p_lo, p_hi = published_series(args.reference_csv, 2)
    fig, ax = plt.subplots(figsize=(7, 5))
    ax.errorbar(
        ebn0, ber, yerr=error_bars(ber, lo, hi), fmt="o-", capsize=3,
        label=f"simulation (schema {schema}, seed {seed:#x}, 95% product interval)",
    )
    ax.errorbar(
        p_ebn0, p_val, yerr=[p_lo, p_hi], fmt="s--", capsize=3,
        label=r"Fossorier1994 Fig. 4.14 (digitized, $\pm\delta$ decades)",
    )
    ax.set_ylabel("BER")
    style(ax, "eBCH(128,64) OSD order-2: simulation vs published BER")
    fig.tight_layout()
    out = args.output_dir / "order2_ber_comparison.png"
    fig.savefig(out, dpi=150)
    print(f"wrote {out}")

    # Order-1 internal-control figure.
    ebn0, ber, lo, hi, bler, blo, bhi = series(cells, 1)
    fig, ax = plt.subplots(figsize=(7, 5))
    ax.errorbar(
        ebn0, ber, yerr=error_bars(ber, lo, hi), fmt="o-", capsize=3,
        label="BER (95% product interval)",
    )
    ax.errorbar(
        ebn0, bler, yerr=error_bars(bler, blo, bhi), fmt="^-", capsize=3,
        label="BLER (97.5% negative-binomial interval)",
    )
    ax.set_ylabel("error rate")
    style(ax, "eBCH(128,64) OSD order-1 internal controls (no published claim)")
    fig.tight_layout()
    out = args.output_dir / "order1_controls.png"
    fig.savefig(out, dpi=150)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
