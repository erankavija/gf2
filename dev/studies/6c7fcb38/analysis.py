#!/usr/bin/env python3
"""Derive every table in `receipts.md` from the committed campaign artifacts.

Run from the repository root:

    python3 dev/studies/6c7fcb38/analysis.py

The script reads only committed artifacts and writes nothing: the three
$\\mathbb{F}_7$ CSVs of campaign run `20260814T230032Z-2085453` beside it, the
shared equivalence CSV of the same run under `dev/studies/047b62ed/`, the prior
grid under `dev/studies/b488f02c/`, the compiler kernel-resource logs of
receipt `20260814T172506Z-1610002` in this directory, and the paired profiled
evidence run `20260815T181923Z` in its own subdirectory. Every figure quoted in
`receipts.md` is printed here under the heading that names its section, so a
reviewer can diff prose against data without re-running the measurement.
"""

from __future__ import annotations

import csv
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

# Paired profiled evidence run: a second run of the same hash-pinned binary
# over the same preregistered cells, kept apart from the timing run so counter
# collection cannot perturb the timing evidence, and carrying no timing
# authority of its own (`profiled-20260815T181923Z/provenance.txt`).
PROFILED = STUDY / "profiled-20260815T181923Z"
PROFILED_COUNTERS = PROFILED / "counters-aggregate.csv"
# The traced three-plane passes, one order each, that separate the bit-plane
# staging kernel from the Gray walk inside the harness's single event span.
PROFILED_THREE_PLANE = {
    12: "trace-n12-threeplane",
    16: "trace-n16-threeplane",
    20: "trace-n20-threeplane",
    24: "trace-n24-threeplane",
}
# The nine measured F_7 kernels: short name, the substring identifying the
# kernel in the profiler's demangled `Name` / `Kernel_Name` field, and the
# substring identifying it in the compiler receipt's mangled name. The lookup
# control is templated on word count, so its two instantiations differ only in
# that argument.
MEASURED_KERNELS = (
    ("permanent_bipedal7_kernel",
     "permanent_bipedal7_kernel(", "permanent_bipedal7_kernelPKhiiPy"),
    ("wave_gf7_lookup_table_kernel<1>",
     "wave_gf7_lookup_table_kernel<1u>", "wave_gf7_lookup_table_kernelILj1EE"),
    ("wave_gf7_lookup_table_kernel<2>",
     "wave_gf7_lookup_table_kernel<2u>", "wave_gf7_lookup_table_kernelILj2EE"),
    ("prepare_three_plane_columns",
     "prepare_three_plane_columns(", "prepare_three_plane_columns"),
    ("wave_gf7_three_plane_kernel",
     "wave_gf7_three_plane_kernel(", "wave_gf7_three_plane_kernel"),
    ("gray_update_micro_kernel",
     "gray_update_micro_kernel(", "gray_update_micro_kernel"),
    ("gray_update_compiler_barrier_baseline_kernel",
     "gray_update_compiler_barrier_baseline_kernel(",
     "gray_update_compiler_barrier_baseline_kernel"),
    ("horizontal_product_micro_kernel",
     "horizontal_product_micro_kernel(", "horizontal_product_micro_kernel"),
    ("horizontal_product_compiler_barrier_baseline_kernel",
     "horizontal_product_compiler_barrier_baseline_kernel(",
     "horizontal_product_compiler_barrier_baseline_kernel"),
)
# The runtime's own copy kernel, present in every counter pass, which serves as
# the same-pass control for a counter that reads zero throughout a pass.
RUNTIME_COPY_KERNEL = "__amd_rocclr_copyBuffer"
COUNTER_NAMES = ("MeanOccupancyPerCU", "OccupancyPercent", "SQ_WAVES")
# The GPU agent fields the occupancy reading is normalised against, read from
# the profiler's own per-pass agent report rather than assumed.
AGENT_FIELDS = (
    "Cu_Count",
    "Simd_Count",
    "Simd_Per_Cu",
    "Max_Waves_Per_Simd",
    "Max_Waves_Per_Cu",
    "Wave_Front_Size",
    "Gfx_Target_Version",
)
# `OccupancyPercent` and `MeanOccupancyPerCU` are one quantity in two units on
# this stack: `100*reduce(SQ_WAVE_CYCLES,sum)/reduce(GRBM_GUI_ACTIVE,max)/
# CU_NUM/32` and `reduce(accumulate(SQ_LEVEL_WAVES,HIGH_RES),sum)/
# reduce(GRBM_GUI_ACTIVE,max)/CU_NUM` (`rocprofv3-avail info --pmc`, gfx1030),
# so a reading has to satisfy MeanOccupancyPerCU = OccupancyPercent *
# Max_Waves_Per_Cu / 100, has to stay inside the agent's own per-CU wave-slot
# count, and cannot hold more waves resident than the dispatch launched.
OCCUPANCY_IDENTITY_TOLERANCE = 0.01
# Round 2 has no second counter to check the identity against, so a reading is
# admissible when it stays inside the agent's per-CU wave-slot count and puts no
# more waves resident than the dispatch's own geometry launches. The one-percent
# slack on the second bound absorbs the counter's own rounding; every reading it
# rejects exceeds the launch count by a factor of ten or more.
ROUND2_LAUNCH_TOLERANCE = 0.01

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
# Archived per-launch q=7 work budget and span boundary, both carried as priors
# rather than as established device properties. The q=7 budget is committed at
# 3.5e8 in dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/
# r4_gpu_uniformity_resample.md:225-227.
ARCHIVED_HANG_BOUNDARY_S = 190.0
ARCHIVED_Q7_WORK_BUDGET = 3.5e8
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


def profiled_kernel_stats(pass_name: str) -> tuple[str, dict[str, dict[str, str]]]:
    """rocprofv3's own per-kernel aggregation for one traced pass.

    Returns the committed file's name for citation and its rows keyed by the
    demangled kernel name, so a caller cites the file it read.
    """
    files = sorted((PROFILED / pass_name).glob("*_kernel_stats.csv"))
    assert len(files) == 1, f"{pass_name}: expected one kernel_stats CSV, found {files}"
    with files[0].open() as handle:
        rows = list(csv.DictReader(handle))
    return files[0].name, {row["Name"]: row for row in rows}


def profiled_kernel_row(
    rows: dict[str, dict[str, str]], key: str
) -> dict[str, str]:
    """The one row of a traced pass whose kernel name carries `key`."""
    matches = [row for name, row in rows.items() if key in name]
    assert len(matches) == 1, f"{key}: matched {len(matches)} kernels"
    return matches[0]


def profiled_cell(pass_name: str) -> dict[str, str]:
    """A profiled pass's own workload row: its record, never timing evidence."""
    rows = read_grid(PROFILED / pass_name / "grid.csv")
    assert len(rows) == 1, f"{pass_name}: expected one workload row, found {len(rows)}"
    return rows[0]


def profiled_agent() -> tuple[int, dict[str, str]]:
    """GPU agent geometry the profiler reports, asserted equal in every pass."""
    seen: set[tuple[str, ...]] = set()
    passes = 0
    for path in sorted(PROFILED.glob("*/*_agent_info.csv")):
        passes += 1
        with path.open() as handle:
            for row in csv.DictReader(handle):
                if row["Agent_Type"] == "GPU":
                    seen.add(tuple(row[field] for field in AGENT_FIELDS))
    assert len(seen) == 1, f"passes disagree on the GPU agent: {seen}"
    return passes, dict(zip(AGENT_FIELDS, seen.pop()))


def profiled_counters() -> dict[tuple[str, str], dict[str, dict[str, str]]]:
    """The committed counter aggregate, keyed by (pass, kernel) then counter."""
    with PROFILED_COUNTERS.open() as handle:
        rows = list(csv.DictReader(handle))
    by_kernel: dict[tuple[str, str], dict[str, dict[str, str]]] = {}
    for row in rows:
        by_kernel.setdefault((row["pass"], row["kernel"]), {})[row["counter"]] = row
    return by_kernel


def profiled_round2() -> dict[tuple[str, str, int], list[dict[str, float]]]:
    """Round-2 per-dispatch counters, grouped by pass, kernel and launch width.

    Round 2 collects over dispatch iterations 1-8 of each workload and commits
    the per-dispatch CSVs in full, so no aggregation stands between the receipt
    and the reading. The launch width is the dispatch's own recorded geometry,
    `Grid_Size / Workgroup_Size` workgroups of at most one wavefront each, which
    is the wave count the launch asks for independently of what `SQ_WAVES`
    reports.
    """
    grouped: dict[tuple[str, str, int], list[dict[str, float]]] = {}
    for path in sorted(PROFILED.glob("pmc2-*/*_counter_collection.csv")):
        pass_name = path.parent.name
        with path.open() as handle:
            rows = list(csv.DictReader(handle))
        by_dispatch: dict[int, dict[str, str]] = {}
        for row in rows:
            entry = by_dispatch.setdefault(int(row["Dispatch_Id"]), dict(row))
            entry[row["Counter_Name"]] = row["Counter_Value"]
        for dispatch, row in sorted(by_dispatch.items()):
            short = next(
                (s for s, key, _ in MEASURED_KERNELS if key in row["Kernel_Name"]),
                None,
            )
            if short is None:
                continue
            workgroup = int(row["Workgroup_Size"])
            assert workgroup <= 32, f"{pass_name}: {workgroup} threads exceed one wave"
            launched = int(row["Grid_Size"]) // workgroup
            grouped.setdefault((pass_name, short, launched), []).append({
                "dispatch": dispatch,
                "per_cu": float(row["MeanOccupancyPerCU"]),
                "sq_waves": float(row["SQ_WAVES"]),
            })
    return grouped


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
    # That study daggers every rendering of a nonconforming rate, in its
    # section 4.4 table and in the derived tables and prose that carry the same
    # figure, and its own verify-rendered-stopping-rule.py enforces that
    # document-wide. The set this block derives is a set of cells, so the
    # comparison is against the section 4.4 cells, one per measured cell.
    rendered = (STUDY.parent / "b488f02c" / "feasibility-study.md").read_text()
    start = rendered.index("### 4.4 Measured throughput")
    end = rendered.find("\n### ", start + 1)
    section_44 = rendered[start:end if end != -1 else len(rendered)]
    daggered = sum(
        1
        for line in section_44.splitlines()
        if line.startswith("|")
        for cell in line.split("|")
        if "†" in cell
    )
    print(f"rendered section 4.4 cells carrying a dagger: {daggered}")
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
    print("== section 12: profiled per-kernel split of the three-plane span ==")
    # The paired profiled run resolves the two launches the timing run's event
    # bracket cannot: rocprofv3's own kernel_stats aggregation carries
    # `prepare_three_plane_columns` and `wave_gf7_three_plane_kernel` as
    # separate rows of the same pass. Each pass re-calibrates its own batch
    # from a single-matrix probe, so the profiled M is the profiled cell's and
    # not the timing cell's, and the trace counts the probe and the untimed
    # warm-up as well as the timed repetitions.
    timing_per_launch = {
        int(row["n"]): float(row["kernel_device_s"]) / int(row["reps"])
        for row in device_rows
        if row["backend"] in BIT_SLICED and row["kernel_device_s"]
    }
    timing_batch = {
        int(row["n"]): row["batch_size"]
        for row in device_rows
        if row["backend"] in BIT_SLICED and row["kernel_device_s"]
    }
    print(f"{'n':>3} {'M':>4} {'reps':>6} {'calls':>6} {'prep us':>9} {'walk us':>12}"
          f" {'pair us':>12} {'prep ms':>9} {'walk ms':>10} {'prep share %':>12}"
          f" {'bracket us':>12} {'pair/bracket %':>15} {'timing M':>8} {'timing us':>12}")
    profiled_split: dict[int, tuple[float, ...]] = {}
    profiled_spread: dict[int, tuple[float, ...]] = {}
    for n, pass_name in PROFILED_THREE_PLANE.items():
        stats_file, rows = profiled_kernel_stats(pass_name)
        prep = profiled_kernel_row(rows, "prepare_three_plane_columns(")
        walk = profiled_kernel_row(rows, "wave_gf7_three_plane_kernel(")
        calls = int(prep["Calls"])
        assert calls == int(walk["Calls"]), f"{pass_name}: staged launches unpaired"
        prep_avg = float(prep["AverageNs"]) / 1e3
        walk_avg = float(walk["AverageNs"]) / 1e3
        prep_total = int(prep["TotalDurationNs"]) / 1e6
        walk_total = int(walk["TotalDurationNs"]) / 1e6
        share = 100.0 * prep_total / (prep_total + walk_total)
        cell = profiled_cell(pass_name)
        bracket = 1e6 * float(cell["kernel_device_s"]) / int(cell["reps"])
        profiled_split[n] = (prep_avg, walk_avg, prep_total, walk_total, share)
        profiled_spread[n] = (
            float(prep["MinNs"]) / 1e3, float(prep["MaxNs"]) / 1e3,
            float(walk["MinNs"]) / 1e3, float(walk["MaxNs"]) / 1e3,
            100.0 * (prep_avg + walk_avg) / bracket,
        )
        print(f"{n:>3} {cell['batch_size']:>4} {cell['reps']:>6} {calls:>6}"
              f" {prep_avg:>9.3f} {walk_avg:>12.3f} {prep_avg + walk_avg:>12.3f}"
              f" {prep_total:>9.3f} {walk_total:>10.3f} {share:>12.4f}"
              f" {bracket:>12.3f} {profiled_spread[n][4]:>15.2f}"
              f" {timing_batch[n]:>8} {1e6 * timing_per_launch[n]:>12.4f}"
              f"  {stats_file}")
    print("the launch gap inside one bracket, and the profiled bracket against"
          " the unprofiled timing span at the timing cell's own batch:")
    rendered_gap = {12: (19.991, 1.1279), 16: (17.879, 0.3174),
                    20: (14.846, 1.0000), 24: (12.664, 1.0011)}
    for n, pass_name in PROFILED_THREE_PLANE.items():
        prep_avg, walk_avg, _, _, _ = profiled_split[n]
        bracket = (prep_avg + walk_avg) / (profiled_spread[n][4] / 100.0)
        gap = bracket - (prep_avg + walk_avg)
        timing = 1e6 * timing_per_launch[n]
        print(f"  n={n:>3} gap={gap:>10.3f} us  bracket/timing={bracket / timing:>9.4f}"
              f"  timing/bracket={timing / bracket:>9.4f}"
              f"  relative difference={100.0 * (bracket - timing) / timing:>9.4f} %")
        want_gap, want_ratio = rendered_gap[n]
        assert round(gap, 3) == want_gap and round(bracket / timing, 4) == want_ratio, (
            f"section 12 renders {want_gap} us and {want_ratio} for n={n}"
        )
    print("per-dispatch spread, and the profiler's own StdDev column beside it:")
    for n, pass_name in PROFILED_THREE_PLANE.items():
        _, rows = profiled_kernel_stats(pass_name)
        prep = profiled_kernel_row(rows, "prepare_three_plane_columns(")
        walk = profiled_kernel_row(rows, "wave_gf7_three_plane_kernel(")
        low_prep, high_prep, low_walk, high_walk, _ = profiled_spread[n]
        print(f"  n={n:>3} prep {low_prep:>10.3f}-{high_prep:<10.3f} us"
              f" (StdDev {float(prep['StdDev']) / 1e3:>12.3f} us)"
              f"  walk {low_walk:>10.3f}-{high_walk:<10.3f} us"
              f" (spread {high_walk - low_walk:>10.3f} us,"
              f" StdDev {float(walk['StdDev']) / 1e3:>12.3f} us)")
    # Every figure section 12 renders from this pass set, as written there.
    rendered_split = {
        12: (2.139, 23.262, 116.432, 1266.257, 8.4207),
        16: (2.312, 312.386, 37.624, 5083.143, 0.7347),
        20: (2.657, 4654.140, 4.350, 7618.827, 0.0571),
        24: (3.020, 67665.938, 0.362, 8119.913, 0.0045),
    }
    rendered_spread = {
        12: (1.920, 5.080, 23.080, 24.480, 55.96),
        16: (2.080, 36.001, 311.083, 344.403, 94.62),
        20: (2.560, 17.000, 4609.434, 8688.478, 99.68),
        24: (2.920, 4.480, 67496.502, 67872.752, 99.98),
    }
    for n, expected in rendered_split.items():
        for values, wanted, places in (
            (profiled_split[n], expected, (3, 3, 3, 3, 4)),
            (profiled_spread[n], rendered_spread[n], (3, 3, 3, 3, 2)),
        ):
            for value, want, place in zip(values, wanted, places):
                assert round(value, place) == want, (
                    f"section 12 renders {want} for n={n} where the artifact now"
                    f" gives {round(value, place)}"
                )
    print("preparation stays inside a launch-latency floor while the walk grows"
          " with the order:")
    orders = sorted(PROFILED_THREE_PLANE)
    for lower, upper in zip(orders, orders[1:]):
        print(f"  n={lower}->{upper} prep x{profiled_split[upper][0] / profiled_split[lower][0]:.3f}"
              f" walk x{profiled_split[upper][1] / profiled_split[lower][1]:.2f}"
              f" share /{profiled_split[lower][4] / profiled_split[upper][4]:.1f}")

    print()
    print("== section 13: occupancy evidence in the timing run's own artifacts ==")
    # Achieved occupancy comes from the paired profiled run, in the blocks
    # below. This block establishes the other half of the criterion's pairing
    # and the separation between the two runs: the compiler figure, which is a
    # compile-time prediction and not a runtime observation, and a token scan
    # over every committed artifact of the timing run, which carries no
    # profiler output at all.
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
    print(f"profiler-token scan over every committed artifact of run {RUN}:")
    total_hits = 0
    for artifact in artifacts:
        text = artifact.read_text(errors="replace").lower()
        hits = {t: text.count(t.lower()) for t in tokens if t.lower() in text}
        total_hits += sum(hits.values())
        print(f"  {artifact.name:>72} hits={hits or '{}'}")
    print(f"total profiler-token hits across that run's artifacts = {total_hits}")
    assert total_hits == 0, (
        "a run artifact now carries profiler output; section 13 of receipts.md"
        " states that none does and must be revised with it"
    )

    print()
    print("== section 13: passes of the paired profiled run, and their exits ==")
    for log_name in ("run.log", "run2.log"):
        text = (PROFILED / log_name).read_text().splitlines()
        commands = [line for line in text if line.startswith("COMMAND[")]
        exits = [line for line in text if line.startswith("EXIT[")]
        codes: dict[str, int] = {}
        for line in exits:
            code = int(line.split("]:", 1)[1].split()[0])
            codes[code] = codes.get(code, 0) + 1
        assert len(commands) == len(exits), f"{log_name}: {len(commands)} commands, {len(exits)} exits"
        print(f"  {log_name}: {len(commands)} passes, exits "
              + " ".join(f"{code}x{count}" for code, count in sorted(codes.items())))
    assert [len([line for line in (PROFILED / name).read_text().splitlines()
                 if line.startswith("COMMAND[")]) for name in ("run.log", "run2.log")] \
        == [18, 11], "section 13 renders eighteen round-1 passes and eleven round-2 passes"

    print("== section 13: device geometry the occupancy counters normalise against ==")
    agent_passes, agent = profiled_agent()
    cu_count = int(agent["Cu_Count"])
    waves_per_cu = int(agent["Max_Waves_Per_Cu"])
    device_slots = cu_count * waves_per_cu
    print("  " + " ".join(f"{field}={agent[field]}" for field in AGENT_FIELDS)
          + f" device wave slots={device_slots}"
          + f" (one agent report, identical in all {agent_passes} pass directories)")
    assert cu_count * int(agent["Simd_Per_Cu"]) == int(agent["Simd_Count"])
    assert int(agent["Simd_Per_Cu"]) * int(agent["Max_Waves_Per_Simd"]) == waves_per_cu

    print()
    print("== section 13: achieved occupancy from the paired profiled counters ==")
    counters = profiled_counters()
    present = {counter for readings in counters.values() for counter in readings}
    assert present == set(COUNTER_NAMES), f"the collected counter set is {sorted(present)}"
    covered = {
        short
        for short, profiler_key, _ in MEASURED_KERNELS
        for (_, kernel) in counters
        if profiler_key in kernel
    }
    missing = {short for short, _, _ in MEASURED_KERNELS} - covered
    assert not missing, f"the counter aggregate covers no dispatch of {sorted(missing)}"
    print(f"counters collected: {' '.join(sorted(present))};"
          f" F_7 kernels covered: {len(covered)} of {len(MEASURED_KERNELS)}")

    # A reading is an occupancy measurement only if it survives the counters'
    # own definitions: the two derived counters are one quantity in two units,
    # neither may exceed the agent's wave-slot count, and a dispatch cannot
    # hold more waves resident than it launched.
    def occupancy_reading(pass_name: str, kernel: str) -> dict[str, object]:
        readings = counters[(pass_name, kernel)]
        occupancy = float(readings["OccupancyPercent"]["mean"])
        per_cu = float(readings["MeanOccupancyPerCU"]["mean"])
        per_cu_max = float(readings["MeanOccupancyPerCU"]["max"])
        launched = float(readings["SQ_WAVES"]["mean"])
        launched_min = float(readings["SQ_WAVES"]["min"])
        launched_max = float(readings["SQ_WAVES"]["max"])
        resident = per_cu * cu_count
        identity = per_cu / (occupancy * waves_per_cu / 100.0) if occupancy else math.inf
        if occupancy == 0.0:
            reason = "OccupancyPercent reads exactly zero on every dispatch"
        elif abs(identity - 1.0) > OCCUPANCY_IDENTITY_TOLERANCE:
            reason = f"the two counters disagree by a factor of {identity:.4g}"
        elif per_cu_max > waves_per_cu:
            reason = f"more than {waves_per_cu} waves on a CU, which has that many slots"
        elif resident > launched:
            reason = "more waves resident than the dispatch launched"
        else:
            reason = ""
        return {
            "dispatches": int(readings["SQ_WAVES"]["dispatches"]),
            "occupancy": occupancy,
            "per_cu": per_cu,
            "per_cu_max": per_cu_max,
            "launched": launched,
            "launched_min": launched_min,
            "launched_max": launched_max,
            "resident": resident,
            "identity": identity,
            "fails": tuple(
                index
                for index, failed in enumerate(
                    (
                        occupancy == 0.0
                        or abs(identity - 1.0) > OCCUPANCY_IDENTITY_TOLERANCE,
                        per_cu_max > waves_per_cu,
                        resident > launched,
                    ),
                    start=1,
                )
                if failed
            ),
            "reason": reason,
            "vgpr": int(readings["SQ_WAVES"]["VGPR_Count"]),
            "scratch": int(readings["SQ_WAVES"]["Scratch_Size"]),
            "lds": int(readings["SQ_WAVES"]["LDS_Block_Size"]),
        }

    print(f"{'pass':>19} {'kernel':>52} {'disp':>7} {'launched':>12} {'occ %':>10}"
          f" {'waves/CU':>10} {'max waves/CU':>13} {'resident':>11} {'identity':>10}"
          f"  fails  verdict")
    readings: dict[str, list[tuple[str, dict[str, object]]]] = {}
    for pass_name, kernel in sorted(counters):
        for short, profiler_key, _ in MEASURED_KERNELS:
            if profiler_key not in kernel:
                continue
            reading = occupancy_reading(pass_name, kernel)
            readings.setdefault(short, []).append((pass_name, reading))
            fails = ",".join(str(index) for index in reading["fails"]) or "-"
            print(f"{pass_name:>19} {short:>52} {reading['dispatches']:>7}"
                  f" {reading['launched']:>12.2f} {reading['occupancy']:>10.6f}"
                  f" {reading['per_cu']:>10.4f} {reading['per_cu_max']:>13.4f}"
                  f" {reading['resident']:>11.2f}"
                  f" {reading['identity']:>10.4g}  {fails:>5}  "
                  + (reading["reason"] or "measures achieved occupancy"))
    print(f"the same passes' runtime copy kernel, the same-pass control for a"
          f" counter that reads zero throughout a pass:")
    for pass_name, kernel in sorted(counters):
        if kernel != RUNTIME_COPY_KERNEL:
            continue
        occupancy = counters[(pass_name, kernel)]["OccupancyPercent"]
        print(f"  {pass_name:>19} {RUNTIME_COPY_KERNEL} OccupancyPercent"
              f" mean={occupancy['mean']} min={occupancy['min']} max={occupancy['max']}")

    compiler = {
        kernel["kernel"]: kernel
        for name in RESOURCE_LOGS
        for kernel in read_resource_log(RESOURCES / name)
    }
    print("round-1 verdict per measured kernel, beside the compiler's"
          " per-SIMD prediction. The prediction is a per-SIMD residency ceiling,"
          " so its device-wide share is the ceiling divided by"
          f" Max_Waves_Per_Simd={agent['Max_Waves_Per_Simd']}, reachable only by a"
          f" grid wide enough to fill all {device_slots} slots:")
    print(f"{'kernel':>52} {'waves/SIMD':>11} {'predicted %':>12} {'round-1 %':>11}"
          f" {'round-1 waves/CU':>18}  evidence")
    achieved: dict[str, tuple[str, dict[str, object]]] = {}
    for short, profiler_key, compiler_key in MEASURED_KERNELS:
        usable = [(p, r) for p, r in readings[short] if not r["reason"]]
        assert len(usable) <= 1, f"{short}: {len(usable)} usable readings"
        entry = next(k for name, k in compiler.items() if compiler_key in name)
        predicted = int(entry["Occupancy [waves/SIMD]"])
        share = 100.0 * predicted / int(agent["Max_Waves_Per_Simd"])
        if usable:
            pass_name, reading = usable[0]
            achieved[short] = (pass_name, reading)
            print(f"{short:>52} {predicted:>11} {share:>12.1f}"
                  f" {reading['occupancy']:>11.6f}"
                  f" {reading['per_cu']:>18.6f}  {pass_name},"
                  f" {reading['dispatches']} dispatches,"
                  f" {reading['launched']:.0f} waves launched")
        else:
            why = "; ".join(f"{p}: {r['reason']}" for p, r in readings[short])
            print(f"{short:>52} {predicted:>11} {share:>12.1f}"
                  f" {'no round-1 reading':>18} {'':>11}  {why}")

    # Every figure section 13's counter table renders, as written there:
    # (pass, kernel) -> dispatches, mean SQ_WAVES, mean OccupancyPercent, mean
    # MeanOccupancyPerCU, the resident waves that implies, and whether the
    # reading survives the three tests.
    rendered_readings = {
        ("pmc-grayupdate", "gray_update_micro_kernel"):
            (31, 1.00, 0.039062, 0.012500, 0.012500, 1.00, ()),
        ("pmc-grayupdate", "gray_update_compiler_barrier_baseline_kernel"):
            (31, 1.00, 0.039062, 0.012500, 0.012500, 1.00, ()),
        ("pmc-horizprod", "horizontal_product_micro_kernel"):
            (105430, 441106.20, 1.050744, 666.348961, 17172.489033, 53307.92, (1, 2)),
        ("pmc-horizprod", "horizontal_product_compiler_barrier_baseline_kernel"):
            (105430, 351235.15, 0.976263, 612.196504, 14281.645119, 48975.72, (1, 2)),
        ("pmc-n12-gpuhip", "permanent_bipedal7_kernel"):
            (466, 525.74, 5.743100, 5.847435, 40.042924, 467.79, (1, 2)),
        ("pmc-n12-lookup", "wave_gf7_lookup_table_kernel<1>"):
            (2807, 1554.45, 0.0, 147.504202, 3512.585827, 11800.34, (1, 2, 3)),
        ("pmc-n12-threeplane", "prepare_three_plane_columns"):
            (33710, 2374.38, 0.021388, 17.578662, 5591.426145, 1406.29, (1, 2)),
        ("pmc-n12-threeplane", "wave_gf7_three_plane_kernel"):
            (33710, 34421.18, 0.072084, 262.625609, 4504.360769, 21010.05, (1, 2)),
        ("pmc-n20-gpuhip", "permanent_bipedal7_kernel"):
            (13, 590.85, 0.0, 3280.454195, 3461.399003, 262436.34, (1, 2, 3)),
        ("pmc-n20-lookup", "wave_gf7_lookup_table_kernel<2>"):
            (115, 28.76, 1.114438, 0.356620, 0.360673, 28.53, ()),
        ("pmc-n20-threeplane", "prepare_three_plane_columns"):
            (1636, 12.99, 0.064581, 59.368457, 209.134371, 4749.48, (1, 2, 3)),
        ("pmc-n20-threeplane", "wave_gf7_three_plane_kernel"):
            (1636, 12.99, 0.284039, 1167.382432, 3312.290413, 93390.59, (1, 2, 3)),
    }
    rendered_keys = {
        (pass_name, short)
        for short, entries in readings.items()
        for pass_name, _ in entries
    }
    assert rendered_keys == set(rendered_readings), (
        "section 13 renders a different reading set than the aggregate now holds:"
        f" {sorted(rendered_keys ^ set(rendered_readings))}"
    )
    for short, entries in readings.items():
        for pass_name, reading in entries:
            expected = rendered_readings[(pass_name, short)]
            got = (
                reading["dispatches"],
                round(float(reading["launched"]), 2),
                round(float(reading["occupancy"]), 6),
                round(float(reading["per_cu"]), 6),
                round(float(reading["per_cu_max"]), 6),
                round(float(reading["resident"]), 2),
                reading["fails"],
            )
            assert got == expected, (
                f"section 13 renders {expected} for {short} in {pass_name} where"
                f" the artifact now gives {got}"
            )
    print("waves launched per dispatch, minimum and maximum over each pass:")
    for short, entries in readings.items():
        for pass_name, reading in entries:
            print(f"  {pass_name:>19} {short:>52}"
                  f" {reading['launched_min']:>12.0f} {reading['launched_max']:>12.0f}")
    for short in ("horizontal_product_micro_kernel",
                  "horizontal_product_compiler_barrier_baseline_kernel"):
        reading = next(r for p, r in readings[short] if p == "pmc-horizprod")
        assert int(reading["launched_min"]) == 31, (
            f"section 13 renders 31 as the smallest {short} reading"
        )
    rendered_launched = {
        ("pmc-n12-threeplane", "prepare_three_plane_columns"): 110594,
        ("pmc-n12-threeplane", "wave_gf7_three_plane_kernel"): 393715,
        ("pmc-horizprod", "horizontal_product_micro_kernel"): 19720463,
        ("pmc-horizprod", "horizontal_product_compiler_barrier_baseline_kernel"): 8147141,
    }
    for (pass_name, short), want in rendered_launched.items():
        reading = next(r for p, r in readings[short] if p == pass_name)
        assert int(reading["launched_max"]) == want, (
            f"section 13 renders {want} waves for {short} in {pass_name}"
        )

    # The three readings section 13 fills the achieved column from, and the
    # span of the unit-identity failure it reports for the rest.
    assert set(achieved) == {
        "wave_gf7_lookup_table_kernel<2>",
        "gray_update_micro_kernel",
        "gray_update_compiler_barrier_baseline_kernel",
    }, f"the measurable kernel set is now {sorted(achieved)}"
    finite = [
        float(reading["identity"])
        for entries in readings.values()
        for _, reading in entries
        if math.isfinite(float(reading["identity"])) and reading["reason"]
    ]
    print(f"unit-identity failure spans {min(finite):.4g} to {max(finite):.4g}"
          f" over {len(finite)} readings; two further readings hold"
          f" OccupancyPercent at exactly zero")
    assert round(min(finite), 1) == 3.2 and round(max(finite) / 1000.0, 1) == 12.8, (
        "section 13 states the identity failure spans 3.2 to 1.28e4"
    )

    print()
    print("== section 13: round-2 early-iteration readings ==")
    # Round 2 re-collects over dispatch iterations 1-8, where round 1's derived
    # counters are still exact. OccupancyPercent reads zero under
    # iteration-range collection on this stack and is omitted, so the unit
    # identity is unavailable and admissibility is the physical test set the
    # run's provenance records, applied per dispatch: the reading inside the
    # agent's per-CU wave-slot count, and no more waves resident than the
    # dispatch's own geometry launches.
    round2 = profiled_round2()
    # The horizontal-product isolate alternates two launch widths within one
    # repetition, one wave per sample of each branch, so its dispatches band by
    # branch against the isolate's 4096-sample batch rather than by exact width.
    def band(pass_name: str, launched: int) -> str:
        if pass_name != "pmc2-horizprod":
            return f"{launched} waves"
        return "zero-fast branch" if launched > 2048 else "nonzero-slow branch"

    banded: dict[tuple[str, str, str], list[dict[str, float]]] = {}
    for (pass_name, short, launched), entries in round2.items():
        for entry in entries:
            entry["launched"] = float(launched)
            banded.setdefault((pass_name, short, band(pass_name, launched)), []).append(entry)
    print(f"{'pass':>21} {'kernel':>52} {'band':>19} {'disp':>5} {'launched':>15}"
          f" {'waves/CU mean':>14} {'min':>12} {'max':>12} {'resident':>10}"
          f" {'residency':>13} {'occupancy %':>12}  excluded")
    round2_reported: dict[tuple[str, str, str], tuple[float, ...]] = {}
    round2_excluded: dict[tuple[str, str, str], list[float]] = {}
    for key, entries in sorted(banded.items()):
        good = [
            entry for entry in entries
            if 0.0 < entry["per_cu"] <= waves_per_cu
            and entry["per_cu"] * cu_count
            <= entry["launched"] * (1.0 + ROUND2_LAUNCH_TOLERANCE)
        ]
        excluded = [entry["per_cu"] for entry in entries if entry not in good]
        round2_excluded[key] = excluded
        if not good:
            print(f"{key[0]:>21} {key[1]:>52} {key[2]:>19} {0:>5}"
                  f" {'-':>15} {'-':>14} {'-':>12} {'-':>12} {'-':>10} {'-':>10}"
                  f" {'-':>12}  every dispatch inadmissible:"
                  f" {' '.join(f'{value:.4g}' for value in excluded)}")
            continue
        values = [entry["per_cu"] for entry in good]
        widths = {int(entry["launched"]) for entry in good}
        mean = sum(values) / len(values)
        resident = [value * cu_count for value in values]
        residency = [
            value * cu_count / entry["launched"] for value, entry in zip(values, good)
        ]
        share = [100.0 * value * cu_count / device_slots for value in values]
        round2_reported[key] = (
            len(good), mean, min(values), max(values),
            min(resident), max(resident), min(residency), max(residency),
            min(share), max(share),
        )
        span = "-".join(str(w) for w in sorted(widths))
        mean_share = 100.0 * mean * cu_count / device_slots
        print(f"{key[0]:>21} {key[1]:>52} {key[2]:>19} {len(good):>5} {span:>15}"
              f" {mean:>14.6f} {min(values):>12.6f} {max(values):>12.6f}"
              f" {min(resident):>10.2f} {min(residency):>6.4f}-{max(residency):<6.4f}"
              f" {mean_share:>12.4f}"
              f"  {len(excluded)} of {len(entries)}"
              + (f": {' '.join(f'{value:.4g}' for value in excluded)}" if excluded else ""))
    accepted_excess = max(
        (entry["per_cu"] * cu_count / entry["launched"]
         for entries in banded.values() for entry in entries
         if 0.0 < entry["per_cu"] <= waves_per_cu
         and entry["per_cu"] * cu_count
         <= entry["launched"] * (1.0 + ROUND2_LAUNCH_TOLERANCE)),
    )
    rejected_factor = min(
        (entry["per_cu"] * cu_count / entry["launched"]
         for entries in banded.values() for entry in entries
         if not (0.0 < entry["per_cu"] <= waves_per_cu
                 and entry["per_cu"] * cu_count
                 <= entry["launched"] * (1.0 + ROUND2_LAUNCH_TOLERANCE))),
    )
    print(f"largest residency any accepted reading shows = {accepted_excess:.9f}"
          f" of the waves launched; smallest of any rejected = {rejected_factor:.1f}x")
    assert round(accepted_excess, 6) == 1.000005 and rejected_factor > 10.0, (
        "section 13 states that the accepted readings exceed the launch count by"
        " at most five parts per million and the rejected ones by tenfold or more"
    )
    probes = [
        stats[1] for key, stats in round2_reported.items() if key[2] == "1 waves"
        and key[0] != "pmc2-grayupdate"
    ]
    print(f"single-matrix probe dispatches, admissible ones: {len(probes)} readings"
          f" from {min(probes):.6f} to {max(probes):.6f} waves per CU")
    assert (round(min(probes), 6), round(max(probes), 6)) == (0.003417, 0.012473), (
        "section 13 renders the probe readings as 0.003417 to 0.012473 waves per CU"
    )
    covered_round2 = {short for _, short, _ in round2_reported}
    missing_round2 = {short for short, _, _ in MEASURED_KERNELS} - covered_round2
    assert not missing_round2, (
        "section 13 reports an admissible round-2 reading for every measured"
        f" kernel; none for {sorted(missing_round2)}"
    )
    print(f"kernels with an admissible round-2 reading: {len(covered_round2)}"
          f" of {len(MEASURED_KERNELS)}")
    print("achieved occupancy per measured kernel, as a share of the device's"
          f" {device_slots} wave slots:")
    for short, _, compiler_key in MEASURED_KERNELS:
        entry = next(k for name, k in compiler.items() if compiler_key in name)
        predicted = int(entry["Occupancy [waves/SIMD]"])
        figures = [
            f"{100.0 * stats[1] * cu_count / device_slots:.4f} % ({key[0]},"
            f" {key[2]}, {stats[0]} dispatches)"
            for key, stats in sorted(round2_reported.items())
            if key[1] == short and not (key[2] == "1 waves"
                                        and key[0] != "pmc2-grayupdate")
        ]
        print(f"  {short:>52} predicted {predicted:>2} waves/SIMD"
              f" = {100.0 * predicted / int(agent['Max_Waves_Per_Simd']):>5.1f} %;"
              f" achieved " + "; ".join(figures))
    # Every round-2 figure section 13 renders: (pass, kernel, band) ->
    # dispatches, mean/min/max waves per CU, and the residency and device-slot
    # shares those imply.
    rendered_round2 = {
        ("pmc2-grayupdate", "gray_update_micro_kernel", "1 waves"):
            (8, 0.012500, 0.0125, 0.0125, 1.0000, 1.0000, 0.0391, 0.0391),
        ("pmc2-grayupdate", "gray_update_compiler_barrier_baseline_kernel", "1 waves"):
            (8, 0.012500, 0.0125, 0.0125, 1.0000, 1.0000, 0.0391, 0.0391),
        ("pmc2-n12-gpuhip", "permanent_bipedal7_kernel", "256 waves"):
            (7, 2.635082, 2.6239, 2.6454, 0.8200, 0.8267, 8.1998, 8.2668),
        ("pmc2-n20-gpuhip", "permanent_bipedal7_kernel", "256 waves"):
            (5, 2.712428, 2.6392, 3.0004, 0.8248, 0.9376, 8.2476, 9.3762),
        ("pmc2-n20-gpuhip", "permanent_bipedal7_kernel", "1024 waves"):
            (1, 8.791383, 8.7914, 8.7914, 0.6868, 0.6868, 27.4731, 27.4731),
        ("pmc2-n12-lookup", "wave_gf7_lookup_table_kernel<1>", "1614 waves"):
            (7, 15.813119, 15.7671, 15.8885, 0.7815, 0.7875, 49.2721, 49.6517),
        ("pmc2-n20-lookup", "wave_gf7_lookup_table_kernel<2>", "28 waves"):
            (7, 0.347347, 0.3468, 0.3478, 0.9908, 0.9937, 1.0837, 1.0869),
        ("pmc2-n16-threeplane", "prepare_three_plane_columns", "14 waves"):
            (7, 0.040845, 0.0379, 0.0475, 0.2166, 0.2716, 0.1185, 0.1485),
        ("pmc2-n16-threeplane", "wave_gf7_three_plane_kernel", "14 waves"):
            (7, 0.168679, 0.1684, 0.1690, 0.9623, 0.9655, 0.5263, 0.5280),
        ("pmc2-n20-threeplane", "prepare_three_plane_columns", "13 waves"):
            (7, 0.042929, 0.0399, 0.0481, 0.2458, 0.2958, 0.1248, 0.1502),
        ("pmc2-n20-threeplane", "wave_gf7_three_plane_kernel", "13 waves"):
            (7, 0.161383, 0.1612, 0.1616, 0.9920, 0.9945, 0.5038, 0.5050),
        ("pmc2-horizprod", "horizontal_product_micro_kernel", "zero-fast branch"):
            (4, 6.824382, 6.2930, 8.2150, 0.1450, 0.1913, 19.6655, 25.6717),
        ("pmc2-horizprod", "horizontal_product_micro_kernel", "nonzero-slow branch"):
            (4, 2.065386, 2.0040, 2.1411, 0.2573, 0.2600, 6.2624, 6.6911),
        ("pmc2-horizprod", "horizontal_product_compiler_barrier_baseline_kernel", "zero-fast branch"):
            (4, 5.848223, 5.5490, 6.2895, 0.1284, 0.1465, 17.3407, 19.6546),
        ("pmc2-horizprod", "horizontal_product_compiler_barrier_baseline_kernel", "nonzero-slow branch"):
            (4, 1.515651, 1.4241, 1.6308, 0.1803, 0.1974, 4.4503, 5.0964),
    }
    for key, expected in rendered_round2.items():
        assert key in round2_reported, f"section 13 renders a reading for {key}"
        count, mean, low, high, _, _, res_low, res_high, share_low, share_high = \
            round2_reported[key]
        got = (count, round(mean, 6), round(low, 4), round(high, 4),
               round(res_low, 4), round(res_high, 4),
               round(share_low, 4), round(share_high, 4))
        assert got == expected, (
            f"section 13 renders {expected} for {key} where the artifact now"
            f" gives {got}"
        )
    # The dispatches the physical tests exclude, which section 13 reports as the
    # round-2 half of the falsification rather than dropping.
    rendered_excluded = {
        ("pmc2-n12-threeplane", "prepare_three_plane_columns", "14 waves"): 7,
        ("pmc2-n12-threeplane", "wave_gf7_three_plane_kernel", "1 waves"): 1,
        ("pmc2-n12-threeplane", "wave_gf7_three_plane_kernel", "14 waves"): 7,
        ("pmc2-n20-gpuhip", "permanent_bipedal7_kernel", "1 waves"): 1,
        ("pmc2-n20-gpuhip", "permanent_bipedal7_kernel", "256 waves"): 1,
    }
    assert {key: len(values) for key, values in round2_excluded.items() if values} \
        == rendered_excluded, (
        "section 13 renders a different set of excluded round-2 dispatches than"
        f" the artifact gives: {[(k, len(v)) for k, v in round2_excluded.items() if v]}"
    )

    print("round-2 SQ_WAVES diagnostic against the same dispatch's launch geometry"
          " (not an admissibility test), which it reproduces on the two isolate"
          " passes and breaks elsewhere:")
    for (pass_name, short, launched), entries in sorted(round2.items()):
        sq_readings = {int(entry["sq_waves"]) for entry in entries}
        agrees = sq_readings == {launched}
        print(f"  {pass_name:>21} {short:>52} launched={launched:>8}"
              f" SQ_WAVES min={min(sq_readings):>10} max={max(sq_readings):>10}"
              f"  {'agrees' if agrees else 'breaks'}")
    breaking = {
        pass_name for (pass_name, _, launched), entries in round2.items()
        if {int(entry["sq_waves"]) for entry in entries} != {launched}
    }
    agreeing = {pass_name for pass_name, _, _ in round2} - breaking
    print(f"round-2 passes whose SQ_WAVES matches the geometry throughout:"
          f" {len(agreeing)} of {len(agreeing) + len(breaking)};"
          f" breaking: {sorted(breaking)}")
    assert breaking == {"pmc2-n16-threeplane", "pmc2-n20-threeplane"}, (
        "section 13 states that round-2 SQ_WAVES breaks in exactly the two"
        f" three-plane passes at n=16 and n=20: {sorted(breaking)}"
    )

    # The rocprofv2 capability probe's four captured dispatches, which section 17
    # cites for the launch-time LDS allocation it records before the abort.
    probe_csv = (PROFILED / "rocprofv2-n20-threeplane" / "pmc_1"
                 / "results_v2-n20-threeplane.csv")
    with probe_csv.open() as handle:
        probe_rows = [row for row in csv.DictReader(handle) if row.get("Kernel_Name")]
    walk = [row for row in probe_rows
            if "wave_gf7_three_plane_kernel" in row["Kernel_Name"]]
    print(f"rocprofv2 capability probe: {len(probe_rows)} dispatches captured;"
          f" wave_gf7_three_plane_kernel LDS_Per_Workgroup="
          f"{ {row['LDS_Per_Workgroup'] for row in walk} }")
    assert len(probe_rows) == 4 and {row["LDS_Per_Workgroup"] for row in walk} == {"512"}, (
        "section 17 cites four captured dispatches and 512 bytes per workgroup"
    )
    assert 512 == 128 * math.ceil(3 * 20 * 8 / 128), (
        "section 17 states 512 is 24n = 480 bytes at a 128-byte granularity"
    )

    print("traced per-dispatch duration of every profiled kernel, from each"
          " trace pass's own kernel_stats aggregation:")
    traced_average_s: dict[tuple[str, str], float] = {}
    for directory in sorted(PROFILED.glob("trace-*")):
        stats_file, rows = profiled_kernel_stats(directory.name)
        for name, row in sorted(rows.items()):
            if name == RUNTIME_COPY_KERNEL:
                continue
            short = next((s for s, key, _ in MEASURED_KERNELS if key in name), name)
            average_s = float(row["AverageNs"]) / 1e9
            traced_average_s[(directory.name, short)] = average_s
            print(f"  {directory.name:>21} {short:>52} calls={row['Calls']:>7}"
                  f" average={average_s:>13.9f} s = {1e6 * average_s:>13.3f} us"
                  f"  {stats_file}")
    longest = max(traced_average_s, key=traced_average_s.get)
    print(f"longest traced dispatch: {longest[1]} in {longest[0]} at"
          f" {traced_average_s[longest]:.9f} s")
    assert longest == ("trace-n20-gpuhip", "permanent_bipedal7_kernel"), (
        "section 13 names the shipped kernel at n=20 as the run's longest dispatch"
    )
    assert round(
        traced_average_s[("trace-horizprod", "horizontal_product_micro_kernel")], 9
    ) == 0.000003069, (
        "section 13 renders 3.069 us as the horizontal-product micro kernel's"
        " traced dispatch length"
    )
    assert round(traced_average_s[longest], 9) == 12.886639517, (
        "section 13 renders 12.886639517 s as that dispatch length"
    )

    print("runtime per-dispatch register allocation against the compiler's"
          " per-lane count:")
    for short, profiler_key, compiler_key in MEASURED_KERNELS:
        entry = next(k for name, k in compiler.items() if compiler_key in name)
        compiled = int(entry["VGPRs"])
        runtime = {r["vgpr"] for _, r in readings[short]}
        assert len(runtime) == 1, f"{short}: passes disagree on VGPR_Count {runtime}"
        allocated = runtime.pop()
        granularity = 8 * math.ceil(compiled / 8)
        print(f"  {short:>52} compiler VGPRs={compiled:>3}"
              f" runtime VGPR_Count={allocated:>3} ceil to 8 = {granularity:>3}"
              f" {'agree' if allocated == granularity else 'DISAGREE'}")
        assert allocated == granularity, (
            f"{short}: the runtime allocation no longer rounds the compiler's"
            f" per-lane count up to a multiple of eight"
        )

    print("waves launched per dispatch against the profiled cell's batch, which"
          " both mappings launch one wave per matrix for:")
    for pass_name in sorted({p for p, _ in counters}):
        workload = PROFILED / pass_name / "grid.csv"
        if not workload.exists():
            continue
        batches = {int(row["batch_size"]) for row in read_grid(workload)}
        for (candidate, kernel) in sorted(counters):
            if candidate != pass_name or kernel == RUNTIME_COPY_KERNEL:
                continue
            largest = int(float(counters[(pass_name, kernel)]["SQ_WAVES"]["max"]))
            short = next(s for s, key, _ in MEASURED_KERNELS if key in kernel)
            print(f"  {pass_name:>19} {short:>52}"
                  f" batches={','.join(str(b) for b in sorted(batches)):>9}"
                  f" SQ_WAVES max={largest:>8}"
                  f" {'equals the largest batch' if largest in batches else 'exceeds the launch geometry'}")

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
    print(f"largest per-launch work in this campaign = {worst_work:.4g}"
          f" ({worst_work / ARCHIVED_Q7_WORK_BUDGET:.2f}x archived q=7 work budget)")
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
    work = m * 2**declared_n
    print(f"per-launch work M*2^n = {work:.6g}"
          f" ({work / ARCHIVED_Q7_WORK_BUDGET:.4f} of archived q=7 work budget)")
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
