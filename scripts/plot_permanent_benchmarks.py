#!/usr/bin/env python3
"""Publication-grade figures for the gf2-algebra permanent benchmark artefact.

Design: Option A -- single script with subcommands (speedup|parallel|cross_cpu|gpu_crossover|all).
Rendering: matplotlib (Python), Agg backend (non-interactive, deterministic PNG output).

Input CSVs (default: dev/benchmarks/gf2_algebra_permanent/):
  s1_speedup-2026-05-11.csv       -- S1 single-thread speedup vs reference (F_3 perm log-time vs n)
  s2_parallel_scaling-2026-05-11.csv -- S2 parallel scaling
  s3_cross_cpu-2026-05-12.csv     -- S3 cross-CPU bars (AVX2-only on dev host; AVX-512 N/A)
  s5_gpu_crossover-2026-05-15.csv -- S5 GPU-vs-CPU crossover

Output figures (default: dev/benchmarks/gf2_algebra_permanent/figures/):
  s1_perm_log_time_vs_n.png       -- Fig (a) log-time vs n for F_3 permanent paths
  s2_parallel_scaling.png         -- Fig (b) parallel-scaling vs core count
  s3_cross_cpu_bars.png           -- Fig (c) AVX2-vs-scalar bars (AVX-512 N/A gracefully)
  s5_gpu_vs_cpu_crossover.png     -- Fig (d) GPU-vs-CPU crossover

Determinism strategy:
  - matplotlib.use("Agg") selected before any other mpl import (no display backend).
  - plt.rcParams["svg.hashsalt"] = "gf2-algebra-permanent" for stable SVG element IDs.
  - No random number usage; all data derived deterministically from CSV inputs.
  - PNG output is deterministic given the same matplotlib version and input data.
  - matplotlib version pinned: 3.10.9 (verified on dev host AMD Ryzen 9 5900X).

Usage:
  python3 scripts/plot_permanent_benchmarks.py [subcommand] [--input-dir DIR] [--output-dir DIR]
  Subcommands: speedup | parallel | cross_cpu | gpu_crossover | all  (default: all)
"""

import argparse
import os
import sys
from pathlib import Path

# --- Determinism: select Agg before importing pyplot ---
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker

# Stable SVG element IDs (no effect on PNG, harmless when set).
plt.rcParams["svg.hashsalt"] = "gf2-algebra-permanent"

import numpy as np

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _resolve_csv(input_dir: Path, filename: str) -> Path:
    """Return the absolute path to *filename* under *input_dir*.

    Raises FileNotFoundError with the absolute path if the file is missing.
    If the exact dated filename is absent but a date-suffix glob match exists,
    log the substitution to stderr so the audit trail records which file was
    actually consumed.
    """
    path = input_dir / filename
    if path.exists():
        return path
    stem = filename.split("-")[0]  # e.g. "s1_speedup"
    candidates = sorted(input_dir.glob(f"{stem}-*.csv"))
    if candidates:
        substitute = candidates[-1]  # most recent date suffix wins
        print(
            f"  [warn] {filename!r} not found in {input_dir}; "
            f"using {substitute.name!r} instead (most recent match)",
            file=sys.stderr,
        )
        return substitute
    raise FileNotFoundError(f"missing input CSV: {path.resolve()}")


def _read_csv(path: Path) -> list[dict]:
    """Read a CSV with '#'-prefixed comment lines, returning list of row dicts."""
    rows = []
    headers = None
    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if headers is None:
                headers = [h.strip() for h in line.split(",")]
                continue
            values = [v.strip() for v in line.split(",")]
            rows.append(dict(zip(headers, values)))
    return rows


def _apply_style() -> None:
    """Apply project-consistent style (no seaborn dependency)."""
    plt.rcParams.update({
        "figure.dpi": 150,
        "figure.figsize": (7, 4.5),
        "font.family": "sans-serif",
        "font.size": 10,
        "axes.titlesize": 11,
        "axes.labelsize": 10,
        "legend.fontsize": 9,
        "grid.alpha": 0.35,
        "lines.linewidth": 1.8,
        "lines.markersize": 6,
    })


# ---------------------------------------------------------------------------
# Figure (a): S1 -- log-time vs n for F_3 permanent paths
# ---------------------------------------------------------------------------

S1_FILENAME = "s1_speedup-2026-05-11.csv"

def plot_speedup(input_dir: Path, output_dir: Path) -> Path:
    """Fig (a): wall-clock time (log scale) vs n for the F_3 permanent paths.

    Overlays:
      - permanent_mod3_reference (S1)
      - permanent_bipedal3_simd (S1; AVX2)
      - permanent_bipedal3_parallel (S2; 12-thread, the dev host's full physical-core
        count). The S2 CSV's thread sweep at each n is restricted to max-thread rows
        for this overlay so the curve directly reads as wall-clock vs n at the parallel
        path's best configuration.

    The scalar bipedal3 fallback path is exercised at unit-test time
    (`test_simd_vs_scalar_n24` and friends) but is not separately benchmarked in S1,
    because the production dispatch path always selects AVX2 on the dev host. If a
    scalar-only S1 column appears in future, the auto-style branch picks it up.
    """
    csv_path = _resolve_csv(input_dir, S1_FILENAME)
    rows = _read_csv(csv_path)

    # Group by impl
    data: dict[str, tuple[list, list]] = {}
    for row in rows:
        impl = row["impl"]
        n = int(row["n"])
        mean_us = float(row["mean_us"])
        data.setdefault(impl, ([], []))
        data[impl][0].append(n)
        data[impl][1].append(mean_us / 1e6)  # convert to seconds

    # Sort by n within each impl
    for impl in data:
        ns, ts = data[impl]
        paired = sorted(zip(ns, ts))
        data[impl] = ([p[0] for p in paired], [p[1] for p in paired])

    # Overlay S2 max-thread parallel data if present.
    parallel_ns: list[int] = []
    parallel_ts: list[float] = []
    try:
        s2_path = _resolve_csv(input_dir, S2_FILENAME)
        s2_rows = _read_csv(s2_path)
        # Group by n, pick the max-thread row per n.
        per_n: dict[int, tuple[int, float]] = {}
        for row in s2_rows:
            n = int(row["n"])
            t = int(row["threads"])
            mean_s = float(row["mean_us"]) / 1e6
            prev = per_n.get(n)
            if prev is None or t > prev[0]:
                per_n[n] = (t, mean_s)
        for n in sorted(per_n.keys()):
            parallel_ns.append(n)
            parallel_ts.append(per_n[n][1])
    except FileNotFoundError:
        # S2 absent: emit a clear info line; the figure still renders reference + SIMD.
        print("  [info] S2 CSV missing; parallel curve omitted from figure (a)",
              file=sys.stderr)

    _apply_style()
    fig, ax = plt.subplots()

    style_map = {
        "permanent_mod3_reference": dict(color="#c0392b", marker="o", linestyle="-",  label="Reference (mod-3 Ryser)"),
        "permanent_bipedal3_simd": dict(color="#2980b9", marker="s", linestyle="-",  label="Bipedal3-SIMD (AVX2)"),
    }
    # Additional impls (e.g. scalar fallback) plotted with auto style if present.
    auto_colors = ["#27ae60", "#8e44ad", "#f39c12"]
    auto_idx = 0

    for impl, (ns, ts) in sorted(data.items()):
        kw = style_map.get(impl)
        if kw is None:
            c = auto_colors[auto_idx % len(auto_colors)]
            auto_idx += 1
            kw = dict(color=c, marker="^", linestyle="--", label=impl.replace("_", " "))
        ax.semilogy(ns, ts, **kw)

    if parallel_ns:
        ax.semilogy(parallel_ns, parallel_ts, color="#27ae60", marker="D",
                    linestyle="-.", label="Bipedal3-parallel (12-thread)")

    ax.set_xlabel("Matrix dimension n")
    ax.set_ylabel("Wall-clock time (seconds, log scale)")
    ax.set_title("F₃ Permanent: log-time vs n (single thread)")
    ax.xaxis.set_major_locator(ticker.MultipleLocator(4))
    ax.grid(True, which="both")
    ax.legend()
    fig.tight_layout()

    out_path = output_dir / "s1_perm_log_time_vs_n.png"
    fig.savefig(out_path, dpi=150)
    plt.close(fig)
    print(f"  wrote {out_path}")
    return out_path


# ---------------------------------------------------------------------------
# Figure (b): S2 -- parallel scaling vs core count
# ---------------------------------------------------------------------------

S2_FILENAME = "s2_parallel_scaling-2026-05-11.csv"

def plot_parallel(input_dir: Path, output_dir: Path) -> Path:
    """Fig (b): parallel efficiency (scaling factor) vs thread count, one curve per n."""
    csv_path = _resolve_csv(input_dir, S2_FILENAME)
    rows = _read_csv(csv_path)

    # Group by n
    data: dict[int, tuple[list, list, list, list]] = {}
    for row in rows:
        n = int(row["n"])
        threads = int(row["threads"])
        sf = float(row["scaling_factor"])
        ci_lo = float(row["scaling_ci_lo"])
        ci_hi = float(row["scaling_ci_hi"])
        data.setdefault(n, ([], [], [], []))
        data[n][0].append(threads)
        data[n][1].append(sf)
        data[n][2].append(ci_lo)
        data[n][3].append(ci_hi)

    # Sort by threads within each n
    for n in data:
        ts, sf, lo, hi = data[n]
        paired = sorted(zip(ts, sf, lo, hi))
        data[n] = (
            [p[0] for p in paired],
            [p[1] for p in paired],
            [p[2] for p in paired],
            [p[3] for p in paired],
        )

    _apply_style()
    fig, ax = plt.subplots()

    color_map = {28: "#2980b9", 32: "#27ae60", 36: "#8e44ad"}
    all_threads: set[int] = set()
    for ts, *_ in data.values():
        all_threads.update(ts)
    t_max = max(all_threads)

    # Ideal speedup line (1 core = 1.0, t cores = t, but expressed as fraction of
    # single-thread mean; ideal scaling_factor = threads / threads = 1.0 always.
    # S2 defines scaling_factor = mean_1thread / (threads * mean_t), so ideal = 1.0.
    thread_grid = sorted(all_threads)
    ax.axhline(1.0, color="#7f8c8d", linestyle="--", linewidth=1.0, label="Ideal (linear)")

    for n in sorted(data.keys()):
        ts, sf, lo, hi = data[n]
        c = color_map.get(n, "#c0392b")
        ax.plot(ts, sf, marker="o", color=c, label=f"n={n}")
        # CI ribbon
        ax.fill_between(ts, lo, hi, color=c, alpha=0.15)

    ax.set_xlabel("Thread count")
    ax.set_ylabel("Parallel efficiency (scaling factor)")
    ax.set_title("F₃ Permanent: parallel scaling vs core count (bipedal3-SIMD)")
    ax.set_xticks(thread_grid)
    ax.set_ylim(0.5, 1.15)
    ax.grid(True)
    ax.legend()
    fig.tight_layout()

    out_path = output_dir / "s2_parallel_scaling.png"
    fig.savefig(out_path, dpi=150)
    plt.close(fig)
    print(f"  wrote {out_path}")
    return out_path


# ---------------------------------------------------------------------------
# Figure (c): S3 -- AVX2-vs-scalar bars (AVX-512 N/A on this host)
# ---------------------------------------------------------------------------

S3_FILENAME = "s3_cross_cpu-2026-05-12.csv"

def plot_cross_cpu(input_dir: Path, output_dir: Path) -> Path:
    """Fig (c): throughput bars: scalar vs AVX2 (AVX-512 axis N/A on dev host).

    S3 scope was amended to AVX2-only (see CSV header comment).  The script
    renders a grouped-bar chart with a clear note that AVX-512 data is absent.
    This is not a missing-data error -- it is explicitly documented in the CSV
    header and the script handles it gracefully without raising.
    """
    csv_path = _resolve_csv(input_dir, S3_FILENAME)
    rows = _read_csv(csv_path)

    # We want: for each n, bars for scalar, avx2 (and avx512 if present).
    # mean_us -> throughput = 1e6 / mean_us  (perms/second)
    data: dict[str, dict[int, float]] = {}
    for row in rows:
        impl = row["impl"]
        n = int(row["n"])
        mean_us = float(row["mean_us"])
        throughput = 1e6 / mean_us  # perms/second
        data.setdefault(impl, {})[n] = throughput

    # Classify impls
    scalar_impl = next((k for k in data if "scalar" in k), None)
    avx2_impl   = next((k for k in data if "avx2" in k and "sanity" not in k), None)
    avx512_impl = next((k for k in data if "avx512" in k), None)

    # Common n values across scalar and avx2 (for sanity-check at n=16/20/24)
    sanity_impl = next((k for k in data if "sanity" in k), None)

    # Guard: refuse to silently save an empty figure if the impl classifiers
    # didn't recognise the CSV's impl names. The substring heuristics depend on
    # the S3 CSV's column naming convention; if it changes upstream we want a
    # clear error rather than an empty bar chart.
    if not scalar_impl or not (sanity_impl or avx2_impl):
        raise ValueError(
            f"S3 CSV at {csv_path} did not yield recognisable 'scalar' and "
            f"'avx2' impl names; found: {sorted(data.keys())}"
        )

    # Choose n values: prefer sanity-check rows (n=16,20,24) since those have both scalar+avx2.
    # Also include the large-n avx2-only rows for context, displayed separately.
    small_ns = sorted(set(data.get(scalar_impl, {}).keys()) & set(data.get(sanity_impl or avx2_impl, {}).keys()))
    large_ns = sorted(set(data.get(avx2_impl, {}).keys()) - set(small_ns))

    _apply_style()
    fig, (ax_small, ax_large) = plt.subplots(1, 2, figsize=(10, 4.5), gridspec_kw={"width_ratios": [3, 2]})

    # -- Left panel: small-n grouped bars (scalar vs AVX2-sanity) --
    bar_width = 0.35
    x = np.arange(len(small_ns))

    if scalar_impl and small_ns:
        vals_scalar = [data[scalar_impl].get(n, 0.0) for n in small_ns]
        ax_small.bar(x - bar_width/2, vals_scalar, bar_width,
                     label="Scalar", color="#c0392b", alpha=0.85)

    avx2_key = sanity_impl if sanity_impl else avx2_impl
    if avx2_key and small_ns:
        vals_avx2 = [data[avx2_key].get(n, 0.0) for n in small_ns]
        ax_small.bar(x + bar_width/2, vals_avx2, bar_width,
                     label="AVX2", color="#2980b9", alpha=0.85)

    if avx512_impl:
        vals_avx512 = [data[avx512_impl].get(n, 0.0) for n in small_ns]
        ax_small.bar(x + bar_width*1.5, vals_avx512, bar_width,
                     label="AVX-512", color="#8e44ad", alpha=0.85)
    else:
        # Annotate clearly that AVX-512 is N/A
        ax_small.text(0.97, 0.95, "AVX-512: N/A\n(dev host: AVX2 only)",
                      transform=ax_small.transAxes, fontsize=8,
                      ha="right", va="top",
                      bbox=dict(boxstyle="round,pad=0.3", facecolor="#ffeeba", alpha=0.8))

    ax_small.set_xticks(x)
    ax_small.set_xticklabels([f"n={n}" for n in small_ns])
    ax_small.set_ylabel("Throughput (perms / second)")
    ax_small.set_title("Cross-CPU throughput: small n\n(scalar vs AVX2, AVX-512 N/A)")
    ax_small.legend()
    ax_small.grid(True, axis="y")

    # -- Right panel: large-n AVX2 throughput (reference absent for n=28..36) --
    if avx2_impl and large_ns:
        vals_large = [data[avx2_impl].get(n, 0.0) for n in large_ns]
        x2 = np.arange(len(large_ns))
        ax_large.bar(x2, vals_large, color="#2980b9", alpha=0.85, label="AVX2")
        ax_large.set_xticks(x2)
        ax_large.set_xticklabels([f"n={n}" for n in large_ns])
        ax_large.set_title("AVX2 throughput: large n")
        ax_large.set_ylabel("Throughput (perms / second)")
        ax_large.yaxis.set_major_formatter(ticker.FormatStrFormatter("%.4f"))
        ax_large.grid(True, axis="y")
        ax_large.text(0.97, 0.95, "AVX-512: N/A", transform=ax_large.transAxes,
                      fontsize=8, ha="right", va="top",
                      bbox=dict(boxstyle="round,pad=0.3", facecolor="#ffeeba", alpha=0.8))
    else:
        ax_large.set_visible(False)

    fig.suptitle("S3: Cross-CPU throughput sweep (AVX2-only scope; host: AMD Ryzen 9 5900X / Zen 3)")
    fig.tight_layout()

    out_path = output_dir / "s3_cross_cpu_bars.png"
    fig.savefig(out_path, dpi=150)
    plt.close(fig)
    print(f"  wrote {out_path}")
    return out_path


# ---------------------------------------------------------------------------
# Figure (d): S5 -- GPU-vs-CPU crossover
# ---------------------------------------------------------------------------

S5_FILENAME = "s5_gpu_crossover-2026-05-15.csv"

def plot_gpu_crossover(input_dir: Path, output_dir: Path) -> Path:
    """Fig (d): GPU vs CPU-SIMD throughput and crossover (batch-size-dependent)."""
    csv_path = _resolve_csv(input_dir, S5_FILENAME)
    rows = _read_csv(csv_path)

    ns = []
    cpu_pps = []
    gpu_pps = []
    ratios = []
    gpu_wins_flags = []

    for row in rows:
        n = int(row["n"])
        ratio_raw = row["gpu_cpu_ratio"]
        gpu_wins_raw = row["gpu_wins"]
        cpu_pps_raw = float(row["cpu_simd_perm_per_s"])
        gpu_pps_raw = float(row["gpu_perm_per_s"])

        # Skip rows where both measurements are zero (no usable data)
        if cpu_pps_raw == 0.0 and gpu_pps_raw == 0.0:
            continue

        ns.append(n)
        cpu_pps.append(cpu_pps_raw)
        gpu_pps.append(gpu_pps_raw)

        if ratio_raw.lower() == "na":
            ratios.append(None)
            gpu_wins_flags.append(None)
        else:
            ratios.append(float(ratio_raw))
            gpu_wins_flags.append(gpu_wins_raw.strip().lower() == "true")

    _apply_style()
    fig, (ax_tp, ax_ratio) = plt.subplots(1, 2, figsize=(11, 4.5))

    # -- Left: throughput bars (CPU vs GPU) --
    x = np.arange(len(ns))
    bar_w = 0.35
    cpu_bars = ax_tp.bar(x - bar_w/2, cpu_pps, bar_w, label="CPU-SIMD (AVX2)", color="#2980b9", alpha=0.85)
    gpu_bars = ax_tp.bar(x + bar_w/2, gpu_pps, bar_w, label="GPU (gfx1030 / RX 6950 XT)", color="#e67e22", alpha=0.85)
    ax_tp.set_xticks(x)
    ax_tp.set_xticklabels([f"n={n}" for n in ns])
    ax_tp.set_ylabel("Throughput (perms / second)")
    ax_tp.set_title("GPU vs CPU-SIMD throughput\n(F₃ permanent, bipedal3)")
    ax_tp.set_yscale("log")
    ax_tp.legend()
    ax_tp.grid(True, which="both", axis="y")

    # Annotate bars where GPU wins
    for i, (cpu_v, gpu_v, wins) in enumerate(zip(cpu_pps, gpu_pps, gpu_wins_flags)):
        if wins is True:
            ax_tp.annotate("GPU wins", xy=(i + bar_w/2, gpu_v),
                           xytext=(0, 6), textcoords="offset points",
                           ha="center", fontsize=7, color="#e67e22")
        elif wins is False:
            ax_tp.annotate("CPU wins", xy=(i - bar_w/2, cpu_v),
                           xytext=(0, 6), textcoords="offset points",
                           ha="center", fontsize=7, color="#2980b9")

    # -- Right: GPU/CPU ratio vs n --
    valid = [(n, r) for n, r, w in zip(ns, ratios, gpu_wins_flags) if r is not None]
    if valid:
        v_ns, v_ratios = zip(*valid)
        colors_ratio = ["#e67e22" if r >= 1.0 else "#2980b9" for r in v_ratios]
        ax_ratio.bar(range(len(v_ns)), v_ratios, color=colors_ratio, alpha=0.85)
        ax_ratio.set_xticks(range(len(v_ns)))
        ax_ratio.set_xticklabels([f"n={n}" for n in v_ns])
        ax_ratio.axhline(1.0, color="#7f8c8d", linestyle="--", linewidth=1.0, label="Crossover (ratio=1)")
        ax_ratio.set_yscale("log")
        ax_ratio.set_ylabel("GPU/CPU throughput ratio (log scale)")
        ax_ratio.set_title("GPU/CPU ratio vs n\n(orange: GPU faster; blue: CPU faster)")
        ax_ratio.legend()
        ax_ratio.grid(True, which="both", axis="y")

        # Annotate N/A entries
        na_ns = [n for n, r, w in zip(ns, ratios, gpu_wins_flags) if r is None]
        if na_ns:
            na_text = "N/A: " + ", ".join(f"n={n}" for n in na_ns)
            ax_ratio.text(0.97, 0.05, na_text + "\n(GPU killed; est. >37 min)",
                          transform=ax_ratio.transAxes, fontsize=7,
                          ha="right", va="bottom",
                          bbox=dict(boxstyle="round,pad=0.3", facecolor="#f8d7da", alpha=0.8))

    fig.suptitle("S5: GPU-vs-CPU crossover  |  CPU: AMD Ryzen 9 5900X  |  GPU: AMD Radeon RX 6950 XT (gfx1030)")
    fig.tight_layout()

    out_path = output_dir / "s5_gpu_vs_cpu_crossover.png"
    fig.savefig(out_path, dpi=150)
    plt.close(fig)
    print(f"  wrote {out_path}")
    return out_path


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

SUBCOMMANDS = {
    "speedup":     plot_speedup,
    "parallel":    plot_parallel,
    "cross_cpu":   plot_cross_cpu,
    "gpu_crossover": plot_gpu_crossover,
}

def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "subcommand",
        nargs="?",
        default="all",
        choices=list(SUBCOMMANDS.keys()) + ["all"],
        help="Which figure(s) to produce (default: all)",
    )
    parser.add_argument(
        "--input-dir",
        type=Path,
        default=Path("dev/benchmarks/gf2_algebra_permanent"),
        help="Directory containing input CSVs (default: dev/benchmarks/gf2_algebra_permanent/)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("dev/benchmarks/gf2_algebra_permanent/figures"),
        help="Directory to write figures (default: dev/benchmarks/gf2_algebra_permanent/figures/)",
    )
    args = parser.parse_args()

    input_dir = args.input_dir.resolve()
    output_dir = args.output_dir.resolve()

    if not input_dir.is_dir():
        raise FileNotFoundError(f"missing input CSV: {input_dir} (not a directory)")

    output_dir.mkdir(parents=True, exist_ok=True)

    if args.subcommand == "all":
        targets = list(SUBCOMMANDS.keys())
    else:
        targets = [args.subcommand]

    # Pre-flight: in `all` mode, verify every target's input CSV resolves
    # BEFORE we write any figure. This prevents the "partial figure set"
    # failure mode where S1/S2/S3 succeed and we crash on S5's missing input,
    # leaving an inconsistent figures/ directory on disk.
    csv_for_target = {
        "speedup":       S1_FILENAME,
        "parallel":      S2_FILENAME,
        "cross_cpu":     S3_FILENAME,
        "gpu_crossover": S5_FILENAME,
    }
    if args.subcommand == "all":
        missing = []
        for name in targets:
            try:
                _resolve_csv(input_dir, csv_for_target[name])
            except FileNotFoundError as exc:
                missing.append(str(exc))
        if missing:
            raise FileNotFoundError(
                "all-mode pre-flight: refusing to write any figure because "
                "one or more required input CSVs are missing:\n  - "
                + "\n  - ".join(missing)
            )

    print(f"Input:  {input_dir}")
    print(f"Output: {output_dir}")
    for name in targets:
        print(f"[{name}]")
        SUBCOMMANDS[name](input_dir, output_dir)

    print("Done.")


if __name__ == "__main__":
    main()
