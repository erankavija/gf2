#!/usr/bin/env python3
"""Derive every table in `receipts.md` from the committed campaign artifacts.

Run from the repository root:

    python3 dev/studies/047b62ed/analysis.py

The script reads only committed artifacts and writes nothing: the four CSVs of
campaign run `20260814T230032Z-2085453` beside it, and the compiler
kernel-resource logs of receipt `20260814T172506Z-1610002` under
`dev/studies/6c7fcb38/`. Every figure quoted in `receipts.md` is printed here
under the heading that names its section, so a reviewer can diff prose against
data without re-running the measurement.
"""

from __future__ import annotations

import math
import pathlib

STUDY = pathlib.Path(__file__).resolve().parent
RUN = "permanent-campaign-20260814T230032Z-2085453"
GRID = STUDY / f"{RUN}-q3-grid.csv"
GRAY = STUDY / f"{RUN}-q3-gray-update.csv"
HPROD = STUDY / f"{RUN}-q3-horizontal-product.csv"
EQUIV = STUDY / f"{RUN}-shared-equivalence.csv"
RESOURCES = STUDY.parent / "6c7fcb38" / "hip-resource-usage-20260814T172506Z-1610002"
RESOURCE_LOGS = (
    "permanent_bipedal3.hip.resource.log",
    "wave_gf3_equivalence.hip.resource.log",
    "gray_update_micro.hip.resource.log",
    "permanent_bipedal7.hip.resource.log",
    "probe.hip.resource.log",
)

Q = 3
ORDERS = [12, 16, 20, 24, 28]
CPU_PATHS = (
    "cpu_scalar",
    "cpu_avx2",
    "cpu_rayon_batch_scalar",
    "cpu_rayon_batch_avx2",
    "cpu_rayon_intra_matrix",
    "cpu_ryser_generic",
)
PROTOTYPES = ("wave-gf3", "fold-gf3")
# The grid writer emits `phase_timing_note` and `note` unquoted, and both carry
# literal commas on the out-of-field rows, so those lines hold more fields than
# the header names. `seed_root` is the one column whose value has a fixed
# prefix, so it anchors the split: everything between `phase_timing_note` and
# the eleven columns that precede `seed_root` belongs to the note, and
# everything after `order_index` belongs to the trailing note.
GRID_SEED_ROOT_COLUMN = "seed_root"
GRID_NOTE_COLUMN = "phase_timing_note"


def read_simple(path: pathlib.Path) -> list[dict[str, str]]:
    """Read a CSV whose only free-text column is the last one."""
    with path.open() as handle:
        body = [line for line in handle if not line.startswith("#")]
    header = body[0].rstrip("\n").split(",")
    rows = []
    for line in body[1:]:
        fields = line.rstrip("\n").split(",")
        if len(fields) > len(header):
            fields = fields[: len(header) - 1] + [",".join(fields[len(header) - 1 :])]
        rows.append(dict(zip(header, fields)))
    return rows


def read_grid(path: pathlib.Path) -> list[dict[str, str]]:
    """Read the grid CSV, which has two unquoted free-text columns."""
    with path.open() as handle:
        body = [line for line in handle if not line.startswith("#")]
    header = body[0].rstrip("\n").split(",")
    note_at = header.index(GRID_NOTE_COLUMN)
    seed_at = header.index(GRID_SEED_ROOT_COLUMN)
    tail_columns = len(header) - seed_at - 1
    rows = []
    for line in body[1:]:
        fields = line.rstrip("\n").split(",")
        if len(fields) == len(header):
            rows.append(dict(zip(header, fields)))
            continue
        anchor = next(
            i for i, value in enumerate(fields) if value.startswith("0xb488f02c")
        )
        head = fields[:note_at]
        note = ",".join(fields[note_at : anchor - (seed_at - note_at - 1)])
        middle = fields[anchor - (seed_at - note_at - 1) : anchor + tail_columns]
        trailing = ",".join(fields[anchor + tail_columns :])
        values = head + [note] + middle + [trailing]
        assert len(values) == len(header), line
        rows.append(dict(zip(header, values)))
    return rows


def read_resource_log(path: pathlib.Path) -> list[dict[str, str]]:
    """Parse one `-Rpass-analysis=kernel-resource-usage` stderr capture.

    Each kernel opens with a `Function Name:` remark and is followed by its
    named resource remarks. The line number of the opening remark is kept so a
    citation can point at the exact line of the committed log.
    """
    kernels: list[dict[str, str]] = []
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        if "remark:" not in line:
            continue
        body = line.split("remark:", 1)[1].split("[-Rpass-analysis")[0].strip()
        if body.startswith("Function Name:"):
            kernels.append({
                "kernel": body.split(":", 1)[1].strip(),
                "log": f"{path.name}:{number}",
            })
        elif kernels and ":" in body:
            key, value = body.split(":", 1)
            kernels[-1][key.strip()] = value.strip()
    return kernels


def wilson(successes: int, total: int, z: float = 1.959963984540054) -> tuple[float, float]:
    """Two-sided Wilson score interval at nominal 95 % coverage.

    `z` is the standard normal 0.975 quantile. The interval is the exact
    solution of |p_hat - p| = z * sqrt(p (1 - p) / total), which stays inside
    [0, 1] and remains defined at zero successes, where a Wald interval
    degenerates to a point.
    """
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
    grid = [r for r in read_grid(GRID) if int(r["q"]) == Q]
    measured = [r for r in grid if r["outcome"] == "measured"]
    by_order: dict[int, dict[str, dict[str, str]]] = {n: {} for n in ORDERS}
    for row in measured:
        by_order[int(row["n"])][label(row)] = row
    device_rows = sorted(
        (r for r in grid if r["backend"] in ("gpu_hip",) + PROTOTYPES),
        key=lambda r: (int(r["n"]), r["backend"], int(r["batch_size"])),
    )

    print("== section 2: grid outcome counts ==")
    outcomes: dict[str, int] = {}
    for row in grid:
        outcomes[row["outcome"]] = outcomes.get(row["outcome"], 0) + 1
    print(f"cells={len(grid)} " + " ".join(f"{k}={v}" for k, v in sorted(outcomes.items())))
    print("non-measured cells, one line per distinct (backend, outcome, reason):")
    seen: set[tuple[str, str, str]] = set()
    for row in grid:
        if row["outcome"] == "measured":
            continue
        key = (row["backend"], row["outcome"], row["note"])
        if key in seen:
            continue
        seen.add(key)
        orders = sorted(
            int(r["n"])
            for r in grid
            if (r["backend"], r["outcome"], r["note"]) == key
        )
        print(f"  {row['backend']} [{row['outcome']}] n={orders}")
        print(f"    {row['note']}")

    print()
    print("== section 3: equivalence verdicts for q=3 ==")
    equiv = [r for r in read_simple(EQUIV) if int(r["q"]) == Q]
    identical = [r for r in equiv if r["status"].startswith("identical")]
    print(f"q=3 rows={len(equiv)} identical={len(identical)}"
          f" unsupported={len(equiv) - len(identical)}")
    print(f"all-fields identical rows in this file="
          f"{sum(1 for r in read_simple(EQUIV) if r['status'].startswith('identical'))}")
    for row in equiv:
        print(f"n={row['n']:>3} reference={row['reference']:>12} backend={row['backend']:>28}"
              f" matrices={row['matrices']:>4} mismatches={row['mismatches']:>2}"
              f" zeros_ref={row['zeros_reference']:>4} zeros_backend={row['zeros_backend']:>4}"
              f" status={row['status'][:60]}")

    print()
    print("== section 4.2: composite throughput, best path per order ==")
    print(f"{'n':>3} {'best overall':>24} {'rate':>12} {'best CPU':>24} {'rate':>12}"
          f" {'best GPU':>16} {'rate':>12} {'best prototype':>16} {'rate':>12}"
          f" {'GPU/CPU':>8} {'proto/CPU':>10} {'proto/GPU':>10}")
    best_cpu: dict[int, tuple[str, float]] = {}
    best_gpu: dict[int, tuple[str, float]] = {}
    best_proto: dict[int, tuple[str, float]] = {}
    for n in ORDERS:
        cells = by_order[n]
        rates = {k: float(v["composite_matrices_per_s"]) for k, v in cells.items()}
        overall = max(rates, key=rates.get)
        cpu = {k: v for k, v in rates.items() if k in CPU_PATHS}
        gpu = {k: v for k, v in rates.items() if k.startswith("gpu_hip")}
        proto = {k: v for k, v in rates.items() if k in PROTOTYPES}
        ck = max(cpu, key=cpu.get)
        gk = max(gpu, key=gpu.get)
        pk = max(proto, key=proto.get)
        best_cpu[n] = (ck, cpu[ck])
        best_gpu[n] = (gk, gpu[gk])
        best_proto[n] = (pk, proto[pk])
        print(f"{n:>3} {overall:>24} {rates[overall]:>12.4f} {ck:>24} {cpu[ck]:>12.4f}"
              f" {gk:>16} {gpu[gk]:>12.4f} {pk:>16} {proto[pk]:>12.4f}"
              f" {gpu[gk] / cpu[ck]:>8.4f} {proto[pk] / cpu[ck]:>10.4f}"
              f" {proto[pk] / gpu[gk]:>10.4f}")

    print()
    print("== section 4.2: fold against halving control, and against the shipped path ==")
    print(f"{'n':>3} {'fold-gf3':>13} {'wave-gf3':>13} {'fold/wave':>10}"
          f" {'best gpu_hip':>14} {'fold/gpu_hip':>13}")
    for n in ORDERS:
        fold = float(by_order[n]["fold-gf3"]["composite_matrices_per_s"])
        wave = float(by_order[n]["wave-gf3"]["composite_matrices_per_s"])
        gk, gr = best_gpu[n]
        print(f"{n:>3} {fold:>13.4f} {wave:>13.4f} {fold / wave:>10.4f}"
              f" {gr:>14.4f} {fold / gr:>13.4f}  ({gk})")

    print()
    print("== section 4.2: every measured cell, composite rate ==")
    for n in ORDERS:
        for key, row in sorted(by_order[n].items(), key=lambda kv: -float(kv[1]["composite_matrices_per_s"])):
            print(f"n={n:>3} {key:>24} M={row['batch_size']:>6} reps={row['reps']:>6}"
                  f" matrices={row['matrices']:>8}"
                  f" composite={float(row['composite_matrices_per_s']):>13.4f}"
                  f" eval_only={float(row['eval_matrices_per_s']):>13.4f}"
                  f" rep_sd_s={row['rep_sd_s']}")

    print()
    print("== section 4.3: device phase columns on every device-backed row ==")
    print(f"{'n':>3} {'path':>10} {'M':>5} {'outcome':>9} {'eval_s':>12} {'kernel_device_s':>16}"
          f" {'h2d_device_s':>13} {'d2h_device_s':>13} {'host_subm_s':>12}"
          f" {'dev_subm_to_kern_s':>19} {'residual_s':>11} {'kernel/eval':>11}"
          f" {'host_subm/eval':>14}")
    for row in device_rows:
        ev = float(row["eval_s"])
        ke = float(row["kernel_device_s"])
        h2d = float(row["h2d_device_s"])
        d2h = float(row["d2h_device_s"])
        hs = float(row["host_submission_s"])
        ds = float(row["device_submission_to_kernel_s"])
        print(f"{row['n']:>3} {row['backend']:>10} {row['batch_size']:>5} {row['outcome']:>9}"
              f" {ev:>12.6f} {ke:>16.6f}"
              f" {h2d:>13.6f} {d2h:>13.6f} {hs:>12.6f} {ds:>19.6f}"
              f" {ev - ke - h2d - d2h - ds:>11.6f} {ke / ev:>11.4f} {hs / ev:>14.4f}")

    print()
    print("== section 4.3: kernel-only against end-to-end throughput ==")
    print(f"{'n':>3} {'path':>10} {'M':>5} {'outcome':>9} {'matrices':>9} {'kernel_only/s':>14}"
          f" {'eval_only/s':>14} {'composite/s':>14} {'kernel/composite':>17}")
    for row in device_rows:
        matrices = int(row["matrices"])
        if row["outcome"] != "measured":
            # A censored cell carries no throughput value, kernel-only included.
            print(f"{row['n']:>3} {row['backend']:>10} {row['batch_size']:>5} {row['outcome']:>9}"
                  f" {matrices:>9} {'withheld':>14} {'NaN':>14} {'NaN':>14} {'-':>17}")
            continue
        kernel_only = matrices / float(row["kernel_device_s"])
        comp = float(row["composite_matrices_per_s"])
        ev = float(row["eval_matrices_per_s"])
        print(f"{row['n']:>3} {row['backend']:>10} {row['batch_size']:>5} {row['outcome']:>9}"
              f" {matrices:>9} {kernel_only:>14.4f} {ev:>14.4f} {comp:>14.4f}"
              f" {kernel_only / comp:>17.4f}")

    print()
    print("== section 4.3: per-launch device cost ==")
    print(f"{'n':>3} {'path':>10} {'M':>5} {'reps':>6} {'launch_host_us':>15}"
          f" {'launch_dev_us':>14} {'kernel_ms':>12} {'h2d_us':>9} {'d2h_us':>9}")
    for row in device_rows:
        reps = int(row["reps"])
        print(f"{row['n']:>3} {row['backend']:>10} {row['batch_size']:>5} {reps:>6}"
              f" {1e6 * float(row['host_submission_s']) / reps:>15.4f}"
              f" {1e6 * float(row['device_submission_to_kernel_s']) / reps:>14.4f}"
              f" {1e3 * float(row['kernel_device_s']) / reps:>12.4f}"
              f" {1e6 * float(row['h2d_device_s']) / reps:>9.4f}"
              f" {1e6 * float(row['d2h_device_s']) / reps:>9.4f}")

    print()
    print("== section 4.4: best operating point of each device path ==")
    for family, chooser in (
        ("prototype", best_proto),
        ("shipped gpu_hip", best_gpu),
    ):
        ratios = {n: chooser[n][1] / best_cpu[n][1] for n in ORDERS}
        peak_ratio_n = max(ratios, key=ratios.get)
        peak_rate_n = max(ORDERS, key=lambda n: chooser[n][1])
        for tag, n in (("max ratio vs best CPU", peak_ratio_n), ("max absolute rate", peak_rate_n)):
            key, rate = chooser[n]
            row = by_order[n][key]
            reps = int(row["reps"])
            print(f"{family:>16} [{tag:>21}] n={n} {key} M={row['batch_size']}"
                  f" rate={rate:.4f} best_cpu={best_cpu[n][0]}@{best_cpu[n][1]:.4f}"
                  f" ratio={rate / best_cpu[n][1]:.4f}")
            print(f"{'':>16}   reps={reps}"
                  f" host_submission_s={row['host_submission_s']}"
                  f" ({1e6 * float(row['host_submission_s']) / reps:.4f} us/launch)"
                  f" device_submission_to_kernel_s={row['device_submission_to_kernel_s']}"
                  f" ({1e6 * float(row['device_submission_to_kernel_s']) / reps:.4f} us/launch)")

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
    print("== section 5: projection accuracy on this file's own q=3 device chains ==")
    print(f"{'path':>18} {'step':>12} {'projection':>12} {'measured':>12} {'error':>9}")
    for key in ("gpu_hip@M=256", "gpu_hip@M=1024", "wave-gf3", "fold-gf3"):
        for lo, hi in zip(ORDERS, ORDERS[1:]):
            if key not in by_order[lo] or key not in by_order[hi]:
                continue
            cell = by_order[hi][key]
            if cell["outcome"] != "measured":
                continue
            proj = float(cell["projected_matrices_per_s"])
            meas = float(cell["composite_matrices_per_s"])
            print(f"{key:>18} {f'{lo}->{hi}':>12} {proj:>12.4f} {meas:>12.4f}"
                  f" {100.0 * (proj - meas) / meas:>8.1f}%")

    print()
    print("== section 5: batch calibration, single-matrix probe against achieved cost ==")
    print(f"{'n':>3} {'path':>24} {'M':>6} {'probe_matrix_s':>15} {'eval_s/matrix':>15}"
          f" {'probe/achieved':>15} {'batch_source':>14}")
    for n in ORDERS:
        for key, row in sorted(by_order[n].items()):
            probe = float(row["probe_matrix_s"])
            achieved = float(row["eval_s"]) / int(row["matrices"])
            source = "fixed" if row["backend"] == "gpu_hip" else "calibrated"
            ratio = probe / achieved if probe == probe else float("nan")
            print(f"{n:>3} {key:>24} {row['batch_size']:>6} {probe:>15.6f} {achieved:>15.9f}"
                  f" {ratio:>15.1f} {source:>14}")

    print()
    print("== section 6.1: Gray-update isolation, q=3, all orders ==")
    for row in read_simple(GRAY):
        if int(row["q"]) != Q:
            continue
        print(f"n={row['n']:>3} {row['backend']:>28} {row['outcome']:>12}"
              f" steps={row['steps']} reps={row['reps']}"
              f" update_s={row['update_s'] or '-'}"
              f" baseline_s={row['compiler_barrier_baseline_s'] or '-'}"
              f" net={row['net_per_operation_s'] or 'ABSENT'} basis={row['duration_basis']}")
    print("distinct notes:")
    seen_notes: set[str] = set()
    for row in read_simple(GRAY):
        if int(row["q"]) != Q or row["note"] in seen_notes:
            continue
        seen_notes.add(row["note"])
        print(f"  {row['backend']}: {row['note']}")

    print()
    print("== section 6.2: horizontal-product isolation, q=3, all orders ==")
    for row in read_simple(HPROD):
        if int(row["q"]) != Q or row["outcome"] != "measured":
            continue
        print(f"n={row['n']} {row['backend']}: outcome={row['outcome']} reps={row['reps']}"
              f" basis={row['duration_basis']}")
        print(f"  zero_fast_s={row['zero_fast_s']}"
              f" baseline={row['zero_fast_compiler_barrier_baseline_s']}"
              f" net_per_operation_s={row['zero_fast_net_per_operation_s'] or 'ABSENT'}"
              f" timed_operations={row['zero_fast_timed_operations']}")
        print(f"  nonzero_slow_s={row['nonzero_slow_s']}"
              f" baseline={row['nonzero_slow_compiler_barrier_baseline_s']}"
              f" net_per_operation_s={row['nonzero_slow_net_per_operation_s'] or 'ABSENT'}"
              f" timed_operations={row['nonzero_slow_timed_operations']}")
    print("unsupported reasons, one line per distinct backend:")
    seen_hp: set[str] = set()
    for row in read_simple(HPROD):
        if int(row["q"]) != Q or row["outcome"] == "measured" or row["backend"] in seen_hp:
            continue
        seen_hp.add(row["backend"])
        reason = row["note"].split("counts; ", 1)[-1]
        print(f"  {row['backend']}: {reason}")

    print()
    print("== section 9: zero fast path frequency, exact expectation, Wilson 95 % ==")
    for row in read_simple(HPROD):
        if int(row["q"]) != Q or row["backend"] != "fold-gf3":
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
              f"  expected {exp_zero:.12f}  Wilson [{lo:.9f}, {hi:.9f}]"
              f"  covers={lo <= exp_zero <= hi}")
        print(f"  nonzero slow: observed {slow}/{total} = {slow / total:.9f}"
              f"  expected {exp_slow:.12f}  Wilson [{slo:.9f}, {shi:.9f}]"
              f"  covers={slo <= exp_slow <= shi}")
        print(f"  csv expected columns: zero_fast={row['zero_fast_expected_frequency']}"
              f" nonzero_slow={row['nonzero_slow_expected_frequency']}")
        print(f"  complement check: expectations sum to {exp_zero + exp_slow:.15f};"
              f" observations sum to {(zeros + slow) / total:.15f}")

    print()
    print("== section 9: branch split of the device-timed operations, Wilson 95 % ==")
    print(f"{'n':>3} {'zero_fast_ops':>15} {'nonzero_slow_ops':>17} {'total':>12}"
          f" {'slow frequency':>15} {'slow expected':>15} {'wilson_lo':>12} {'wilson_hi':>12}")
    for row in read_simple(HPROD):
        if int(row["q"]) != Q or row["backend"] != "fold-gf3":
            continue
        n = int(row["n"])
        fast_ops = int(row["zero_fast_timed_operations"])
        slow_ops = int(row["nonzero_slow_timed_operations"])
        total_ops = fast_ops + slow_ops
        exp_slow = ((Q - 1) / Q) ** n
        lo, hi = wilson(slow_ops, total_ops)
        print(f"{n:>3} {fast_ops:>15} {slow_ops:>17} {total_ops:>12}"
              f" {slow_ops / total_ops:>15.9f} {exp_slow:>15.9f} {lo:>12.9f} {hi:>12.9f}")
        assert total_ops == int(row["reps"]) * int(row["samples_per_rep"]), row

    print()
    print("== section 9: zero fast path share across the three fields ==")
    print(f"{'q':>3} {'n':>3} {'zero_fast':>15} {'nonzero_slow':>15}")
    for field in (3, 5, 7):
        for n in (min(ORDERS), max(ORDERS)):
            slow = ((field - 1) / field) ** n
            print(f"{field:>3} {n:>3} {1.0 - slow:>15.9f} {slow:>15.9f}")

    print()
    print("== section 9: permanent-zero fraction pooled per order, Wilson 95 % ==")
    print(f"{'n':>3} {'zeros':>10} {'matrices':>10} {'fraction':>11} {'wilson_lo':>11} {'wilson_hi':>11}")
    for n in ORDERS:
        zeros = sum(int(r["zeros"]) for r in measured if int(r["n"]) == n)
        total = sum(int(r["matrices"]) for r in measured if int(r["n"]) == n)
        lo, hi = wilson(zeros, total)
        print(f"{n:>3} {zeros:>10} {total:>10} {zeros / total:>11.6f} {lo:>11.6f} {hi:>11.6f}")

    print()
    print("== section 7: compiler kernel-resource remarks ==")
    print(f"{'kernel':>62} {'SGPR':>5} {'VGPR':>5} {'scratch':>8} {'sgpr_sp':>8}"
          f" {'vgpr_sp':>8} {'LDS':>5} {'occ':>4}  log")
    for name in RESOURCE_LOGS:
        for kernel in read_resource_log(RESOURCES / name):
            print(f"{kernel['kernel'][:62]:>62} {kernel['TotalSGPRs']:>5} {kernel['VGPRs']:>5}"
                  f" {kernel['ScratchSize [bytes/lane]']:>8} {kernel['SGPRs Spill']:>8}"
                  f" {kernel['VGPRs Spill']:>8} {kernel['LDS Size [bytes/block]']:>5}"
                  f" {kernel['Occupancy [waves/SIMD]']:>4}  {kernel['log']}")

    print()
    print("== section 10: launch geometry actually resident per repetition ==")
    print(f"{'n':>3} {'path':>10} {'M':>5} {'blocks':>7} {'lanes/block':>12} {'active lanes':>13}")
    for row in device_rows:
        m = int(row["batch_size"])
        lanes = 1 if row["backend"] == "gpu_hip" else min(32, 2 ** int(row["n"]))
        print(f"{row['n']:>3} {row['backend']:>10} {m:>5} {m:>7} {lanes:>12} {m * lanes:>13}")

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


if __name__ == "__main__":
    main()
