#!/usr/bin/env python3
"""
Overlay simulation results against SO-GRAND paper reference data.

Usage
-----
Basic comparison (print table):
    python compare_results.py --ref <ref.csv> --sim <sim.csv> [options]

Plot overlay (requires matplotlib):
    python compare_results.py --ref <ref.csv> --sim <sim.csv> --plot

Arguments
---------
--ref FILE      Reference CSV from dev/reference_data/ (produced by parse_pgfplots.py)
--sim FILE      Simulation output CSV (must have eb_n0_db and value columns)
--metric STR    Filter by metric, e.g. BLER or BER (default: all)
--decoder STR   Filter reference curves by decoder, e.g. SOGRAND (default: all)
--plot          Produce a matplotlib figure instead of a table
--out FILE      Save plot to this path (implies --plot)

Simulation CSV format
---------------------
The simulation CSV must contain at least:
    eb_n0_db,value[,metric,decoder,code_params,...]

Any extra columns are ignored.  A 'metric' column is optional; if absent it
defaults to the value of --metric (or 'BLER' if that is also absent).
"""

import argparse
import csv
import sys
from pathlib import Path


# --------------------------------------------------------------------------- #
# I/O helpers                                                                  #
# --------------------------------------------------------------------------- #

def load_csv(path: Path) -> list[dict]:
    with open(path, newline='') as f:
        return list(csv.DictReader(f))


def filter_rows(rows: list[dict],
                metric: str | None = None,
                decoder: str | None = None) -> list[dict]:
    out = rows
    if metric:
        out = [r for r in out if r.get('metric', '').upper() == metric.upper()]
    if decoder:
        out = [r for r in out
               if decoder.lower() in r.get('decoder', '').lower()
               or decoder.lower() in r.get('legend_full', '').lower()]
    return out


def group_by_curve(rows: list[dict], key_col: str = 'decoder') -> dict:
    """Group rows by a curve identifier column, returning dict of lists."""
    groups: dict[str, list] = {}
    for row in rows:
        key = row.get(key_col) or row.get('legend_full') or 'unknown'
        groups.setdefault(key, []).append(row)
    return groups


# --------------------------------------------------------------------------- #
# Comparison logic                                                             #
# --------------------------------------------------------------------------- #

def interpolate(x_target: float, xs: list[float], ys: list[float]) -> float | None:
    """Linear interpolation (log-x domain for BER/BLER curves)."""
    import math
    pairs = sorted(zip(xs, ys))
    for i in range(len(pairs) - 1):
        x0, y0 = pairs[i]
        x1, y1 = pairs[i + 1]
        if x0 <= x_target <= x1:
            if x0 == x1:
                return y0
            t = (x_target - x0) / (x1 - x0)
            return y0 + t * (y1 - y0)
    return None


def compare_curves(ref_rows: list[dict], sim_rows: list[dict]) -> list[dict]:
    """
    For each Eb/N0 in the simulation rows, look up the reference value
    (interpolated if necessary) and compute the ratio and absolute difference.
    """
    ref_xs = [float(r['eb_n0_db']) for r in ref_rows]
    ref_ys = [float(r['value']) for r in ref_rows]

    results = []
    for row in sim_rows:
        x = float(row['eb_n0_db'])
        sim_val = float(row['value'])
        ref_val = interpolate(x, ref_xs, ref_ys)
        if ref_val is None:
            continue
        ratio = sim_val / ref_val if ref_val != 0 else float('inf')
        delta = abs(sim_val - ref_val)
        results.append({
            'eb_n0_db': x,
            'sim': sim_val,
            'ref': ref_val,
            'ratio_sim_over_ref': ratio,
            'abs_diff': delta,
        })
    return results


# --------------------------------------------------------------------------- #
# Output                                                                       #
# --------------------------------------------------------------------------- #

def print_comparison_table(ref_path: Path, sim_path: Path,
                            metric: str | None, decoder: str | None) -> None:
    ref_rows = load_csv(ref_path)
    sim_rows = load_csv(sim_path)

    ref_filtered = filter_rows(ref_rows, metric=metric, decoder=decoder)
    sim_filtered = filter_rows(sim_rows, metric=metric)

    if not ref_filtered:
        print('WARNING: no reference rows match the given filters.', file=sys.stderr)
        return
    if not sim_filtered:
        print('WARNING: no simulation rows match the given filters.', file=sys.stderr)
        return

    # Ungrouped comparison: treat all filtered rows as a single curve each
    cmp = compare_curves(ref_filtered, sim_filtered)
    if not cmp:
        print('No overlapping Eb/N0 points found.', file=sys.stderr)
        return

    header = f"{'Eb/N0':>8}  {'Sim':>12}  {'Ref':>12}  {'Ratio':>8}  {'|Diff|':>12}"
    print(header)
    print('-' * len(header))
    for c in cmp:
        print(f"{c['eb_n0_db']:>8.3f}  {c['sim']:>12.5g}  {c['ref']:>12.5g}"
              f"  {c['ratio_sim_over_ref']:>8.4f}  {c['abs_diff']:>12.5g}")


def plot_overlay(ref_path: Path, sim_path: Path,
                 metric: str | None, decoder: str | None,
                 out_path: Path | None) -> None:
    try:
        import matplotlib.pyplot as plt
        import matplotlib.ticker as ticker
    except ImportError:
        print('matplotlib is required for --plot. Install with: pip install matplotlib',
              file=sys.stderr)
        sys.exit(1)

    ref_rows = filter_rows(load_csv(ref_path), metric=metric, decoder=decoder)
    sim_rows = filter_rows(load_csv(sim_path), metric=metric)

    fig, ax = plt.subplots(figsize=(8, 5))

    # Plot reference curves grouped by legend_full
    ref_groups = group_by_curve(ref_rows, 'legend_full')
    for label, rows in sorted(ref_groups.items()):
        xs = [float(r['eb_n0_db']) for r in rows]
        ys = [float(r['value']) for r in rows]
        xs, ys = zip(*sorted(zip(xs, ys)))
        ax.semilogy(xs, ys, '--', linewidth=1.2,
                    label=f'Ref: {label}' if label else 'Reference')

    # Plot simulation curves
    sim_groups = group_by_curve(sim_rows, 'decoder')
    for label, rows in sorted(sim_groups.items()):
        xs = [float(r['eb_n0_db']) for r in rows]
        ys = [float(r['value']) for r in rows]
        xs, ys = zip(*sorted(zip(xs, ys)))
        ax.semilogy(xs, ys, 'o-', linewidth=1.8,
                    label=f'Sim: {label}' if label else 'Simulation')

    ax.set_xlabel('Eb/N0 (dB)')
    y_label = metric if metric else 'Value'
    ax.set_ylabel(y_label)
    ax.set_title(f'Reference vs. Simulation — {ref_path.stem}')
    ax.grid(True, which='both', linestyle=':')
    ax.legend(fontsize=8, loc='best')

    if out_path:
        fig.savefig(out_path, bbox_inches='tight', dpi=150)
        print(f'Plot saved to {out_path}')
    else:
        plt.show()


# --------------------------------------------------------------------------- #
# Entry point                                                                  #
# --------------------------------------------------------------------------- #

def main() -> None:
    p = argparse.ArgumentParser(
        description='Compare simulation results against SO-GRAND paper reference data.')
    p.add_argument('--ref', required=True, type=Path,
                   help='Reference CSV (from dev/reference_data/)')
    p.add_argument('--sim', required=True, type=Path,
                   help='Simulation output CSV')
    p.add_argument('--metric', default=None,
                   help='Filter by metric (BLER, BER, …)')
    p.add_argument('--decoder', default=None,
                   help='Filter reference by decoder name substring')
    p.add_argument('--plot', action='store_true',
                   help='Show a matplotlib overlay plot')
    p.add_argument('--out', default=None, type=Path,
                   help='Save plot to this file (implies --plot)')
    args = p.parse_args()

    if args.out:
        args.plot = True

    if args.plot:
        plot_overlay(args.ref, args.sim,
                     metric=args.metric, decoder=args.decoder,
                     out_path=args.out)
    else:
        print_comparison_table(args.ref, args.sim,
                                metric=args.metric, decoder=args.decoder)


if __name__ == '__main__':
    main()
