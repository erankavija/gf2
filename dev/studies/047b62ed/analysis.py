#!/usr/bin/env python3
"""Derive every table in `receipts.md` from the committed campaign CSVs.

Run from the repository root:

    python3 dev/studies/047b62ed/analysis.py

The script reads only the committed artifacts of campaign run
`20260813T230032Z-1321576` and writes nothing. Every figure quoted in
`receipts.md` is printed here under the heading that names its section, so a
reviewer can diff prose against data without re-running the measurement.
"""

from __future__ import annotations

import csv
import math
import pathlib

STUDY = pathlib.Path(__file__).resolve().parent
RUN = "permanent-campaign-20260813T230032Z-1321576"
GRID = STUDY / f"{RUN}-q3-grid.csv"
GRAY = STUDY / f"{RUN}-q3-gray-update.csv"
HPROD = STUDY / f"{RUN}-q3-horizontal-product.csv"
EQUIV = STUDY / f"{RUN}-shared-equivalence.csv"

Q = 3
ORDERS = [12, 16, 20, 24, 28]


def read(path: pathlib.Path) -> list[dict[str, str]]:
    with path.open() as handle:
        body = [line for line in handle if not line.startswith("#")]
    return list(csv.DictReader(body))


def wilson(successes: int, total: int, z: float = 1.959963984540054) -> tuple[float, float]:
    """Two-sided Wilson score interval at nominal 95 % coverage."""
    if total == 0:
        return (float("nan"), float("nan"))
    phat = successes / total
    denom = 1.0 + z * z / total
    centre = (phat + z * z / (2 * total)) / denom
    half = z * math.sqrt(phat * (1 - phat) / total + z * z / (4 * total * total)) / denom
    return (max(0.0, centre - half), min(1.0, centre + half))


def ryser_work(n: int) -> float:
    """Ryser term count times per-term width, the harness projection model."""
    return n * (2.0**n)


def label(row: dict[str, str]) -> str:
    if row["backend"] == "gpu_hip":
        return f"gpu_hip@M={row['batch_size']}"
    return row["backend"]


def main() -> None:
    grid = [r for r in read(GRID) if int(r["q"]) == Q]
    measured = [r for r in grid if r["outcome"] == "measured"]
    by_order: dict[int, dict[str, dict[str, str]]] = {n: {} for n in ORDERS}
    for row in measured:
        by_order[int(row["n"])][label(row)] = row

    print("== section 4: composite throughput, best path per order ==")
    print(f"{'n':>3} {'best overall':>22} {'rate':>12} {'best CPU':>24} {'rate':>12}"
          f" {'best GPU':>16} {'rate':>12} {'GPU/CPU':>8}")
    best_cpu: dict[int, tuple[str, float]] = {}
    best_gpu: dict[int, tuple[str, float]] = {}
    for n in ORDERS:
        cells = by_order[n]
        rates = {k: float(v["composite_matrices_per_s"]) for k, v in cells.items()}
        overall = max(rates, key=rates.get)
        cpu = {k: v for k, v in rates.items() if k.startswith("cpu_")}
        gpu = {k: v for k, v in rates.items() if k.startswith("gpu_hip")}
        ck = max(cpu, key=cpu.get)
        gk = max(gpu, key=gpu.get)
        best_cpu[n] = (ck, cpu[ck])
        best_gpu[n] = (gk, gpu[gk])
        print(f"{n:>3} {overall:>22} {rates[overall]:>12.4f} {ck:>24} {cpu[ck]:>12.4f}"
              f" {gk:>16} {gpu[gk]:>12.4f} {gpu[gk] / cpu[ck]:>8.4f}")

    print()
    print("== section 4: device phase columns on every gpu_hip row ==")
    print(f"{'n':>3} {'M':>5} {'outcome':>9} {'eval_s':>12} {'kernel_device_s':>16}"
          f" {'h2d_device_s':>13} {'d2h_device_s':>13} {'host_subm_s':>12}"
          f" {'dev_subm_to_kern_s':>19} {'residual_s':>11} {'kernel/eval':>11}")
    for row in sorted((r for r in grid if r["backend"] == "gpu_hip"),
                      key=lambda r: (int(r["n"]), int(r["batch_size"]))):
        ev = float(row["eval_s"])
        ke = float(row["kernel_device_s"])
        h2d = float(row["h2d_device_s"])
        d2h = float(row["d2h_device_s"])
        hs = float(row["host_submission_s"])
        ds = float(row["device_submission_to_kernel_s"])
        print(f"{row['n']:>3} {row['batch_size']:>5} {row['outcome']:>9} {ev:>12.6f} {ke:>16.6f}"
              f" {h2d:>13.6f} {d2h:>13.6f} {hs:>12.6f} {ds:>19.6f}"
              f" {ev - ke - h2d - d2h - ds:>11.6f} {ke / ev:>11.4f}")

    print()
    print("== section 4: kernel-only against end-to-end throughput, gpu_hip rows ==")
    print(f"{'n':>3} {'M':>5} {'outcome':>9} {'matrices':>9} {'kernel_only/s':>14}"
          f" {'eval_only/s':>14} {'composite/s':>14} {'kernel/composite':>17}")
    for row in sorted((r for r in grid if r["backend"] == "gpu_hip"),
                      key=lambda r: (int(r["n"]), int(r["batch_size"]))):
        matrices = int(row["matrices"])
        if row["outcome"] != "measured":
            # A censored cell carries no throughput value, kernel-only included.
            print(f"{row['n']:>3} {row['batch_size']:>5} {row['outcome']:>9} {matrices:>9}"
                  f" {'withheld':>14} {'NaN':>14} {'NaN':>14} {'-':>17}")
            continue
        kernel_only = matrices / float(row["kernel_device_s"])
        comp = float(row["composite_matrices_per_s"])
        ev = float(row["eval_matrices_per_s"])
        print(f"{row['n']:>3} {row['batch_size']:>5} {row['outcome']:>9} {matrices:>9}"
              f" {kernel_only:>14.4f} {ev:>14.4f} {comp:>14.4f}"
              f" {kernel_only / comp:>17.4f}")

    print()
    print("== section 4: per-launch and per-matrix device cost on gpu_hip rows ==")
    print(f"{'n':>3} {'M':>5} {'reps':>5} {'launch_host_us':>15} {'launch_dev_us':>14}"
          f" {'kernel_ms':>12} {'h2d_us':>9} {'d2h_us':>9}")
    for row in sorted((r for r in grid if r["backend"] == "gpu_hip"),
                      key=lambda r: (int(r["n"]), int(r["batch_size"]))):
        reps = int(row["reps"])
        print(f"{row['n']:>3} {row['batch_size']:>5} {reps:>5}"
              f" {1e6 * float(row['host_submission_s']) / reps:>15.4f}"
              f" {1e6 * float(row['device_submission_to_kernel_s']) / reps:>14.4f}"
              f" {1e3 * float(row['kernel_device_s']) / reps:>12.4f}"
              f" {1e6 * float(row['h2d_device_s']) / reps:>9.4f}"
              f" {1e6 * float(row['d2h_device_s']) / reps:>9.4f}")

    print()
    print("== section 5: censored cells ==")
    for row in grid:
        if row["outcome"] != "censored":
            continue
        print(f"n={row['n']} {label(row)} reps={row['reps']} matrices={row['matrices']}")
        print(f"  total_s={row['total_s']} rep_min_s={row['rep_min_s']} rep_max_s={row['rep_max_s']}")
        print(f"  composite_matrices_per_s={row['composite_matrices_per_s']}"
              f" eval_matrices_per_s={row['eval_matrices_per_s']}")
        print(f"  projected_matrices_per_s={row['projected_matrices_per_s']}"
              f" projection_reference_n={row['projection_reference_n']}")
        print(f"  note={row['note']}")
        ref_n = int(row["projection_reference_n"])
        ref = by_order[ref_n][label(row)]
        ref_rate = float(ref["composite_matrices_per_s"])
        scaled = ref_rate * ryser_work(ref_n) / ryser_work(int(row["n"]))
        print(f"  reference cell n={ref_n} rate={ref_rate:.6f};"
              f" Ryser-model rescale = {scaled:.6f}")

    print()
    print("== section 5: projection accuracy on this file's own q=3 GPU chain ==")
    print(f"{'M':>5} {'step':>12} {'projection':>12} {'measured':>12} {'error':>9}")
    for m in ("256", "1024"):
        key = f"gpu_hip@M={m}"
        for lo, hi in zip(ORDERS, ORDERS[1:]):
            if key not in by_order[lo] or key not in by_order[hi]:
                continue
            cell = by_order[hi][key]
            if cell["outcome"] != "measured":
                continue
            proj = float(cell["projected_matrices_per_s"])
            meas = float(cell["composite_matrices_per_s"])
            print(f"{m:>5} {f'{lo}->{hi}':>12} {proj:>12.4f} {meas:>12.4f}"
                  f" {100.0 * (proj - meas) / meas:>8.1f}%")

    print()
    print("== section 6: Gray-update isolation (q=3, n=12) ==")
    for row in read(GRAY):
        if int(row["q"]) != Q:
            continue
        print(f"{row['backend']:>28} {row['outcome']:>10} steps={row['steps']} reps={row['reps']}"
              f" update_s={row['update_s'] or '-'} baseline_s={row['compiler_barrier_baseline_s'] or '-'}"
              f" net={row['net_per_operation_s'] or '-'} basis={row['duration_basis']}")

    print()
    print("== section 6: horizontal-product isolation (q=3, n=12) ==")
    for row in read(HPROD):
        if int(row["q"]) != Q or row["outcome"] != "measured":
            continue
        print(f"{row['backend']}: outcome={row['outcome']} reps={row['reps']}")
        print(f"  zero_fast_s={row['zero_fast_s']}"
              f" baseline={row['zero_fast_compiler_barrier_baseline_s']}"
              f" net_per_operation_s={row['zero_fast_net_per_operation_s']}"
              f" timed_operations={row['zero_fast_timed_operations']}")
        print(f"  nonzero_slow_s={row['nonzero_slow_s']}"
              f" baseline={row['nonzero_slow_compiler_barrier_baseline_s']}"
              f" net_per_operation_s={row['nonzero_slow_net_per_operation_s'] or 'ABSENT'}"
              f" timed_operations={row['nonzero_slow_timed_operations']}")

    print()
    print("== section 9: zero fast path frequency, exact expectation, Wilson 95 % ==")
    for row in read(HPROD):
        if int(row["q"]) != Q:
            continue
        n = int(row["n"])
        total = int(row["zero_fast_observed_denominator"])
        zeros = int(row["zero_fast_observed_numerator"])
        slow = int(row["nonzero_slow_observed_numerator"])
        exp_slow = ((Q - 1) / Q) ** n
        exp_zero = 1.0 - exp_slow
        lo, hi = wilson(zeros, total)
        slo, shi = wilson(slow, total)
        print(f"n={n} samples={total}")
        print(f"  zero fast:    observed {zeros}/{total} = {zeros / total:.9f}"
              f"  expected {exp_zero:.9f}  Wilson [{lo:.9f}, {hi:.9f}]"
              f"  covers={lo <= exp_zero <= hi}")
        print(f"  nonzero slow: observed {slow}/{total} = {slow / total:.9f}"
              f"  expected {exp_slow:.9f}  Wilson [{slo:.9f}, {shi:.9f}]"
              f"  covers={slo <= exp_slow <= shi}")
        print(f"  complement check: expectations sum to {exp_zero + exp_slow:.15f};"
              f" observations sum to {(zeros + slow) / total:.15f}")
        break

    print()
    print("== section 9.1: zero fast path share across the three fields ==")
    print(f"{'q':>3} {'n':>3} {'zero_fast':>13} {'nonzero_slow':>13}")
    for field in (3, 5, 7):
        for n in (min(ORDERS), max(ORDERS)):
            slow = ((field - 1) / field) ** n
            print(f"{field:>3} {n:>3} {1.0 - slow:>13.9f} {slow:>13.9f}")

    print()
    print("== section 9.2: permanent-zero fraction pooled per order, Wilson 95 % ==")
    print(f"{'n':>3} {'zeros':>10} {'matrices':>10} {'fraction':>11} {'wilson_lo':>11} {'wilson_hi':>11}")
    for n in ORDERS:
        zeros = sum(int(r["zeros"]) for r in measured if int(r["n"]) == n)
        total = sum(int(r["matrices"]) for r in measured if int(r["n"]) == n)
        lo, hi = wilson(zeros, total)
        print(f"{n:>3} {zeros:>10} {total:>10} {zeros / total:>11.6f} {lo:>11.6f} {hi:>11.6f}")

    print()
    print("== section 11: prior-figure checks ==")
    for n in (24, 28):
        gpu256 = by_order[n].get("gpu_hip@M=256")
        avx2 = by_order[n].get("cpu_avx2")
        if gpu256 is None or avx2 is None:
            continue
        g = float(gpu256["composite_matrices_per_s"])
        a = float(avx2["composite_matrices_per_s"])
        ck, cr = best_cpu[n]
        print(f"n={n}: gpu_hip@M=256 {g:.4f} / cpu_avx2 {a:.4f} = {g / a:.2f}x;"
              f" against best CPU {ck} {cr:.4f} = {g / cr:.4f}x")
    for n in ORDERS:
        gk, gr = best_gpu[n]
        intra = by_order[n].get("cpu_rayon_intra_matrix")
        if intra is None:
            continue
        ir = float(intra["composite_matrices_per_s"])
        print(f"n={n}: {gk} {gr:.4f} / cpu_rayon_intra_matrix {ir:.4f} = {gr / ir:.4f}x")

    # Quoted from the published table in
    # `dev/studies/b488f02c/feasibility-study.md` section 4.4, q=3 rows. The
    # copy exists so the agreement percentages below are reproducible from this
    # script alone; that table remains the source of truth for its own figures.
    prior = {
        12: {"cpu_scalar": 50846.0, "cpu_avx2": 18182.0, "cpu_rayon_batch_scalar": 280056.0,
             "cpu_rayon_intra_matrix": 38462.0, "cpu_ryser_generic": 6902.0,
             "gpu_hip@M=256": 218275.0, "gpu_hip@M=1024": 247646.0},
        16: {"cpu_scalar": 3638.0, "cpu_avx2": 1196.0, "cpu_rayon_batch_scalar": 36311.0,
             "cpu_rayon_intra_matrix": 4439.0, "cpu_ryser_generic": 307.8,
             "gpu_hip@M=256": 30210.0, "gpu_hip@M=1024": 61306.0},
        20: {"cpu_scalar": 229.9, "cpu_avx2": 74.82, "cpu_rayon_batch_scalar": 2500.0,
             "cpu_rayon_intra_matrix": 2982.0, "cpu_ryser_generic": 15.18,
             "gpu_hip@M=256": 2136.0, "gpu_hip@M=1024": 4863.0},
        24: {"cpu_scalar": 14.35, "cpu_avx2": 4.650, "cpu_rayon_batch_scalar": 155.4,
             "cpu_rayon_intra_matrix": 296.6, "cpu_ryser_generic": 0.777,
             "gpu_hip@M=256": 136.4, "gpu_hip@M=1024": 310.4},
        28: {"cpu_scalar": 0.903, "cpu_avx2": 0.289, "cpu_rayon_batch_scalar": 9.986,
             "cpu_rayon_intra_matrix": 19.58, "cpu_ryser_generic": 0.0419,
             "gpu_hip@M=256": 8.532, "gpu_hip@M=1024": 19.27},
    }
    print()
    print("== section 11: agreement with feasibility-study section 4.4 ==")
    print(f"{'n':>3} {'path':>24} {'study 4.4':>12} {'this run':>12} {'delta':>8}")
    spread = []
    for n in ORDERS:
        for key, ref in prior[n].items():
            cell = by_order[n].get(key)
            if cell is None:
                print(f"{n:>3} {key:>24} {ref:>12.4f} {'censored':>12} {'-':>8}")
                continue
            here = float(cell["composite_matrices_per_s"])
            delta = 100.0 * (here - ref) / ref
            spread.append(abs(delta))
            print(f"{n:>3} {key:>24} {ref:>12.4f} {here:>12.4f} {delta:>7.2f}%")
    spread.sort()
    print(f"pairs={len(spread)} median|delta|={spread[len(spread) // 2]:.2f}%"
          f" max|delta|={spread[-1]:.2f}%")

    print()
    print("== section 3: equivalence verdicts for q=3 ==")
    for row in read(EQUIV):
        if int(row["q"]) != Q:
            continue
        print(f"n={row['n']:>3} reference={row['reference']:>12} backend={row['backend']:>28}"
              f" matrices={row['matrices']:>4} mismatches={row['mismatches']:>2}"
              f" zeros_ref={row['zeros_reference']:>4} zeros_backend={row['zeros_backend']:>4}"
              f" status={row['status']}")


if __name__ == "__main__":
    main()
