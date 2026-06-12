#!/usr/bin/env python3
"""Plot side-by-side AWGN BLER curves: gf2-sim vs aff3ct.

Part of the external-library comparison harness (JIT issue 18e69a1a). Reads a
merged comparison CSV produced by ``run.sh`` and renders both BLER curves on a
single semilog-y axis, annotating the FER=1e-2 crossing of each curve and the
dB gap between them (the headline criterion: agreement within +/- 0.2 dB at
FER 1e-2).

Input CSV columns (header required)::

    es_n0_db, gf2_sim_bler, aff3ct_bler, gf2_sim_fps, aff3ct_fps

Usage (from repo root)::

    python3 dev/benchmarks/gf2-sim/comparison/plot.py \\
        --csv dev/benchmarks/gf2-sim/comparison/dvb-t2-r12-16qam-vs-aff3ct.csv \\
        --title "DVB-T2 r1/2 LDPC (N=64800) vs aff3ct v4.4.0" \\
        --output dev/benchmarks/gf2-sim/comparison/dvb-t2-r12-vs-aff3ct.png

The script is non-interactive (no ``plt.show()``); safe for headless CI.
Only matplotlib (Agg backend) is required.
"""

import argparse
import csv
import sys

try:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
except ImportError:
    print(
        "Error: matplotlib is required. Install with: pip install matplotlib",
        file=sys.stderr,
    )
    sys.exit(1)


def parse_args():
    p = argparse.ArgumentParser(
        description="Plot gf2-sim vs aff3ct AWGN BLER curves from a merged comparison CSV.",
    )
    p.add_argument("--csv", required=True, help="Merged *-vs-aff3ct.csv path.")
    p.add_argument("--output", required=True, help="Output PNG path.")
    p.add_argument("--title", default="gf2-sim vs aff3ct BLER", help="Plot title.")
    return p.parse_args()


def read_rows(path):
    """Reads the merged CSV into a list of dict rows (floats; empty -> None).

    Leading ``#`` comment lines (the provenance banner ``run.sh`` writes) are
    skipped so the header row is the one ``csv.DictReader`` sees.
    """
    rows = []
    with open(path, newline="") as f:
        lines = [ln for ln in f if not ln.lstrip().startswith("#")]
        reader = csv.DictReader(lines)
        for r in reader:
            parsed = {}
            for k, v in r.items():
                v = (v or "").strip()
                parsed[k] = float(v) if v not in ("", "nan", "NaN") else None
            rows.append(parsed)
    return rows


def fer_crossing(xs, blers, target=1e-2):
    """Log-linear interpolation of the Es/N0 at which BLER == ``target``.

    Returns ``None`` if the curve never brackets ``target`` (all points above
    or all below). Assumes ``xs`` ascending. Interpolates in (x, log10(BLER)).
    """
    pts = [
        (x, b)
        for x, b in zip(xs, blers)
        if x is not None and b is not None and b > 0.0
    ]
    pts.sort(key=lambda t: t[0])
    import math

    lt = math.log10(target)
    for i in range(len(pts) - 1):
        x0, b0 = pts[i]
        x1, b1 = pts[i + 1]
        y0, y1 = math.log10(b0), math.log10(b1)
        # target between the two log-BLER values?
        if (y0 - lt) * (y1 - lt) <= 0 and y0 != y1:
            t = (lt - y0) / (y1 - y0)
            return x0 + t * (x1 - x0)
    return None


def main():
    args = parse_args()
    rows = read_rows(args.csv)
    if not rows:
        print(f"Error: no rows in {args.csv}", file=sys.stderr)
        sys.exit(1)

    xs = [r["es_n0_db"] for r in rows]
    gf2 = [r.get("gf2_sim_bler") for r in rows]
    aff = [r.get("aff3ct_bler") for r in rows]

    fig, ax = plt.subplots(figsize=(7.5, 5.5))

    def plot_curve(blers, label, marker, color):
        pts = [
            (x, b)
            for x, b in zip(xs, blers)
            if x is not None and b is not None and b > 0.0
        ]
        if pts:
            px = [p[0] for p in pts]
            pb = [p[1] for p in pts]
            ax.semilogy(px, pb, marker=marker, color=color, label=label, linewidth=1.6)

    plot_curve(gf2, "gf2-sim (NMS 0.75)", "o", "#1f77b4")
    plot_curve(aff, "aff3ct v4.4.0 (NMS 0.75)", "s", "#d62728")

    # FER=1e-2 crossings and gap annotation.
    x_gf2 = fer_crossing(xs, gf2)
    x_aff = fer_crossing(xs, aff)
    ax.axhline(1e-2, color="gray", linestyle="--", linewidth=0.8, alpha=0.7)
    note = "FER=1e-2 crossing:\n"
    if x_gf2 is not None:
        ax.axvline(x_gf2, color="#1f77b4", linestyle=":", linewidth=0.8, alpha=0.6)
        note += f"  gf2-sim: {x_gf2:.3f} dB\n"
    else:
        note += "  gf2-sim: (no bracket)\n"
    if x_aff is not None:
        ax.axvline(x_aff, color="#d62728", linestyle=":", linewidth=0.8, alpha=0.6)
        note += f"  aff3ct:  {x_aff:.3f} dB\n"
    else:
        note += "  aff3ct:  (no bracket)\n"
    if x_gf2 is not None and x_aff is not None:
        note += f"  gap:     {abs(x_gf2 - x_aff):.3f} dB"
    ax.text(
        0.02,
        0.04,
        note,
        transform=ax.transAxes,
        fontsize=9,
        family="monospace",
        verticalalignment="bottom",
        bbox=dict(boxstyle="round", facecolor="white", alpha=0.85),
    )

    ax.set_xlabel("Es/N0 (dB)")
    ax.set_ylabel("Block Error Rate (BLER / FER)")
    ax.set_title(args.title)
    ax.grid(True, which="both", linestyle=":", alpha=0.5)
    ax.legend(loc="upper right")
    fig.tight_layout()
    fig.savefig(args.output, dpi=130)
    print(f"wrote {args.output}")
    if x_gf2 is not None and x_aff is not None:
        print(f"FER=1e-2: gf2-sim {x_gf2:.3f} dB, aff3ct {x_aff:.3f} dB, "
              f"gap {abs(x_gf2 - x_aff):.3f} dB")


if __name__ == "__main__":
    main()
