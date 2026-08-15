#!/usr/bin/env python3
"""Derive every table in `receipts.md` from the committed campaign artifacts.

Run from the repository root:

    python3 dev/studies/6c7fcb38/analysis.py

The script reads only committed artifacts and writes nothing: the three
$\\mathbb{F}_7$ CSVs of campaign run `20260814T230032Z-2085453` beside it, the
shared equivalence CSV of the same run under `dev/studies/047b62ed/`, the prior
grid under `dev/studies/b488f02c/`, and the compiler kernel-resource logs of
receipt `20260814T172506Z-1610002` in this directory. Every figure quoted in
`receipts.md` is printed here under the heading that names its section, so a
reviewer can diff prose against data without re-running the measurement.
"""

from __future__ import annotations

import math
import pathlib

STUDY = pathlib.Path(__file__).resolve().parent
RUN = "permanent-campaign-20260814T230032Z-2085453"
GRID = STUDY / f"{RUN}-q7-grid.csv"
GRAY = STUDY / f"{RUN}-q7-gray-update.csv"
HPROD = STUDY / f"{RUN}-q7-horizontal-product.csv"
# One global equivalence invocation gates all three fields; it is committed
# under the F_3 study directory and this script reads its q=7 rows.
EQUIV = STUDY.parent / "047b62ed" / f"{RUN}-shared-equivalence.csv"
# Prior grid this campaign has to confirm or overturn: the committed artifact
# the feasibility study's section 4.4 table is rendered from, read here rather
# than that table, so no figure of it is maintained in two places.
PRIOR = STUDY.parent / "b488f02c" / "throughput-2026-08-07.csv"
RESOURCES = STUDY / "hip-resource-usage-20260814T172506Z-1610002"
# The three logs holding the kernels this campaign measures. The two
# horizontal-product kernels and the two Gray-update kernels appear in the
# permanent_bipedal7 and gray_update_micro logs respectively;
# horizontal_product_micro.hip is compiled as an included fragment of the
# permanent_bipedal7 translation unit (receipt.txt, translation_unit_role).
RESOURCE_LOGS = (
    "permanent_bipedal7.hip.resource.log",
    "wave_gf7_equivalence.hip.resource.log",
    "gray_update_micro.hip.resource.log",
    "probe.hip.resource.log",
)

Q = 7
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
PROTOTYPES = ("f7-lookup-table-control", "f7-three-plane-permanent")
# The one bit-sliced F_7 path that executes as a full-permanent backend: its
# columns are three u64 bit planes (wave_gf7_equivalence.hip:75, :134-163).
BIT_SLICED = ("f7-three-plane-permanent",)
# gfx1030 wave width, and the lane count every F_7 prototype block launches at
# the orders this campaign measures: active_lanes_for_order(n) = 32 for n >= 5
# (dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:29-31, :16).
WAVE_LANES = 32
# Archived per-launch work budget, carried as a prior rather than as an
# established device property. The archived calibration states its budget for
# q=5; no q=7 budget is committed, so only the per-launch span boundary is
# applied to this field (dev/archive/ae82bd73-gf2-algebra-permanent/plans/
# b293af5a/r4_gpu_uniformity_resample.md section 2.5).
ARCHIVED_HANG_BOUNDARY_S = 190.0
# The grid writer emits `phase_timing_note` and `note` unquoted, and both carry
# literal commas on the out-of-field rows, so those lines hold more fields than
# the header names (bug 3ea21d74). `seed_root` is the one column whose value has
# a fixed prefix, so it anchors the split: everything between
# `phase_timing_note` and the eleven columns that precede `seed_root` belongs to
# the note, and everything after `order_index` belongs to the trailing note.
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

    `3 * n * 8` for the three-plane Gray-walk kernel's staged column planes and
    `8 * n * ceil(n / 16)` for the lookup control's staged nibble words
    (dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:683, :718,
    :723). The preparation kernel that stages the planes is launched with none
    (`:677`).
    """
    if backend == "f7-three-plane-permanent":
        return 3 * n * 8
    if backend == "f7-lookup-table-control":
        return 8 * n * math.ceil(n / 16)
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

    print("== section 1: reserved stream block for execution id 7002 ==")
    execution_id = 7002
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
    print("measured orders per backend:")
    for backend in sorted({r["backend"] for r in grid}):
        orders = sorted({int(r["n"]) for r in grid
                         if r["backend"] == backend and r["outcome"] == "measured"})
        print(f"  {backend:>28} measured at n={orders}")

    print()
    print("== section 3: equivalence verdicts for q=7 ==")
    equiv_all = read_simple(EQUIV)
    equiv = [r for r in equiv_all if int(r["q"]) == Q]
    identical = [r for r in equiv if r["status"].startswith("identical")]
    print(f"q=7 rows={len(equiv)} identical={len(identical)}"
          f" unsupported={len(equiv) - len(identical)}")
    print(f"all-fields identical rows in this file="
          f"{sum(1 for r in equiv_all if r['status'].startswith('identical'))}")
    for row in equiv:
        print(f"n={row['n']:>3} reference={row['reference']:>18} backend={row['backend']:>28}"
              f" matrices={row['matrices']:>4} mismatches={row['mismatches']:>2}"
              f" zeros_ref={row['zeros_reference']:>4} zeros_backend={row['zeros_backend']:>4}"
              f" status={row['status'][:60]}")
    print("per-order identical backend counts, matrix counts and reference kernel:")
    for n in EQUIV_ORDERS:
        cells = [r for r in equiv if int(r["n"]) == n and r["status"].startswith("identical")]
        counts = {int(r["matrices"]) for r in cells}
        zeros = {int(r["zeros_reference"]) for r in cells}
        refs = {r["reference"] for r in cells}
        print(f"  n={n:>3} identical_backends={len(cells)} matrices={sorted(counts)}"
              f" zeros_reference={sorted(zeros)}"
              f" mismatch_total={sum(int(r['mismatches']) for r in cells)}"
              f" reference={sorted(refs)}")

    print()
    print("== section 4.2: composite throughput, best path per order ==")
    print(f"{'n':>3} {'best overall':>26} {'rate':>14} {'best CPU':>22} {'rate':>14}"
          f" {'best prototype':>26} {'rate':>14} {'best GPU':>16} {'rate':>14}"
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
        print(f"{n:>3} {overall:>26} {rates[overall]:>14.4f} {ck:>22} {cpu[ck]:>14.4f}"
              f" {pk:>26} {proto[pk]:>14.4f} {gpu_cell}"
              f" {proto[pk] / cpu[ck]:>10.4f} {gpu_ratio}")

    print()
    print("== section 4.2: bit-sliced path against the lookup control and the shipped path ==")
    print(f"{'n':>3} {'three-plane':>15} {'lookup control':>16} {'plane/lookup':>13}"
          f" {'best gpu_hip':>14} {'plane/gpu_hip':>14}")
    for n in ORDERS:
        plane = float(by_order[n]["f7-three-plane-permanent"]["composite_matrices_per_s"])
        ctrl = by_order[n].get("f7-lookup-table-control")
        ctrl_text = (f"{float(ctrl['composite_matrices_per_s']):>16.4f}"
                     if ctrl else f"{'censored':>16}")
        ratio_text = (
            f"{plane / float(ctrl['composite_matrices_per_s']):>13.4f}"
            if ctrl else f"{'-':>13}"
        )
        gpu = best_gpu[n]
        gpu_text = f"{gpu[1]:>14.4f}" if gpu else f"{'all censored':>14}"
        gpu_ratio = f"{plane / gpu[1]:>14.4f}  ({gpu[0]})" if gpu else f"{'-':>14}"
        print(f"{n:>3} {plane:>15.4f} {ctrl_text} {ratio_text} {gpu_text} {gpu_ratio}")

    print()
    print("== section 4.2: every measured cell, composite rate ==")
    for n in ORDERS:
        for key, row in sorted(
            by_order[n].items(), key=lambda kv: -float(kv[1]["composite_matrices_per_s"])
        ):
            print(f"n={n:>3} {key:>26} M={row['batch_size']:>6} reps={row['reps']:>6}"
                  f" matrices={row['matrices']:>8}"
                  f" composite={float(row['composite_matrices_per_s']):>14.4f}"
                  f" eval_only={float(row['eval_matrices_per_s']):>14.4f}"
                  f" rep_sd_s={row['rep_sd_s']}")

    print()
    print("== section 4.3: device phase columns on every device-backed row ==")
    print(f"{'n':>3} {'path':>26} {'M':>5} {'outcome':>9} {'eval_s':>13} {'kernel_device_s':>16}"
          f" {'h2d_device_s':>13} {'d2h_device_s':>13} {'host_subm_s':>13}"
          f" {'dev_subm_to_kern_s':>19} {'residual_s':>11} {'kernel/eval':>11}"
          f" {'host_subm/eval':>14}")
    for row in device_rows:
        if not row["kernel_device_s"]:
            print(f"{row['n']:>3} {row['backend']:>26} {row['batch_size']:>5} {row['outcome']:>9}"
                  f"   phase columns empty: {row['phase_timing_note']}")
            continue
        ev = float(row["eval_s"])
        ke = float(row["kernel_device_s"])
        h2d = float(row["h2d_device_s"])
        d2h = float(row["d2h_device_s"])
        hs = float(row["host_submission_s"])
        ds = float(row["device_submission_to_kernel_s"])
        print(f"{row['n']:>3} {row['backend']:>26} {row['batch_size']:>5} {row['outcome']:>9}"
              f" {ev:>13.6f} {ke:>16.6f}"
              f" {h2d:>13.6f} {d2h:>13.6f} {hs:>13.6f} {ds:>19.6f}"
              f" {ev - ke - h2d - d2h - ds:>11.6f} {ke / ev:>11.4f} {hs / ev:>14.4f}")
    hs_ratio = [(float(r["host_submission_s"]) / float(r["eval_s"]), r["backend"], r["n"])
                for r in device_rows if r["kernel_device_s"]]
    gpu_ratios = [t for t in hs_ratio if t[1] == "gpu_hip"]
    proto_ratios = [t for t in hs_ratio if t[1] != "gpu_hip"]
    print(f"host_submission_s/eval_s on gpu_hip rows:   "
          f"{min(gpu_ratios)[0]:.4f}..{max(gpu_ratios)[0]:.4f}")
    print(f"host_submission_s/eval_s on prototype rows: "
          f"{min(proto_ratios)[0]:.4f}..{max(proto_ratios)[0]:.4f}")

    print()
    print("== section 4.3: kernel-only against end-to-end throughput ==")
    print(f"{'n':>3} {'path':>26} {'M':>5} {'outcome':>9} {'matrices':>9} {'kernel_only/s':>14}"
          f" {'eval_only/s':>14} {'composite/s':>14} {'kernel/composite':>17}")
    for row in device_rows:
        matrices = int(row["matrices"])
        if row["outcome"] != "measured":
            # A censored cell carries no throughput value, kernel-only included.
            print(f"{row['n']:>3} {row['backend']:>26} {row['batch_size']:>5} {row['outcome']:>9}"
                  f" {matrices:>9} {'withheld':>14} {'NaN':>14} {'NaN':>14} {'-':>17}")
            continue
        kernel_only = matrices / float(row["kernel_device_s"])
        comp = float(row["composite_matrices_per_s"])
        ev = float(row["eval_matrices_per_s"])
        print(f"{row['n']:>3} {row['backend']:>26} {row['batch_size']:>5} {row['outcome']:>9}"
              f" {matrices:>9} {kernel_only:>14.4f} {ev:>14.4f} {comp:>14.4f}"
              f" {kernel_only / comp:>17.4f}")

    print()
    print("== section 4.3: per-launch device cost ==")
    print(f"{'n':>3} {'path':>26} {'M':>5} {'reps':>6} {'launch_host_us':>15}"
          f" {'launch_dev_us':>14} {'kernel_ms':>14} {'h2d_us':>9} {'d2h_us':>9}")
    launch_dev = []
    for row in device_rows:
        if not row["kernel_device_s"]:
            continue
        reps = int(row["reps"])
        dev_us = 1e6 * float(row["device_submission_to_kernel_s"]) / reps
        launch_dev.append(dev_us)
        print(f"{row['n']:>3} {row['backend']:>26} {row['batch_size']:>5} {reps:>6}"
              f" {1e6 * float(row['host_submission_s']) / reps:>15.4f}"
              f" {dev_us:>14.4f}"
              f" {1e3 * float(row['kernel_device_s']) / reps:>14.4f}"
              f" {1e6 * float(row['h2d_device_s']) / reps:>9.4f}"
              f" {1e6 * float(row['d2h_device_s']) / reps:>9.4f}")
    print(f"device submission-to-kernel per launch across every device row: "
          f"{min(launch_dev):.4f}..{max(launch_dev):.4f} us")

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
    print("prototype-over-best-CPU ratio at every order:")
    for n in ORDERS:
        key, rate = best_proto[n]
        print(f"  n={n:>3} {key} {rate:.4f} / {best_cpu[n][0]} {best_cpu[n][1]:.4f}"
              f" = {rate / best_cpu[n][1]:.4f}")
    print("shipped-over-best-CPU ratio at every order it is measured:")
    for n in ORDERS:
        gpu = best_gpu[n]
        if gpu is None:
            print(f"  n={n:>3} all gpu_hip cells censored")
            continue
        print(f"  n={n:>3} {gpu[0]} {gpu[1]:.4f} / {best_cpu[n][0]} {best_cpu[n][1]:.4f}"
              f" = {gpu[1] / best_cpu[n][1]:.4f}")

    print()
    print("== section 4.5: batch size, device parallelism and shared bytes per block ==")
    print(f"{'n':>3} {'path':>26} {'M':>6} {'lanes/block':>12} {'active lanes':>13}"
          f" {'dyn shared B/block':>19} {'batch source':>13}")
    for row in device_rows:
        m = int(row["batch_size"])
        n = int(row["n"])
        lanes = 1 if row["backend"] == "gpu_hip" else min(WAVE_LANES, 2**n)
        source = "fixed" if row["backend"] == "gpu_hip" else "calibrated"
        print(f"{n:>3} {row['backend']:>26} {m:>6} {lanes:>12} {m * lanes:>13}"
              f" {prototype_shared_bytes(row['backend'], n):>19} {source:>13}")

    print()
    print("== section 4.5: single-matrix probe against achieved per-matrix cost ==")
    print(f"{'n':>3} {'path':>26} {'M':>6} {'probe_matrix_s':>15} {'eval_s/matrix':>16}"
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
                # `probe_matrix_s` is committed to six decimals, so at the small
                # orders one printed value covers a range of true probes. The
                # reconstruction is therefore checked against the interval that
                # range implies rather than against a single value.
                lo_probe = max(probe - 0.5e-6, 1e-12)
                hi_probe = probe + 0.5e-6
                span = (math.ceil(2.0 / hi_probe), math.ceil(2.0 / lo_probe))
                inside = span[0] <= int(row["batch_size"]) <= span[1]
                check = (" ceil(2s/probe)=" + str(reconstructed)
                         + (" exact" if reconstructed == int(row["batch_size"])
                            else f" printed-precision span {span}"
                                 f" {'covers' if inside else 'MISSES'} M"))
            print(f"{n:>3} {key:>26} {row['batch_size']:>6} {probe:>15.6f} {achieved:>16.9f}"
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
        print(f"  phase_timing_note={row['phase_timing_note']}")
        print(f"  note={row['note']}")
        ref_n = int(row["projection_reference_n"])
        ref = by_order[ref_n].get(label(row))
        if ref is None:
            print(f"  reference cell n={ref_n} {label(row)} is not measured in this grid")
            continue
        ref_rate = float(ref["composite_matrices_per_s"])
        scaled = ref_rate * ryser_work(ref_n) / ryser_work(int(row["n"]))
        print(f"  reference cell n={ref_n} rate={ref_rate:.6f};"
              f" Ryser-model rescale = {scaled:.6f};"
              f" implied repetition = {int(row['batch_size']) / scaled:.1f} s against the 120 s cap")

    print()
    print("== section 5: projection accuracy on this file's own q=7 chains ==")
    print(f"{'path':>26} {'step':>12} {'projection':>14} {'measured':>14} {'error':>9}")
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
            print(f"{key:>26} {f'{lo}->{hi}':>12} {proj:>14.4f} {meas:>14.4f}"
                  f" {100.0 * (proj - meas) / meas:>8.1f}%")

    print()
    print("== section 6.1: Gray-update isolation, q=7, all orders ==")
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
        print(f"  {backend:>12} orders={sorted(int(r['n']) for r in cells)}")
        print(f"  {backend:>12} net spread     {min(nets):.6e}..{max(nets):.6e}"
              f"  max/min={max(nets) / min(nets):.4f}")
        print(f"  {backend:>12} net/n spread   {min(per_row):.6e}..{max(per_row):.6e}"
              f"  max/min={max(per_row) / min(per_row):.4f}")
    host = {int(r["n"]): float(r["net_per_operation_s"])
            for r in gray if r["backend"] == "cpu_scalar" and r["outcome"] == "measured"}
    device = {int(r["n"]): float(r["net_per_operation_s"])
              for r in gray if r["backend"] == "gpu_hip" and r["outcome"] == "measured"}
    print("device net per operation over host net per operation, where both exist:")
    for n in sorted(set(host) & set(device)):
        print(f"  n={n:>3} device/host = {device[n] / host[n]:.1f}")
    print("distinct notes:")
    seen_notes: set[str] = set()
    for row in gray:
        if row["note"] in seen_notes:
            continue
        seen_notes.add(row["note"])
        print(f"  {row['backend']} [{row['outcome']}]: {row['note'][:200]}")

    print()
    print("== section 6.2: horizontal-product isolation, q=7, all orders ==")
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
    print("net per operation by branch, the two F_7 circuits under four names:")
    for n in ORDERS:
        rows_at_n = {r["backend"]: r for r in hprod if int(r["n"]) == n}
        parts = []
        for name in ("gpu_hip", "f7-lookup-table-control",
                     "f7-three-plane-accumulator", "f7-three-plane-permanent"):
            row = rows_at_n.get(name)
            zero = (row["zero_fast_net_per_operation_s"] if row else "") or "ABSENT"
            slow = (row["nonzero_slow_net_per_operation_s"] if row else "") or "ABSENT"
            parts.append(f"{name}: zero={zero} slow={slow}")
        print(f"  n={n:>3} " + " | ".join(parts))
    print("gpu_hip and f7-lookup-table-control select the same F7Lookup circuit"
          " (horizontal_product.rs:488, :507-508); largest disagreement between them:")
    for branch in ("zero_fast", "nonzero_slow"):
        worst = 0.0
        worst_n = None
        for n in ORDERS:
            rows_at_n = {r["backend"]: r for r in hprod if int(r["n"]) == n}
            left = rows_at_n.get("gpu_hip", {}).get(f"{branch}_net_per_operation_s")
            right = rows_at_n.get("f7-lookup-table-control", {}).get(
                f"{branch}_net_per_operation_s")
            if not left or not right:
                continue
            gap = abs(float(left) - float(right)) / float(right)
            if gap > worst:
                worst, worst_n = gap, n
        print(f"  {branch:>13}: {100.0 * worst:.2f} % at n={worst_n}")
    print("the two F7ThreePlane rows against each other, where both carry a duration:")
    for branch in ("zero_fast", "nonzero_slow"):
        for n in ORDERS:
            rows_at_n = {r["backend"]: r for r in hprod if int(r["n"]) == n}
            left = rows_at_n.get("f7-three-plane-accumulator", {}).get(
                f"{branch}_net_per_operation_s")
            right = rows_at_n.get("f7-three-plane-permanent", {}).get(
                f"{branch}_net_per_operation_s")
            if not left or not right:
                continue
            print(f"  {branch:>13} n={n:>3} accumulator={left} permanent={right}"
                  f" ratio={float(left) / float(right):.4f}")
    print("unsupported and censored reasons, one line per distinct backend:")
    seen_hp: set[str] = set()
    for row in hprod:
        if row["outcome"] == "measured" or row["backend"] in seen_hp:
            continue
        seen_hp.add(row["backend"])
        reason = row["note"].split("counts; ", 1)[-1]
        print(f"  {row['backend']} [{row['outcome']}]: {reason[:260]}")

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
    print("every q=7 row at one order carries the same observation batch:")
    for n in ORDERS:
        pairs = {(r["zero_fast_observed_numerator"], r["nonzero_slow_observed_numerator"],
                  r["zero_fast_observed_denominator"])
                 for r in hprod if int(r["n"]) == n}
        print(f"  n={n:>3} rows={sum(1 for r in hprod if int(r['n']) == n)}"
              f" distinct (zero, nonzero, denominator) triples: {sorted(pairs)}")

    print()
    print("== section 9: branch split of the device-timed operations, Wilson 95 % ==")
    print(f"{'n':>3} {'backend':>28} {'zero_fast_ops':>15} {'nonzero_slow_ops':>17} {'total':>12}"
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
        print(f"{n:>3} {row['backend']:>28} {fast_ops:>15} {slow_ops:>17} {total_ops:>12}"
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
    print("== section 7: compiler kernel-resource remarks for the measured F_7 kernel set ==")
    print(f"{'kernel':>62} {'SGPR':>5} {'VGPR':>5} {'scratch':>8} {'sgpr_sp':>8}"
          f" {'vgpr_sp':>8} {'LDS':>5} {'occ':>4}  log")
    for name in RESOURCE_LOGS:
        for kernel in read_resource_log(RESOURCES / name):
            print(f"{kernel['kernel'][:62]:>62} {kernel['TotalSGPRs']:>5} {kernel['VGPRs']:>5}"
                  f" {kernel['ScratchSize [bytes/lane]']:>8} {kernel['SGPRs Spill']:>8}"
                  f" {kernel['VGPRs Spill']:>8} {kernel['LDS Size [bytes/block]']:>5}"
                  f" {kernel['Occupancy [waves/SIMD]']:>4}  {kernel['log']}")

    print()
    print("== section 7.1: committed dynamic shared bytes per block, per measured order ==")
    print(f"{'n':>3} {'wave_gf7_three_plane_kernel (24n)':>36}"
          f" {'wave_gf7_lookup_table_kernel (8n*ceil(n/16))':>46}")
    for n in ORDERS:
        print(f"{n:>3} {prototype_shared_bytes('f7-three-plane-permanent', n):>36}"
              f" {prototype_shared_bytes('f7-lookup-table-control', n):>46}")

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
    print(f"{'n':>3} {'control M':>10} {'control rate':>14} {'lookup M':>9} {'lookup rate':>14}"
          f" {'plane M':>8} {'plane rate':>14} {'plane/control':>14}")
    for n in ORDERS:
        gpu = best_gpu[n]
        control_m = by_order[n][gpu[0]]["batch_size"] if gpu else "-"
        control_rate = f"{gpu[1]:>14.4f}" if gpu else f"{'all censored':>14}"
        ctrl_cell = by_order[n].get("f7-lookup-table-control")
        plane_cell = by_order[n]["f7-three-plane-permanent"]
        plane_rate = float(plane_cell["composite_matrices_per_s"])
        ratio = f"{plane_rate / gpu[1]:>14.4f}" if gpu else f"{'-':>14}"
        ctrl_m = ctrl_cell["batch_size"] if ctrl_cell else "-"
        ctrl_rate = (f"{float(ctrl_cell['composite_matrices_per_s']):>14.4f}"
                     if ctrl_cell else f"{'censored':>14}")
        print(f"{n:>3} {control_m:>10} {control_rate}"
              f" {ctrl_m:>9} {ctrl_rate}"
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
    print(f"{'n':>3} {'path':>26} {'prior run':>14} {'this run':>14} {'delta':>8}")
    spread: list[float] = []
    by_group: dict[str, list[float]] = {}
    for n in ORDERS:
        for key, ref in sorted(prior[n].items()):
            cell = by_order[n].get(key)
            if cell is None:
                print(f"{n:>3} {key:>26} {ref:>14.4f} {'not measured':>14} {'-':>8}")
                continue
            here = float(cell["composite_matrices_per_s"])
            delta = 100.0 * (here - ref) / ref
            spread.append(abs(delta))
            group = "gpu" if key.startswith("gpu_hip") else "cpu"
            by_group.setdefault(group, []).append(abs(delta))
            print(f"{n:>3} {key:>26} {ref:>14.4f} {here:>14.4f} {delta:>7.2f}%")
    spread.sort()
    print(f"pairs={len(spread)} median|delta|={spread[len(spread) // 2]:.2f}%"
          f" max|delta|={spread[-1]:.2f}%")
    for group in sorted(by_group):
        print(f"  max|delta| over {group:>8} = {max(by_group[group]):.2f}%"
              f"  (n={len(by_group[group])})")

    print()
    print("== section 11: sign of the run-to-run delta, grouped by the kernel each path forces ==")
    # cpu_scalar and cpu_rayon_batch_scalar both force permanent_bipedal7
    # (dev/research/permanent-sampling-feas/src/backend.rs:414, :437);
    # cpu_ryser_generic forces the field-agnostic permanent_ryser (:456).
    families = {
        "packed permanent_bipedal7": ("cpu_scalar", "cpu_rayon_batch_scalar"),
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
            print(f"n={n}: no shared order (prior rayon={rayon}, prior gpu cells: {sorted(gpus)})")
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
    print("== section 11.1: independent check of the prior study's daggered cells ==")
    # The prior study marks its stopping-rule-nonconforming rendered cells with a
    # dagger under bug 4fdd781a and carries its own check for the convention,
    # dev/studies/b488f02c/verify-rendered-stopping-rule.py. This block derives
    # the same set from the CSV independently, so the receipts can cite the
    # marking and state that an independent derivation agrees with it.
    #
    # The harness rule, as implemented: a cell stops once both minimums are met
    # or the cap is reached, and it is censored only when the cap arrives first
    # (protocol.rs:178-186, :190-191). So the cap bounds when a further
    # repetition may start, not a cell's total: a cell that met both minimums may
    # legitimately close above 120 s because the repetition in flight finished.
    # A measured cell violates the rule exactly when the cap was reached without
    # both minimums, which is what `capped_before_minimums` returns true for.
    def outside_rule(row: dict[str, str]) -> bool:
        reps = int(row["reps"])
        wall = float(row["total_s"])
        return wall >= 120.0 and not (reps >= 5 and wall >= 5.0)

    for row in prior_rows:
        if row["outcome"] == "measured" and outside_rule(row):
            print(f"  q={row['q']} n={row['n']} {row['backend']} M={row['batch_size']}"
                  f" reps={row['reps']} total_s={row['total_s']}"
                  f" composite={row['composite_matrices_per_s']}"
                  f"  [cap reached before both minimums; expect a dagger on this cell]")
    # The rendered marking is the prior study's own artifact; agreement between
    # the set derived here and the daggered set there is what the receipts cite.
    rendered = (STUDY.parent / "b488f02c" / "feasibility-study.md").read_text()
    daggered = sum(1 for line in rendered.splitlines()
                   if line.startswith("|") and "†" in line)
    print(f"rendered section 4.4 rows carrying a dagger: {daggered}")
    assert daggered == sum(1 for row in prior_rows
                           if row["outcome"] == "measured" and outside_rule(row)), (
        "the daggered rendered cells no longer match the cells this check derives"
    )
    print("prior cells that close above the cap while conforming, which the rule allows"
          " and which the rendered table correctly leaves unmarked:")
    for row in prior_rows:
        if (row["outcome"] == "measured" and float(row["total_s"]) > 120.0
                and not outside_rule(row)):
            print(f"  q={row['q']} n={row['n']} {row['backend']} M={row['batch_size']}"
                  f" reps={row['reps']} total_s={row['total_s']}"
                  f"  [both minimums met before the cap; the last repetition ran long]")
    offenders = [row for row in prior_rows
                 if row["outcome"] == "measured" and int(row["q"]) == Q and outside_rule(row)]
    print(f"q=7 cells among the violations: count={len(offenders)}")
    print("the same check applied to this run's own q=7 grid:")
    here = [row for row in measured if outside_rule(row)]
    print(f"  measured cells outside the stopping rule: {len(here)}")
    longest = max(measured, key=lambda r: float(r["total_s"]))
    fewest = min(measured, key=lambda r: int(r["reps"]))
    print(f"  longest measured cell: n={longest['n']} {longest['backend']}"
          f" M={longest['batch_size']} reps={longest['reps']}"
          f" total_s={longest['total_s']}")
    print(f"  fewest repetitions:    n={fewest['n']} {fewest['backend']}"
          f" M={fewest['batch_size']} reps={fewest['reps']}"
          f" total_s={fewest['total_s']}")
    print("every prior q=7 measured cell's reps and total_s:")
    for row in prior_rows:
        if int(row["q"]) == Q and row["outcome"] == "measured":
            print(f"  n={row['n']:>3} {row['backend']:>24} M={row['batch_size']:>6}"
                  f" reps={row['reps']:>4} total_s={row['total_s']}")

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
    print("== section 12: bit-plane preparation is inside the measured kernel span ==")
    # `prepare_three_plane_columns` (wave_gf7_equivalence.hip:134) and
    # `wave_gf7_three_plane_kernel` (:166) are both enqueued inside one
    # ThreePlaneBatchLaunch call (:670-686), and the harness brackets that whole
    # enqueue between kKernelStart and kKernelEnd (wave_batch_stream.h:204-207).
    # So `kernel_device_s` on a f7-three-plane-permanent row is the sum of the
    # two launches and no committed column separates them.
    print(f"{'n':>3} {'M':>6} {'reps':>6} {'kernel_device_s':>16} {'kernel s/launch':>16}"
          f" {'h2d_device_s':>13} {'gen_s':>10} {'host portion':>13}")
    for row in device_rows:
        if row["backend"] not in BIT_SLICED or not row["kernel_device_s"]:
            continue
        reps = int(row["reps"])
        print(f"{row['n']:>3} {row['batch_size']:>6} {reps:>6}"
              f" {float(row['kernel_device_s']):>16.6f}"
              f" {float(row['kernel_device_s']) / reps:>16.6f}"
              f" {float(row['h2d_device_s']):>13.6f} {float(row['gen_s']):>10.6f}"
              f" {'0 by source':>13}")

    print()
    print("== section 13: occupancy evidence available in this run's artifacts ==")
    # Achieved occupancy asks for profiler counters taken during the measured
    # run. This block reports what the run's own artifacts contain: the compiler
    # figure, which is a compile-time prediction and not a runtime observation,
    # and a token scan over every committed artifact of the run for profiler
    # output.
    print("compiler Occupancy [waves/SIMD], per F_7 kernel, from the compile receipt:")
    for name in ("permanent_bipedal7.hip.resource.log",
                 "wave_gf7_equivalence.hip.resource.log"):
        for kernel in read_resource_log(RESOURCES / name):
            print(f"  {kernel['kernel'][:62]:>62} occupancy="
                  f"{kernel['Occupancy [waves/SIMD]']:>3} (compiler, not runtime)"
                  f"  {kernel['log']}")
    tokens = ("rocprof", "rocprofv2", "rocprofv3", "occupanc", "counter",
              "profil", "MeanOccupancyPerCU", "GRBM", "SQ_WAVES")
    artifacts = sorted(
        list(STUDY.glob(f"{RUN}*.csv"))
        + list(STUDY.glob(f"{RUN}*.log"))
        + list(STUDY.glob(f"{RUN}*.txt"))
        + [EQUIV, EQUIV.with_suffix(".log")]
    )
    print("profiler-token scan over every committed artifact of this run:")
    total_hits = 0
    for artifact in artifacts:
        text = artifact.read_text(errors="replace").lower()
        hits = {t: text.count(t.lower()) for t in tokens if t.lower() in text}
        total_hits += sum(hits.values())
        print(f"  {artifact.name:>72} hits={hits or '{}'}")
    print(f"total profiler-token hits across the run's artifacts = {total_hits}")
    assert total_hits == 0, (
        "a run artifact now carries profiler output; section 13 of receipts.md"
        " states that none does and must be revised with it"
    )

    print()
    print("== section 14: exact operation above the sixteen-lane limit ==")
    for path in BIT_SLICED:
        print(f"path {path}")
        for n in ORDERS:
            cell = by_order[n].get(path)
            eq = [r for r in equiv if int(r["n"]) == n and r["backend"] == path]
            eq_text = "no equivalence row"
            if eq:
                eq_text = (f"reference={eq[0]['reference']} matrices={eq[0]['matrices']}"
                           f" mismatches={eq[0]['mismatches']} status={eq[0]['status'][:20]}")
            rate = (f"{float(cell['composite_matrices_per_s']):.4f}" if cell else "not measured")
            batch = cell["batch_size"] if cell else "-"
            print(f"  n={n:>3} grid outcome="
                  f"{'measured' if cell else 'not measured'} M={batch:>6} composite={rate:>14}"
                  f"  {eq_text}")
    print("the sixteen-lane bound that makes this the demonstration it is:")
    for row in equiv:
        if int(row["n"]) > 16 and row["backend"] == "cpu_scalar":
            print(f"  n={row['n']} cpu_scalar: {row['status'][:90]}")

    print()
    print("== section 15: per-launch device work and span against the archived calibration ==")
    print(f"{'n':>3} {'path':>26} {'M':>6} {'M*2^n':>16}"
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
        print(f"{n:>3} {row['backend']:>26} {m:>6} {work:>16.4g}"
              f" {span:>16.4f} {span / ARCHIVED_HANG_BOUNDARY_S:>15.4f}")
    print(f"largest per-launch work in this campaign = {worst_work:.4g}")
    print(f"longest kernel span per launch = {worst_span:.4f} s"
          f" ({worst_span / ARCHIVED_HANG_BOUNDARY_S:.4f} of the archived"
          f" {ARCHIVED_HANG_BOUNDARY_S:.0f} s hang boundary)")

    print()
    print("== section 15: the declared operating point, q=7 n=20 ==")
    declared_n = 20
    cell = by_order[declared_n][best_proto[declared_n][0]]
    rate = float(cell["composite_matrices_per_s"])
    cpu_key, cpu_rate = best_cpu[declared_n]
    reps = int(cell["reps"])
    m = int(cell["batch_size"])
    print(f"best prototype {best_proto[declared_n][0]} M={m} rate={rate:.4f} matrices/s")
    print(f"best applicable in-tree CPU path {cpu_key} rate={cpu_rate:.4f} matrices/s")
    print(f"ratio = {rate / cpu_rate:.4f} (target 1.5)")
    print(f"device_submission_to_kernel_s={cell['device_submission_to_kernel_s']}"
          f" over reps={reps} = {1e6 * float(cell['device_submission_to_kernel_s']) / reps:.4f}"
          f" us/launch")
    print(f"kernel_device_s={cell['kernel_device_s']} over reps={reps}"
          f" = {float(cell['kernel_device_s']) / reps:.6f} s/launch")
    print(f"per-launch work M*2^n = {m * 2**declared_n:.6g}")
    print("every path measured at this order, ranked:")
    for key, row in sorted(
        by_order[declared_n].items(),
        key=lambda kv: -float(kv[1]["composite_matrices_per_s"]),
    ):
        print(f"  {key:>26} {float(row['composite_matrices_per_s']):>14.4f}")
    print("the protocol's own premeasurement configuration for this field is"
          " (7,20) on the accelerator at M=1024:")
    gpu_cell = by_order[declared_n].get("gpu_hip@M=1024")
    if gpu_cell is not None:
        gpu_rate = float(gpu_cell["composite_matrices_per_s"])
        print(f"  gpu_hip@M=1024 {gpu_rate:.4f} matrices/s;"
              f" against {cpu_key} {cpu_rate:.4f} that is {gpu_rate / cpu_rate:.4f}x;"
              f" the prototype is {rate / gpu_rate:.4f}x the accelerator cell")


if __name__ == "__main__":
    main()
