#!/usr/bin/env python3
"""Generate paper-comparison SVGs for the GRAND/SOGRAND epic presentation.

Reads canonical simulation CSVs from ``dev/simulation_results/`` and paper
reference CSVs from ``dev/reference_data/``, and emits one SVG per figure
(Fig 4 / Fig 5 / Fig 6 / Fig 7 / Fig 8 / Fig 9) into the same directory
this script lives in.
"""

from __future__ import annotations

import csv
from pathlib import Path

import matplotlib
import matplotlib.pyplot as plt

matplotlib.rcParams.update({
    "font.family": "DejaVu Sans",
    "axes.labelsize": 11,
    "axes.titlesize": 12,
    "legend.fontsize": 9,
    "xtick.labelsize": 9,
    "ytick.labelsize": 9,
    "figure.dpi": 100,
    "savefig.dpi": 120,
    "svg.fonttype": "none",
})

REPO = Path(__file__).resolve().parents[3]
SIM = REPO / "dev" / "simulation_results"
REF = REPO / "dev" / "reference_data"
OUT = Path(__file__).resolve().parent


def read_sim(path: Path) -> tuple[list[float], list[float]]:
    xs, ys = [], []
    with path.open() as f:
        for row in csv.DictReader(f):
            try:
                x = float(row["eb_n0_db"])
                y = float(row["bler"])
            except (KeyError, ValueError):
                continue
            xs.append(x)
            ys.append(y)
    return xs, ys


def read_ref(path: Path, decoder: str) -> tuple[list[float], list[float]]:
    xs, ys = [], []
    with path.open() as f:
        for row in csv.DictReader(f):
            if row.get("metric") != "BLER_or_BER":
                continue
            if row.get("decoder") != decoder:
                continue
            try:
                x = float(row["eb_n0_db"])
                y = float(row["value"])
            except (KeyError, ValueError):
                continue
            xs.append(x)
            ys.append(y)
    pairs = sorted(zip(xs, ys))
    return [p[0] for p in pairs], [p[1] for p in pairs]


def plot_figure(
    title: str,
    out_name: str,
    curves: list[tuple[str, list[float], list[float], dict]],
    ylim: tuple[float, float] = (1e-6, 1.2),
    xlim: tuple[float, float] | None = None,
):
    fig, ax = plt.subplots(figsize=(7.5, 4.8))
    for label, xs, ys, kw in curves:
        ys_plot = [max(y, 1e-7) for y in ys]
        ax.semilogy(xs, ys_plot, label=label, **kw)
    ax.set_xlabel(r"$E_\mathrm{b}/N_0$ (dB)")
    ax.set_ylabel("BLER")
    ax.set_title(title)
    ax.grid(True, which="both", alpha=0.3)
    ax.set_ylim(*ylim)
    if xlim:
        ax.set_xlim(*xlim)
    ax.legend(loc="lower left", framealpha=0.9)
    fig.tight_layout()
    path = OUT / out_name
    fig.savefig(path, format="svg")
    plt.close(fig)
    print(f"Wrote {path.relative_to(REPO)}")


def main() -> None:
    # Fig 4 — (625, 225) CRC(25,15)^2
    ours_x, ours_y = read_sim(SIM / "fig4_crc_product.csv")
    sp_x, sp_y = read_sim(SIM / "fig4_ldpc_sp.csv")
    nms_x, nms_y = read_sim(SIM / "fig4_ldpc_nms.csv")
    paper_x, paper_y = read_ref(REF / "fig_prod_crc_25x15.csv", "CRC_prod_SOGRAND")
    plot_figure(
        title=r"Fig. 4 — $(625, 225) = (25,15)^2$ CRC product vs 5G NR LDPC, AWGN",
        out_name="fig4_crc_25_15_comparison.svg",
        curves=[
            ("Paper: CRC product (SOGRAND)", paper_x, paper_y,
             dict(color="#d62728", marker="o", linestyle="--", linewidth=1.5, markersize=6)),
            ("Ours: CRC product (SOGRAND)", ours_x, ours_y,
             dict(color="#d62728", marker="s", linestyle="-", linewidth=2, markersize=6)),
            ("Ours: LDPC BG2 (SP)", sp_x, sp_y,
             dict(color="#2ca02c", marker="^", linestyle="-", linewidth=1.5, markersize=5)),
            ("Ours: LDPC BG2 (NMS)", nms_x, nms_y,
             dict(color="#1f77b4", marker="v", linestyle="-", linewidth=1.5, markersize=5)),
        ],
        ylim=(1e-5, 1.2),
        xlim=(-0.1, 3.1),
    )

    # Fig 5 — (4096, 3249) eBCH(64,57)^2
    ours_x, ours_y = read_sim(SIM / "fig5_ebch_64_57_product.csv")
    sp_x, sp_y = read_sim(SIM / "fig5_ldpc_sp.csv")
    nms_x, nms_y = read_sim(SIM / "fig5_ldpc_nms.csv")
    paper_x, paper_y = read_ref(REF / "fig_prod_ebch_64x57_sq_noP.csv", "eBCH_prod_SOGRAND")
    plot_figure(
        title=r"Fig. 5 — $(4096, 3249) = (64,57)^2$ eBCH product vs 5G NR LDPC, AWGN",
        out_name="fig5_ebch_64_57_comparison.svg",
        curves=[
            ("Paper: eBCH product (SOGRAND)", paper_x, paper_y,
             dict(color="#d62728", marker="o", linestyle="--", linewidth=1.5, markersize=6)),
            ("Ours: eBCH product (SOGRAND)", ours_x, ours_y,
             dict(color="#d62728", marker="s", linestyle="-", linewidth=2, markersize=6)),
            ("Ours: LDPC BG1 (SP)", sp_x, sp_y,
             dict(color="#2ca02c", marker="^", linestyle="-", linewidth=1.5, markersize=5)),
            ("Ours: LDPC BG1 (NMS)", nms_x, nms_y,
             dict(color="#1f77b4", marker="v", linestyle="-", linewidth=1.5, markersize=5)),
        ],
        ylim=(1e-7, 1.5),
        xlim=(1.9, 3.8),
    )

    # Fig 6 — (256, 49) eBCH(16,7)^2
    ours_x, ours_y = read_sim(SIM / "fig6_ebch_16_7_product.csv")
    sp_x, sp_y = read_sim(SIM / "fig6_ldpc_sp.csv")
    nms_x, nms_y = read_sim(SIM / "fig6_ldpc_nms.csv")
    paper_x, paper_y = read_ref(REF / "fig_prod_ebch_256x49.csv", "eBCH_prod_SOGRAND")
    plot_figure(
        title=r"Fig. 6 — $(256, 49) = (16,7)^2$ eBCH product vs 5G NR LDPC, AWGN",
        out_name="fig6_ebch_16_7_comparison.svg",
        curves=[
            ("Paper: eBCH product (SOGRAND)", paper_x, paper_y,
             dict(color="#d62728", marker="o", linestyle="--", linewidth=1.5, markersize=6)),
            ("Ours: eBCH product (SOGRAND)", ours_x, ours_y,
             dict(color="#d62728", marker="s", linestyle="-", linewidth=2, markersize=6)),
            ("Ours: LDPC BG2 (SP)", sp_x, sp_y,
             dict(color="#2ca02c", marker="^", linestyle="-", linewidth=1.5, markersize=5)),
            ("Ours: LDPC BG2 (NMS)", nms_x, nms_y,
             dict(color="#1f77b4", marker="v", linestyle="-", linewidth=1.5, markersize=5)),
        ],
        ylim=(1e-5, 1.2),
        xlim=(-0.1, 4.1),
    )


if __name__ == "__main__":
    main()
