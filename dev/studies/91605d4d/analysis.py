#!/usr/bin/env python3
"""Derive every table in `receipts.md` from the committed campaign artifacts.

Run from the repository root:

    python3 dev/studies/91605d4d/analysis.py

The script reads only committed artifacts and writes nothing: the three
$\\mathbb{F}_5$ CSVs of campaign run `20260814T230032Z-2085453` beside it, the
shared equivalence CSV of the same run under `dev/studies/047b62ed/`, the prior
grid under `dev/studies/b488f02c/`, and the compiler kernel-resource logs of
receipt `20260814T172506Z-1610002` under `dev/studies/6c7fcb38/`. Every figure
quoted in `receipts.md` is printed here under the heading that names its
section, so a reviewer can diff prose against data without re-running the
measurement.
"""

from __future__ import annotations

import math
import pathlib

STUDY = pathlib.Path(__file__).resolve().parent
RUN = "permanent-campaign-20260814T230032Z-2085453"
GRID = STUDY / f"{RUN}-q5-grid.csv"
GRAY = STUDY / f"{RUN}-q5-gray-update.csv"
HPROD = STUDY / f"{RUN}-q5-horizontal-product.csv"
# One global equivalence invocation gates all three fields; it is committed
# under the F_3 study directory and this script reads its q=5 rows.
EQUIV = STUDY.parent / "047b62ed" / f"{RUN}-shared-equivalence.csv"
# Prior grid this campaign has to confirm or overturn: the committed artifact
# the feasibility study's section 4.4 table is rendered from, read here rather
# than that table, so no figure of it is maintained in two places.
PRIOR = STUDY.parent / "b488f02c" / "throughput-2026-08-07.csv"
RESOURCES = STUDY.parent / "6c7fcb38" / "hip-resource-usage-20260814T172506Z-1610002"
# The five logs holding the kernels this campaign measures. The two
# horizontal-product kernels appear in the permanent_bipedal7 log because
# horizontal_product_micro.hip is compiled as an included fragment of that
# translation unit (receipt.txt, translation_unit_role: included-fragment).
RESOURCE_LOGS = (
    "permanent_bipedal5.hip.resource.log",
    "f5_wave_equivalence.hip.resource.log",
    "gray_update_micro.hip.resource.log",
    "permanent_bipedal7.hip.resource.log",
    "probe.hip.resource.log",
)

Q = 5
ORDERS = [12, 16, 20, 24, 28]
EQUIV_ORDERS = [8, 12, 16, 20, 24, 28]
CPU_PATHS = (
    "cpu_scalar",
    "cpu_avx2",
    "cpu_rayon_batch_scalar",
    "cpu_rayon_batch_avx2",
    "cpu_rayon_intra_matrix",
    "cpu_ryser_generic",
)
PROTOTYPES = ("f5-byte-control", "f5-three-plane")
# gfx1030 wave width, and the lane count every F_5 prototype block launches at
# the orders this campaign measures: active_lanes_for_order(n) = 32 for n >= 5
# (dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:29-31, :16).
WAVE_LANES = 32
# Archived per-launch work budget for this field, carried as a prior rather
# than as an established device property: sub_batch * 2^n <= 1.3e9 at q=5
# (dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/
# r4_gpu_uniformity_resample.md section 2.5).
ARCHIVED_Q5_WORK_BUDGET = 1.3e9
# The same archived calibration places the hang boundary at ~190-200 s per
# launch and reports bounded sub-batches holding every launch at ~10-117 s.
ARCHIVED_HANG_BOUNDARY_S = 190.0
# The grid writer emits `phase_timing_note` and `note` unquoted, and both carry
# literal commas on the out-of-field rows, so those lines hold more fields than
# the header names. `seed_root` is the one column whose value has a fixed
# prefix, so it anchors the split: everything between `phase_timing_note` and
# the eleven columns that precede `seed_root` belongs to the note, and
# everything after `order_index` belongs to the trailing note.
GRID_SEED_ROOT_COLUMN = "seed_root"
GRID_NOTE_COLUMN = "phase_timing_note"
GRID_SEED_ROOT_PREFIX = "0xb488f02c"


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
            i
            for i, value in enumerate(fields)
            if value.startswith(GRID_SEED_ROOT_PREFIX)
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


def occupancy_model(vgprs: int) -> int:
    """Waves/SIMD predicted from the per-lane vector-register count alone.

    A fit to the receipt rather than a quoted hardware constant: a 1024-entry
    per-SIMD vector-register budget allocated in units of 16, capped at the
    wave-slot ceiling the empty probe kernel reports. Section 8 checks it
    against every kernel entry in the receipt and prints any mismatch.
    """
    if vgprs <= 0:
        return 16
    allocated = 16 * math.ceil(vgprs / 16)
    return min(16, 1024 // allocated)


def prototype_shared_bytes(backend: str, n: int) -> int:
    """Dynamic shared bytes the launch requests per block, from the source.

    `n * n` for the byte control's staged matrix and `3 * n * 8` for the
    three-plane path's staged column planes
    (dev/research/permanent_wave_gpu/hip/f5_wave_equivalence.hip:727-737).
    """
    if backend == "f5-byte-control":
        return n * n
    if backend == "f5-three-plane":
        return 3 * n * 8
    return 0


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

    print("== section 1: reserved stream block for execution id 5002 ==")
    execution_id = 5002
    first = execution_id * 22_500_000 + 1
    print(f"execution_id={execution_id} first={first} last={first + 22_500_000 - 1}")

    print()
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
    print("== section 3: equivalence verdicts for q=5 ==")
    equiv_all = read_simple(EQUIV)
    equiv = [r for r in equiv_all if int(r["q"]) == Q]
    identical = [r for r in equiv if r["status"].startswith("identical")]
    print(f"q=5 rows={len(equiv)} identical={len(identical)}"
          f" unsupported={len(equiv) - len(identical)}")
    print(f"all-fields identical rows in this file="
          f"{sum(1 for r in equiv_all if r['status'].startswith('identical'))}")
    for row in equiv:
        print(f"n={row['n']:>3} reference={row['reference']:>12} backend={row['backend']:>28}"
              f" matrices={row['matrices']:>4} mismatches={row['mismatches']:>2}"
              f" zeros_ref={row['zeros_reference']:>4} zeros_backend={row['zeros_backend']:>4}"
              f" status={row['status'][:60]}")
    print("per-order identical backend counts and matrix counts:")
    for n in EQUIV_ORDERS:
        cells = [r for r in equiv if int(r["n"]) == n and r["status"].startswith("identical")]
        counts = {int(r["matrices"]) for r in cells}
        zeros = {int(r["zeros_reference"]) for r in cells}
        print(f"  n={n:>3} identical_backends={len(cells)} matrices={sorted(counts)}"
              f" zeros_reference={sorted(zeros)}"
              f" mismatch_total={sum(int(r['mismatches']) for r in cells)}")

    print()
    print("== section 4.2: composite throughput, best path per order ==")
    print(f"{'n':>3} {'best overall':>24} {'rate':>14} {'best CPU':>24} {'rate':>14}"
          f" {'best prototype':>16} {'rate':>14} {'best GPU':>16} {'rate':>14}"
          f" {'proto/CPU':>10} {'GPU/CPU':>9}")
    best_cpu: dict[int, tuple[str, float]] = {}
    best_gpu: dict[int, tuple[str, float] | None] = {}
    best_proto: dict[int, tuple[str, float]] = {}
    for n in ORDERS:
        cells = by_order[n]
        rates = {k: float(v["composite_matrices_per_s"]) for k, v in cells.items()}
        overall = max(rates, key=rates.get)
        cpu = {k: v for k, v in rates.items() if k in CPU_PATHS}
        gpu = {k: v for k, v in rates.items() if k.startswith("gpu_hip")}
        proto = {k: v for k, v in rates.items() if k in PROTOTYPES}
        ck = max(cpu, key=cpu.get)
        pk = max(proto, key=proto.get)
        best_cpu[n] = (ck, cpu[ck])
        best_proto[n] = (pk, proto[pk])
        if gpu:
            gk = max(gpu, key=gpu.get)
            best_gpu[n] = (gk, gpu[gk])
            gpu_cell = f"{gk:>16} {gpu[gk]:>14.4f}"
            gpu_ratio = f"{gpu[gk] / cpu[ck]:>9.4f}"
        else:
            best_gpu[n] = None
            gpu_cell = f"{'all censored':>16} {'-':>14}"
            gpu_ratio = f"{'-':>9}"
        print(f"{n:>3} {overall:>24} {rates[overall]:>14.4f} {ck:>24} {cpu[ck]:>14.4f}"
              f" {pk:>16} {proto[pk]:>14.4f} {gpu_cell}"
              f" {proto[pk] / cpu[ck]:>10.4f} {gpu_ratio}")

    print()
    print("== section 4.2: three-plane against the byte control, and against the shipped path ==")
    print(f"{'n':>3} {'f5-three-plane':>15} {'f5-byte-control':>16} {'plane/byte':>11}"
          f" {'best gpu_hip':>14} {'plane/gpu_hip':>14}")
    for n in ORDERS:
        plane = float(by_order[n]["f5-three-plane"]["composite_matrices_per_s"])
        byte_cell = by_order[n].get("f5-byte-control")
        byte_text = f"{float(byte_cell['composite_matrices_per_s']):>16.4f}" if byte_cell else f"{'censored':>16}"
        ratio_text = (
            f"{plane / float(byte_cell['composite_matrices_per_s']):>11.4f}"
            if byte_cell else f"{'-':>11}"
        )
        gpu = best_gpu[n]
        gpu_text = f"{gpu[1]:>14.4f}" if gpu else f"{'all censored':>14}"
        gpu_ratio = f"{plane / gpu[1]:>14.4f}  ({gpu[0]})" if gpu else f"{'-':>14}"
        print(f"{n:>3} {plane:>15.4f} {byte_text} {ratio_text} {gpu_text} {gpu_ratio}")

    print()
    print("== section 4.2: every measured cell, composite rate ==")
    for n in ORDERS:
        for key, row in sorted(
            by_order[n].items(), key=lambda kv: -float(kv[1]["composite_matrices_per_s"])
        ):
            print(f"n={n:>3} {key:>24} M={row['batch_size']:>6} reps={row['reps']:>6}"
                  f" matrices={row['matrices']:>8}"
                  f" composite={float(row['composite_matrices_per_s']):>14.4f}"
                  f" eval_only={float(row['eval_matrices_per_s']):>14.4f}"
                  f" rep_sd_s={row['rep_sd_s']}")

    print()
    print("== section 4.3: device phase columns on every device-backed row ==")
    print(f"{'n':>3} {'path':>16} {'M':>5} {'outcome':>9} {'eval_s':>13} {'kernel_device_s':>16}"
          f" {'h2d_device_s':>13} {'d2h_device_s':>13} {'host_subm_s':>13}"
          f" {'dev_subm_to_kern_s':>19} {'residual_s':>11} {'kernel/eval':>11}"
          f" {'host_subm/eval':>14}")
    for row in device_rows:
        if not row["kernel_device_s"]:
            print(f"{row['n']:>3} {row['backend']:>16} {row['batch_size']:>5} {row['outcome']:>9}"
                  f"   phase columns empty: {row['phase_timing_note']}")
            continue
        ev = float(row["eval_s"])
        ke = float(row["kernel_device_s"])
        h2d = float(row["h2d_device_s"])
        d2h = float(row["d2h_device_s"])
        hs = float(row["host_submission_s"])
        ds = float(row["device_submission_to_kernel_s"])
        print(f"{row['n']:>3} {row['backend']:>16} {row['batch_size']:>5} {row['outcome']:>9}"
              f" {ev:>13.6f} {ke:>16.6f}"
              f" {h2d:>13.6f} {d2h:>13.6f} {hs:>13.6f} {ds:>19.6f}"
              f" {ev - ke - h2d - d2h - ds:>11.6f} {ke / ev:>11.4f} {hs / ev:>14.4f}")

    print()
    print("== section 4.3: kernel-only against end-to-end throughput ==")
    print(f"{'n':>3} {'path':>16} {'M':>5} {'outcome':>9} {'matrices':>9} {'kernel_only/s':>14}"
          f" {'eval_only/s':>14} {'composite/s':>14} {'kernel/composite':>17}")
    for row in device_rows:
        matrices = int(row["matrices"])
        if row["outcome"] != "measured":
            # A censored cell carries no throughput value, kernel-only included.
            print(f"{row['n']:>3} {row['backend']:>16} {row['batch_size']:>5} {row['outcome']:>9}"
                  f" {matrices:>9} {'withheld':>14} {'NaN':>14} {'NaN':>14} {'-':>17}")
            continue
        kernel_only = matrices / float(row["kernel_device_s"])
        comp = float(row["composite_matrices_per_s"])
        ev = float(row["eval_matrices_per_s"])
        print(f"{row['n']:>3} {row['backend']:>16} {row['batch_size']:>5} {row['outcome']:>9}"
              f" {matrices:>9} {kernel_only:>14.4f} {ev:>14.4f} {comp:>14.4f}"
              f" {kernel_only / comp:>17.4f}")

    print()
    print("== section 4.3: per-launch device cost ==")
    print(f"{'n':>3} {'path':>16} {'M':>5} {'reps':>6} {'launch_host_us':>15}"
          f" {'launch_dev_us':>14} {'kernel_ms':>14} {'h2d_us':>9} {'d2h_us':>9}")
    for row in device_rows:
        if not row["kernel_device_s"]:
            continue
        reps = int(row["reps"])
        print(f"{row['n']:>3} {row['backend']:>16} {row['batch_size']:>5} {reps:>6}"
              f" {1e6 * float(row['host_submission_s']) / reps:>15.4f}"
              f" {1e6 * float(row['device_submission_to_kernel_s']) / reps:>14.4f}"
              f" {1e3 * float(row['kernel_device_s']) / reps:>14.4f}"
              f" {1e6 * float(row['h2d_device_s']) / reps:>9.4f}"
              f" {1e6 * float(row['d2h_device_s']) / reps:>9.4f}")

    print()
    print("== section 4.4: best operating point of each device path ==")
    for family, chooser in (("prototype", best_proto), ("shipped gpu_hip", best_gpu)):
        available = [n for n in ORDERS if chooser[n] is not None]
        if not available:
            print(f"{family:>16} has no measured cell at any order")
            continue
        ratios = {n: chooser[n][1] / best_cpu[n][1] for n in available}
        peak_ratio_n = max(ratios, key=ratios.get)
        peak_rate_n = max(available, key=lambda n: chooser[n][1])
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
    print("== section 4.5: batch size, device parallelism and shared bytes per block ==")
    print(f"{'n':>3} {'path':>16} {'M':>6} {'lanes/block':>12} {'active lanes':>13}"
          f" {'shared B/block':>15} {'batch source':>13}")
    for row in device_rows:
        m = int(row["batch_size"])
        n = int(row["n"])
        lanes = 1 if row["backend"] == "gpu_hip" else min(WAVE_LANES, 2**n)
        source = "fixed" if row["backend"] == "gpu_hip" else "calibrated"
        print(f"{n:>3} {row['backend']:>16} {m:>6} {lanes:>12} {m * lanes:>13}"
              f" {prototype_shared_bytes(row['backend'], n):>15} {source:>13}")

    print()
    print("== section 4.5: single-matrix probe against achieved per-matrix cost ==")
    print(f"{'n':>3} {'path':>24} {'M':>6} {'probe_matrix_s':>15} {'eval_s/matrix':>16}"
          f" {'probe/achieved':>15} {'batch_source':>14}")
    for n in ORDERS:
        for key, row in sorted(by_order[n].items()):
            probe = float(row["probe_matrix_s"])
            achieved = float(row["eval_s"]) / int(row["matrices"])
            source = "fixed" if row["backend"] == "gpu_hip" else "calibrated"
            ratio = probe / achieved if probe == probe else float("nan")
            # A calibrated cell takes M = ceil(2 s / probe_matrix_s), clamped to
            # [floor, 65536] (protocol.rs:724-737). Reconstructing it from the
            # committed probe shows the batch is the calibration's output and
            # not a separate choice.
            reconstructed = (
                math.ceil(2.0 / probe) if source == "calibrated" and probe == probe else None
            )
            check = ""
            if reconstructed is not None:
                check = (" ceil(2s/probe)=" + str(reconstructed)
                         + (" exact" if reconstructed == int(row["batch_size"]) else " DIFFERS"))
            print(f"{n:>3} {key:>24} {row['batch_size']:>6} {probe:>15.6f} {achieved:>16.9f}"
                  f" {ratio:>15.1f} {source:>14}{check}")

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
        ref = by_order[ref_n].get(label(row))
        if ref is None:
            print(f"  reference cell n={ref_n} {label(row)} is not measured in this grid")
            continue
        ref_rate = float(ref["composite_matrices_per_s"])
        scaled = ref_rate * ryser_work(ref_n) / ryser_work(int(row["n"]))
        print(f"  reference cell n={ref_n} rate={ref_rate:.6f};"
              f" Ryser-model rescale = {scaled:.6f}")
        if int(row["reps"]) > 0:
            per_rep = float(row["total_s"]) / int(row["reps"])
            print(f"  observed {row['reps']} repetition(s) at {per_rep:.6f} s each"
                  f" against the 120 s cap and the 5-repetition minimum")

    print()
    print("== section 5: projection accuracy on this file's own q=5 chains ==")
    print(f"{'path':>24} {'step':>12} {'projection':>14} {'measured':>14} {'error':>9}")
    for key in ("gpu_hip@M=256", "gpu_hip@M=1024") + PROTOTYPES + (
        "cpu_scalar", "cpu_rayon_batch_scalar", "cpu_ryser_generic",
    ):
        for lo, hi in zip(ORDERS, ORDERS[1:]):
            if key not in by_order[lo] or key not in by_order[hi]:
                continue
            cell = by_order[hi][key]
            if cell["outcome"] != "measured":
                continue
            proj = float(cell["projected_matrices_per_s"])
            meas = float(cell["composite_matrices_per_s"])
            print(f"{key:>24} {f'{lo}->{hi}':>12} {proj:>14.4f} {meas:>14.4f}"
                  f" {100.0 * (proj - meas) / meas:>8.1f}%")

    print()
    print("== section 6.1: Gray-update isolation, q=5, all orders ==")
    gray = [r for r in read_simple(GRAY) if int(r["q"]) == Q]
    for row in gray:
        if row["outcome"] != "measured":
            continue
        n = int(row["n"])
        net = float(row["net_per_operation_s"])
        print(f"n={n:>3} {row['backend']:>12} {row['outcome']:>9}"
              f" steps={row['steps']} reps={row['reps']:>6}"
              f" update_s={row['update_s']:>14} baseline_s={row['compiler_barrier_baseline_s']:>14}"
              f" net={net:.12f} per_row={net / n:.12e} basis={row['duration_basis']}")
    print("per-row net cost, flatness across orders:")
    for backend in ("cpu_scalar", "gpu_hip"):
        cells = [r for r in gray if r["backend"] == backend and r["outcome"] == "measured"]
        if not cells:
            continue
        nets = [float(r["net_per_operation_s"]) for r in cells]
        per_row = [float(r["net_per_operation_s"]) / int(r["n"]) for r in cells]
        print(f"  {backend:>12} net spread     {min(nets):.6e}..{max(nets):.6e}"
              f"  max/min={max(nets) / min(nets):.4f}")
        print(f"  {backend:>12} net/n spread   {min(per_row):.6e}..{max(per_row):.6e}"
              f"  max/min={max(per_row) / min(per_row):.4f}")
    host = {int(r["n"]): float(r["net_per_operation_s"])
            for r in gray if r["backend"] == "cpu_scalar" and r["outcome"] == "measured"}
    device = {int(r["n"]): float(r["net_per_operation_s"])
              for r in gray if r["backend"] == "gpu_hip" and r["outcome"] == "measured"}
    print("device net per operation over host net per operation:")
    for n in sorted(set(host) & set(device)):
        print(f"  n={n:>3} device/host = {device[n] / host[n]:.1f}")
    print("distinct notes:")
    seen_notes: set[str] = set()
    for row in gray:
        if row["note"] in seen_notes:
            continue
        seen_notes.add(row["note"])
        print(f"  {row['backend']}: {row['note']}")

    print()
    print("== section 6.2: horizontal-product isolation, q=5, all orders ==")
    hprod = [r for r in read_simple(HPROD) if int(r["q"]) == Q]
    for row in hprod:
        if row["outcome"] == "unsupported":
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
    print("nonzero-slow net per operation, three-plane against the byte circuit:")
    for n in ORDERS:
        rows_at_n = {r["backend"]: r for r in hprod if int(r["n"]) == n}
        byte_row = rows_at_n.get("f5-byte-control")
        plane_row = rows_at_n.get("f5-three-plane")
        gpu_row = rows_at_n.get("gpu_hip")
        parts = []
        for name, row in (("gpu_hip", gpu_row), ("f5-byte-control", byte_row),
                          ("f5-three-plane", plane_row)):
            value = row["nonzero_slow_net_per_operation_s"] if row else ""
            parts.append(f"{name}={value or 'ABSENT'}")
        if byte_row and plane_row and byte_row["nonzero_slow_net_per_operation_s"] \
                and plane_row["nonzero_slow_net_per_operation_s"]:
            ratio = (float(byte_row["nonzero_slow_net_per_operation_s"])
                     / float(plane_row["nonzero_slow_net_per_operation_s"]))
            parts.append(f"byte/plane={ratio:.4f}")
        if gpu_row and byte_row and gpu_row["nonzero_slow_net_per_operation_s"] \
                and byte_row["nonzero_slow_net_per_operation_s"]:
            same = (float(gpu_row["nonzero_slow_net_per_operation_s"])
                    / float(byte_row["nonzero_slow_net_per_operation_s"]))
            parts.append(f"gpu/byte={same:.4f}")
        print(f"  n={n:>3} " + "  ".join(parts))
    print("gpu_hip and f5-byte-control select the same F5Byte circuit"
          " (horizontal_product.rs:487, :499); largest disagreement between them:")
    for branch in ("zero_fast", "nonzero_slow"):
        worst = 0.0
        worst_n = None
        for n in ORDERS:
            rows_at_n = {r["backend"]: r for r in hprod if int(r["n"]) == n}
            left = rows_at_n.get("gpu_hip", {}).get(f"{branch}_net_per_operation_s")
            right = rows_at_n.get("f5-byte-control", {}).get(f"{branch}_net_per_operation_s")
            if not left or not right:
                continue
            gap = abs(float(left) - float(right)) / float(right)
            if gap > worst:
                worst, worst_n = gap, n
        print(f"  {branch:>13}: {100.0 * worst:.2f} % at n={worst_n}")
    print("unsupported and censored reasons, one line per distinct backend:")
    seen_hp: set[str] = set()
    for row in hprod:
        if row["outcome"] == "measured" or row["backend"] in seen_hp:
            continue
        seen_hp.add(row["backend"])
        reason = row["note"].split("counts; ", 1)[-1]
        print(f"  {row['backend']} [{row['outcome']}]: {reason}")

    print()
    print("== section 9: zero fast path frequency, exact expectation, Wilson 95 % ==")
    for row in hprod:
        if row["backend"] != "gpu_hip":
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
    print("every q=5 row at one order carries the same observation batch:")
    for n in ORDERS:
        pairs = {(r["zero_fast_observed_numerator"], r["nonzero_slow_observed_numerator"],
                  r["zero_fast_observed_denominator"])
                 for r in hprod if int(r["n"]) == n}
        print(f"  n={n:>3} distinct (zero, nonzero, denominator) triples across all"
              f" 14 rows: {sorted(pairs)}")

    print()
    print("== section 9: branch split of the device-timed operations, Wilson 95 % ==")
    print(f"{'n':>3} {'backend':>16} {'zero_fast_ops':>15} {'nonzero_slow_ops':>17} {'total':>12}"
          f" {'slow frequency':>15} {'slow expected':>15} {'wilson_lo':>12} {'wilson_hi':>12}"
          f" {'covers':>7}")
    for row in hprod:
        if row["outcome"] == "unsupported" or not row["zero_fast_timed_operations"]:
            continue
        n = int(row["n"])
        fast_ops = int(row["zero_fast_timed_operations"])
        slow_ops = int(row["nonzero_slow_timed_operations"])
        total_ops = fast_ops + slow_ops
        exp_slow = ((Q - 1) / Q) ** n
        lo, hi = wilson(slow_ops, total_ops)
        expected_count = exp_slow * total_ops
        sigma = math.sqrt(total_ops * exp_slow * (1 - exp_slow))
        print(f"{n:>3} {row['backend']:>16} {fast_ops:>15} {slow_ops:>17} {total_ops:>12}"
              f" {slow_ops / total_ops:>15.9f} {exp_slow:>15.9f} {lo:>12.9f} {hi:>12.9f}"
              f" {str(lo <= exp_slow <= hi):>7}"
              f" expected_count={expected_count:>10.1f}"
              f" z={(slow_ops - expected_count) / sigma:>+6.2f}")
        assert total_ops == int(row["reps"]) * int(row["samples_per_rep"]), row

    print()
    print("== section 9: zero fast path share across the three fields ==")
    print(f"{'q':>3} {'n':>3} {'zero_fast':>15} {'nonzero_slow':>15}")
    for field in (3, 5, 7):
        for n in (min(ORDERS), max(ORDERS)):
            slow = ((field - 1) / field) ** n
            print(f"{field:>3} {n:>3} {1.0 - slow:>15.9f} {slow:>15.9f}")

    print()
    print("== section 9.1: permanent-zero fraction pooled per order, Wilson 95 % ==")
    print(f"{'n':>3} {'zeros':>10} {'matrices':>10} {'fraction':>11} {'wilson_lo':>11} {'wilson_hi':>11}")
    for n in ORDERS:
        zeros = sum(int(r["zeros"]) for r in measured if int(r["n"]) == n)
        total = sum(int(r["matrices"]) for r in measured if int(r["n"]) == n)
        lo, hi = wilson(zeros, total)
        print(f"{n:>3} {zeros:>10} {total:>10} {zeros / total:>11.6f} {lo:>11.6f} {hi:>11.6f}")

    print()
    print("== section 7: compiler kernel-resource remarks for the measured F_5 kernel set ==")
    print(f"{'kernel':>62} {'SGPR':>5} {'VGPR':>5} {'scratch':>8} {'sgpr_sp':>8}"
          f" {'vgpr_sp':>8} {'LDS':>5} {'occ':>4}  log")
    for name in RESOURCE_LOGS:
        for kernel in read_resource_log(RESOURCES / name):
            print(f"{kernel['kernel'][:62]:>62} {kernel['TotalSGPRs']:>5} {kernel['VGPRs']:>5}"
                  f" {kernel['ScratchSize [bytes/lane]']:>8} {kernel['SGPRs Spill']:>8}"
                  f" {kernel['VGPRs Spill']:>8} {kernel['LDS Size [bytes/block]']:>5}"
                  f" {kernel['Occupancy [waves/SIMD]']:>4}  {kernel['log']}")

    print()
    print("== section 8: occupancy against per-lane vector registers, every kernel in the receipt ==")
    print(f"{'kernel':>60} {'VGPR':>5} {'SGPR':>5} {'scratch':>8} {'LDS':>5}"
          f" {'reported occ':>13} {'model occ':>10} {'agree':>6}  log")
    mismatches = 0
    entries = 0
    for path in sorted(RESOURCES.glob("*.resource.log")):
        for kernel in read_resource_log(path):
            entries += 1
            vgprs = int(kernel["VGPRs"])
            reported = int(kernel["Occupancy [waves/SIMD]"])
            model = occupancy_model(vgprs)
            agree = model == reported
            mismatches += 0 if agree else 1
            print(f"{kernel['kernel'][:60]:>60} {vgprs:>5} {kernel['TotalSGPRs']:>5}"
                  f" {kernel['ScratchSize [bytes/lane]']:>8}"
                  f" {kernel['LDS Size [bytes/block]']:>5} {reported:>13} {model:>10}"
                  f" {str(agree):>6}  {kernel['log']}")
    print(f"entries={entries} mismatches={mismatches}")
    assert mismatches == 0, "the per-lane register model no longer reproduces the receipt"

    print()
    print("== section 10: control mapping against lane-owns-interval, matched orders ==")
    print(f"{'n':>3} {'control M':>10} {'control rate':>14} {'byte M':>7} {'byte rate':>14}"
          f" {'plane M':>8} {'plane rate':>14} {'plane/control':>14}")
    for n in ORDERS:
        gpu = best_gpu[n]
        control_m = by_order[n][gpu[0]]["batch_size"] if gpu else "-"
        control_rate = f"{gpu[1]:>14.4f}" if gpu else f"{'all censored':>14}"
        byte_cell = by_order[n].get("f5-byte-control")
        plane_cell = by_order[n]["f5-three-plane"]
        plane_rate = float(plane_cell["composite_matrices_per_s"])
        ratio = f"{plane_rate / gpu[1]:>14.4f}" if gpu else f"{'-':>14}"
        byte_m = byte_cell["batch_size"] if byte_cell else "-"
        byte_rate = (f"{float(byte_cell['composite_matrices_per_s']):>14.4f}"
                     if byte_cell else f"{'censored':>14}")
        print(f"{n:>3} {control_m:>10} {control_rate}"
              f" {byte_m:>7} {byte_rate}"
              f" {plane_cell['batch_size']:>8} {plane_rate:>14.4f} {ratio}")

    print()
    print("== section 11: agreement with the prior grid's committed rates ==")
    prior_rows = read_simple(PRIOR)
    prior: dict[int, dict[str, float]] = {n: {} for n in ORDERS}
    for row in prior_rows:
        if int(row["q"]) != Q or row["outcome"] != "measured":
            continue
        prior[int(row["n"])][label(row)] = float(row["composite_matrices_per_s"])
    print(f"source: {PRIOR}")
    print(f"{'n':>3} {'path':>24} {'prior run':>14} {'this run':>14} {'delta':>8}")
    spread: list[float] = []
    by_group: dict[str, list[float]] = {}
    for n in ORDERS:
        for key, ref in sorted(prior[n].items()):
            cell = by_order[n].get(key)
            if cell is None:
                print(f"{n:>3} {key:>24} {ref:>14.4f} {'censored':>14} {'-':>8}")
                continue
            here = float(cell["composite_matrices_per_s"])
            delta = 100.0 * (here - ref) / ref
            spread.append(abs(delta))
            group = "gpu" if key.startswith("gpu_hip") else "cpu"
            by_group.setdefault(group, []).append(abs(delta))
            if n >= 16:
                by_group.setdefault("n>=16", []).append(abs(delta))
            print(f"{n:>3} {key:>24} {ref:>14.4f} {here:>14.4f} {delta:>7.2f}%")
    spread.sort()
    print(f"pairs={len(spread)} median|delta|={spread[len(spread) // 2]:.2f}%"
          f" max|delta|={spread[-1]:.2f}%")
    for group in sorted(by_group):
        print(f"  max|delta| over {group:>8} = {max(by_group[group]):.2f}%"
              f"  (n={len(by_group[group])})")

    print()
    print("== section 11: sign of the run-to-run delta, grouped by the kernel each path forces ==")
    # cpu_scalar and cpu_rayon_batch_scalar both force permanent_bipedal5
    # (dev/research/permanent-sampling-feas/src/backend.rs:410, :431);
    # cpu_ryser_generic forces the field-agnostic permanent_ryser (:458).
    families = {
        "packed permanent_bipedal5": ("cpu_scalar", "cpu_rayon_batch_scalar"),
        "generic permanent_ryser": ("cpu_ryser_generic",),
        "shipped gpu_hip": ("gpu_hip@M=256", "gpu_hip@M=1024"),
    }
    for family, keys in families.items():
        deltas = []
        for n in ORDERS:
            for key in keys:
                ref = prior[n].get(key)
                cell = by_order[n].get(key)
                if ref is None or cell is None:
                    continue
                here = float(cell["composite_matrices_per_s"])
                deltas.append(100.0 * (here - ref) / ref)
        negative = sum(1 for d in deltas if d < 0)
        print(f"  {family:>26} pairs={len(deltas):>2} negative={negative:>2}"
              f" range=[{min(deltas):+.2f}%, {max(deltas):+.2f}%]"
              f" mean={sum(deltas) / len(deltas):+.2f}%")

    print()
    print("== section 11: prior batch rayon against the prior GPU at every shared order ==")
    for n in ORDERS:
        rayon = prior[n].get("cpu_rayon_batch_scalar")
        gpus = {k: v for k, v in prior[n].items() if k.startswith("gpu_hip")}
        if rayon is None or not gpus:
            print(f"n={n}: no shared order (prior gpu cells: {sorted(gpus)})")
            continue
        gk = max(gpus, key=gpus.get)
        here_rayon = by_order[n].get("cpu_rayon_batch_scalar")
        here_gpu = best_gpu[n]
        here = ""
        if here_rayon is not None and here_gpu is not None:
            here = (f"  this run {float(here_rayon['composite_matrices_per_s']) / here_gpu[1]:.4f}x"
                    f" ({here_gpu[0]})")
        print(f"n={n}: prior rayon {rayon:.4f} / prior {gk} {gpus[gk]:.4f}"
              f" = {rayon / gpus[gk]:.4f}x{here}")

    print()
    print("== section 11: prior cells published as measured below the 5-repetition minimum ==")
    for row in prior_rows:
        if row["outcome"] == "measured" and int(row["reps"]) < 5:
            print(f"  q={row['q']} n={row['n']} {row['backend']} M={row['batch_size']}"
                  f" reps={row['reps']} total_s={row['total_s']}"
                  f" composite={row['composite_matrices_per_s']}")
    print("the same cells in this run:")
    for row in grid:
        if (int(row["n"]), row["backend"]) == (28, "cpu_rayon_batch_scalar"):
            print(f"  q=5 n={row['n']} {row['backend']} M={row['batch_size']}"
                  f" reps={row['reps']} total_s={row['total_s']}"
                  f" outcome={row['outcome']} composite={row['composite_matrices_per_s']}")

    print()
    print("== section 11: prior censored GPU cells against this run's ==")
    for row in prior_rows:
        if int(row["q"]) == Q and row["outcome"] == "censored":
            print(f"  prior  n={row['n']} {label(row)} projected={row['projected_matrices_per_s']}")
    for row in grid:
        if row["outcome"] == "censored" and row["backend"] == "gpu_hip":
            print(f"  this   n={row['n']} {label(row)} projected={row['projected_matrices_per_s']}"
                  f" reference_n={row['projection_reference_n']}")

    print()
    print("== section 12: per-launch device work against the archived q=5 calibration ==")
    print(f"{'n':>3} {'path':>16} {'M':>6} {'M*2^n':>16} {'budget share':>13}"
          f" {'kernel s/launch':>16} {'boundary share':>15}")
    worst_work = 0.0
    worst_span = 0.0
    for row in device_rows:
        if not row["kernel_device_s"]:
            continue
        n = int(row["n"])
        m = int(row["batch_size"])
        work = m * (2.0**n)
        span = float(row["kernel_device_s"]) / int(row["reps"])
        worst_work = max(worst_work, work)
        worst_span = max(worst_span, span)
        print(f"{n:>3} {row['backend']:>16} {m:>6} {work:>16.4g}"
              f" {work / ARCHIVED_Q5_WORK_BUDGET:>13.4f}"
              f" {span:>16.4f} {span / ARCHIVED_HANG_BOUNDARY_S:>15.4f}")
    print(f"largest per-launch work in this campaign = {worst_work:.4g}"
          f" ({worst_work / ARCHIVED_Q5_WORK_BUDGET:.4f} of the archived q=5 budget"
          f" {ARCHIVED_Q5_WORK_BUDGET:.3g})")
    print(f"longest kernel span per launch = {worst_span:.4f} s"
          f" ({worst_span / ARCHIVED_HANG_BOUNDARY_S:.4f} of the archived"
          f" {ARCHIVED_HANG_BOUNDARY_S:.0f} s hang boundary)")

    print()
    print("== section 12: the declared operating point, q=5 n=24 ==")
    declared_n = 24
    cell = by_order[declared_n]["f5-three-plane"]
    rate = float(cell["composite_matrices_per_s"])
    cpu_key, cpu_rate = best_cpu[declared_n]
    reps = int(cell["reps"])
    m = int(cell["batch_size"])
    print(f"best prototype f5-three-plane M={m} rate={rate:.4f} matrices/s")
    print(f"best applicable in-tree CPU path {cpu_key} rate={cpu_rate:.4f} matrices/s")
    print(f"ratio = {rate / cpu_rate:.4f} (target 1.5)")
    print(f"device_submission_to_kernel_s={cell['device_submission_to_kernel_s']}"
          f" over reps={reps} = {1e6 * float(cell['device_submission_to_kernel_s']) / reps:.4f}"
          f" us/launch")
    print(f"kernel_device_s={cell['kernel_device_s']} over reps={reps}"
          f" = {float(cell['kernel_device_s']) / reps:.4f} s/launch")
    print(f"per-launch work M*2^n = {m * 2**declared_n:.6g}"
          f" ({m * 2**declared_n / ARCHIVED_Q5_WORK_BUDGET:.4f} of the archived q=5 budget)")
    print("CPU paths measured at this order, ranked:")
    for key, row in sorted(
        by_order[declared_n].items(),
        key=lambda kv: -float(kv[1]["composite_matrices_per_s"]),
    ):
        if key in CPU_PATHS:
            print(f"  {key:>24} {float(row['composite_matrices_per_s']):>12.4f}")


if __name__ == "__main__":
    main()
