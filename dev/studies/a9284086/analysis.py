#!/usr/bin/env python3
"""Derive every table in `receipt.md` from the committed artifacts.

Run from the repository root:

    python3 dev/studies/a9284086/analysis.py

The script reads only committed artifacts and writes nothing: the three field
campaigns' grid CSVs (`dev/studies/047b62ed/`, `dev/studies/91605d4d/`,
`dev/studies/6c7fcb38/`), the compiler kernel-resource receipt those campaigns
share, the committed F_7 paired profiled evidence run, the prior sustained
receipt under `dev/studies/b488f02c/`, and this study's own profiled evidence
run in its subdirectory. Every figure quoted in `receipt.md` is printed here
under the heading that names its section, and the structural checks the receipt
relies on are `assert`ed, so the script fails rather than printing a stale
number.
"""

from __future__ import annotations

import csv
import pathlib

STUDY = pathlib.Path(__file__).resolve().parent
STUDIES = STUDY.parent
RUN = "permanent-campaign-20260814T230032Z-2085453"
RESOURCES = STUDIES / "6c7fcb38" / "hip-resource-usage-20260814T172506Z-1610002"
F7_PROFILED = STUDIES / "6c7fcb38" / "profiled-20260815T181923Z"
SUSTAINED = STUDIES / "b488f02c" / "sustained-2026-08-07.csv"
PROFILED = STUDY / "profiled-20260816T002340Z"

# The grid writer emits `phase_timing_note` and `note` unquoted and both carry
# literal commas on the out-of-field rows, so those lines hold more fields than
# the header names (bug 3ea21d74). `seed_root` is the one column whose value has
# a fixed prefix, so it anchors the split.
GRID_NOTE_COLUMN = "phase_timing_note"
GRID_SEED_ROOT_COLUMN = "seed_root"
GRID_SEED_ROOT_PREFIX = "0xb488f02c"

ORDERS = (12, 16, 20, 24, 28)
# gfx1030 wave width and per-CU/per-SIMD wave-slot counts. Every figure below is
# read from the profiler's own per-pass agent report rather than assumed; these
# names are the fields read.
AGENT_FIELDS = (
    "Cu_Count",
    "Simd_Count",
    "Simd_Per_Cu",
    "Max_Waves_Per_Simd",
    "Max_Waves_Per_Cu",
    "Wave_Front_Size",
    "Lds_Size_In_Kb",
    "Gfx_Target_Version",
)
# Round-2 admissibility, the policy committed in
# `dev/studies/6c7fcb38/receipts.md` section 13 and made authoritative by the
# 2026-08-16 amendment in that run's `provenance.txt`: a reading counts when
# `MeanOccupancyPerCU` lies inside (0, Max_Waves_Per_Cu] and puts no more waves
# resident than the dispatch's own launch geometry supplies, with a one-percent
# slack for the counter's rounding.
LAUNCH_TOLERANCE = 0.01

# Archived prior calibration, cited as a prior rather than as an established
# device property (dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/
# r4_gpu_uniformity_resample.md section 2.5).
ARCHIVED_SPAN_BOUNDARY_S = 190.0
ARCHIVED_WORK_BUDGET = {3: 4.0e9, 5: 1.3e9, 7: 3.5e8}
# The one recorded device fault, q=3 n=24 M=4096 on the shipped kernel
# (dev/studies/b488f02c/gpu-hang-2026-08-07.log), whose attribution to a
# watchdog timeout that file explicitly retracts.
FAULT_Q, FAULT_N, FAULT_M = 3, 24, 4096


class Path:
    """One retained GPU permanent path and the kernels its launch dispatches."""

    def __init__(self, q, name, kernels, block_lanes, shared_bytes, geometry_cite):
        self.q = q
        self.name = name
        self.kernels = kernels
        self.block_lanes = block_lanes
        self.shared_bytes = shared_bytes
        self.geometry_cite = geometry_cite


class Kernel:
    """One device kernel: display name, profiler substring, receipt log entry."""

    def __init__(self, display, profiler, mangled, log, staged_items=None,
                 whole_body_stride=False):
        self.display = display
        self.profiler = profiler
        self.mangled = mangled
        self.log = log
        # Items the strided global-to-shared staging loop covers, as a function
        # of n; ``None`` for a kernel with no staging loop.
        self.staged_items = staged_items
        # True when the strided loop is the kernel's whole body, so its lane
        # share is the kernel's lane share.
        self.whole_body_stride = whole_body_stride


SHIPPED_LANES = 1  # dim3 block(1, 1, 1); only thread 0 works.
PROTOTYPE_LANES = 32  # active_lanes_for_order(n) = 32 for n >= 5.

BIPEDAL3 = Kernel(
    "permanent_bipedal3_kernel",
    "permanent_bipedal3_kernel(",
    "_Z25permanent_bipedal3_kernelPKhiiPy",
    "permanent_bipedal3.hip.resource.log",
)
BIPEDAL5 = Kernel(
    "permanent_bipedal5_kernel",
    "permanent_bipedal5_kernel(",
    "_Z25permanent_bipedal5_kernelPKhiiPy",
    "permanent_bipedal5.hip.resource.log",
)
BIPEDAL7 = Kernel(
    "permanent_bipedal7_kernel",
    "permanent_bipedal7_kernel(",
    "_Z25permanent_bipedal7_kernelPKhiiPy",
    "permanent_bipedal7.hip.resource.log",
)

PATHS = [
    Path(3, "gpu_hip", [BIPEDAL3], SHIPPED_LANES, lambda n: 0,
         "permanent_bipedal3.hip:334-335, :350"),
    Path(3, "wave-gf3", [Kernel(
        "wave_gf3_kernel<kHalving>",
        "wave_gf3_kernel<((anonymous namespace)::FoldKind)0>",
        "_ZN12_GLOBAL__N_115wave_gf3_kernelILNS_8FoldKindE0EEEvPKhiiPj",
        "wave_gf3_equivalence.hip.resource.log",
        staged_items=lambda n: n)],
        PROTOTYPE_LANES, lambda n: 16 * n, "wave_gf3_equivalence.hip:597-606"),
    Path(3, "fold-gf3", [Kernel(
        "wave_gf3_kernel<kZeroMaskSignPopcount>",
        "wave_gf3_kernel<((anonymous namespace)::FoldKind)1>",
        "_ZN12_GLOBAL__N_115wave_gf3_kernelILNS_8FoldKindE1EEEvPKhiiPj",
        "wave_gf3_equivalence.hip.resource.log",
        staged_items=lambda n: n)],
        PROTOTYPE_LANES, lambda n: 16 * n, "wave_gf3_equivalence.hip:597-606"),
    Path(5, "gpu_hip", [BIPEDAL5], SHIPPED_LANES, lambda n: 0,
         "permanent_bipedal5.hip:253-254, :269"),
    Path(5, "f5-byte-control", [Kernel(
        "f5_byte_control_kernel",
        "f5_byte_control_kernel(",
        "_ZN12_GLOBAL__N_122f5_byte_control_kernelEPKhiiPj",
        "f5_wave_equivalence.hip.resource.log",
        staged_items=lambda n: n * n)],
        PROTOTYPE_LANES, lambda n: n * n, "f5_wave_equivalence.hip:727-737"),
    Path(5, "f5-three-plane", [Kernel(
        "f5_three_plane_kernel",
        "f5_three_plane_kernel(",
        "_ZN12_GLOBAL__N_121f5_three_plane_kernelEPKhiiPj",
        "f5_wave_equivalence.hip.resource.log",
        staged_items=lambda n: n)],
        PROTOTYPE_LANES, lambda n: 24 * n, "f5_wave_equivalence.hip:727-737"),
    Path(7, "gpu_hip", [BIPEDAL7], SHIPPED_LANES, lambda n: 0,
         "permanent_bipedal7.hip:350-351, :366"),
    Path(7, "f7-lookup-table-control", [Kernel(
        "wave_gf7_lookup_table_kernel<1>",
        "wave_gf7_lookup_table_kernel<1u>",
        "_ZN12_GLOBAL__N_128wave_gf7_lookup_table_kernelILj1EEEvPKhiiPj",
        "wave_gf7_equivalence.hip.resource.log",
        staged_items=lambda n: n),
        Kernel(
        "wave_gf7_lookup_table_kernel<2>",
        "wave_gf7_lookup_table_kernel<2u>",
        "_ZN12_GLOBAL__N_128wave_gf7_lookup_table_kernelILj2EEEvPKhiiPj",
        "wave_gf7_equivalence.hip.resource.log",
        staged_items=lambda n: n)],
        PROTOTYPE_LANES, lambda n: 8 * n * ((n + 15) // 16),
        "wave_gf7_equivalence.hip:715-724"),
    Path(7, "f7-three-plane-permanent", [Kernel(
        "prepare_three_plane_columns",
        "prepare_three_plane_columns(",
        "_ZN12_GLOBAL__N_127prepare_three_plane_columnsEPKhiiPm",
        "wave_gf7_equivalence.hip.resource.log",
        staged_items=lambda n: n, whole_body_stride=True),
        Kernel(
        "wave_gf7_three_plane_kernel",
        "wave_gf7_three_plane_kernel(",
        "_ZN12_GLOBAL__N_127wave_gf7_three_plane_kernelEPKmiiPj",
        "wave_gf7_equivalence.hip.resource.log",
        staged_items=lambda n: 3 * n)],
        PROTOTYPE_LANES, lambda n: 24 * n, "wave_gf7_equivalence.hip:676-684"),
]
FIELD_STUDY = {3: "047b62ed", 5: "91605d4d", 7: "6c7fcb38"}
EXECUTION_ID = {3: 3002, 5: 5002, 7: 7002}

# The per-lane register budget a committed design states for each kernel, in
# 32-bit units, as a function of n. Every one is a source-level lower bound on
# stable lane state that explicitly excludes loop temporaries, spills, and
# compiler allocation, and names the compiler resource report authoritative.
# ``None`` is an absence of a committed budget, checked against the cited
# source range rather than filled with a back-derived figure.
COMMITTED_REGISTER_BUDGET = {
    "permanent_bipedal3_kernel": (None, "permanent_bipedal3.hip:157-168"),
    "wave_gf3_kernel<kHalving>": (lambda n: 9, "wave_gf3_equivalence.hip:10-12"),
    "wave_gf3_kernel<kZeroMaskSignPopcount>":
        (lambda n: 9, "wave_gf3_equivalence.hip:10-12"),
    "permanent_bipedal5_kernel": (None, "permanent_bipedal5.hip:83-93"),
    "f5_byte_control_kernel":
        (lambda n: (n + 20) / 4, "f5_wave_equivalence.hip:9-12"),
    "f5_three_plane_kernel": (lambda n: 11, "f5_wave_equivalence.hip:13-15"),
    "permanent_bipedal7_kernel": (None, "permanent_bipedal7.hip:83-94"),
    "wave_gf7_lookup_table_kernel<1>":
        (lambda n: 7, "wave_gf7_equivalence.hip:14-17"),
    "wave_gf7_lookup_table_kernel<2>":
        (lambda n: 9, "wave_gf7_equivalence.hip:14-17"),
    "prepare_three_plane_columns": (None, "wave_gf7_equivalence.hip:132-155"),
    "wave_gf7_three_plane_kernel":
        (lambda n: 11, "wave_gf7_equivalence.hip:8-12"),
}
# The compiler's own occupancy model, reproduced from VGPRs alone across every
# entry of the resource receipt: a 1024-register per-SIMD budget allocated in
# units of 16, capped at the architectural wave-slot ceiling.
VGPR_BUDGET_PER_SIMD = 1024
VGPR_ALLOCATION_UNIT = 16
WAVE_SLOT_CEILING = 16


# Every HIP translation unit a retained path's kernels are compiled from, and
# the pattern that finds a per-lane register model in one. The sweep below turns
# the receipt's "no committed budget" rows into a structural check over the
# whole device source tree rather than a hand-maintained list.
HIP_SOURCE_ROOTS = (
    pathlib.Path("crates/gf2-kernels-hip/hip"),
    pathlib.Path("dev/research/permanent_wave_gpu/hip"),
)
REGISTER_MODEL_PATTERNS = (
    "32-bit-register",
    "32-bit units",
    "32-bit register units",
    "stable lane state",
    "Register/shared-memory",
)


def repo_root() -> pathlib.Path:
    return STUDIES.parent.parent


def occupancy_from_vgprs(vgprs: float) -> int:
    units = max(1, -(-int(-(-vgprs // 1)) // VGPR_ALLOCATION_UNIT))
    return min(WAVE_SLOT_CEILING,
               VGPR_BUDGET_PER_SIMD // (VGPR_ALLOCATION_UNIT * units))

# This study's own profiled cells, one pass each for trace and counters.
PROFILED_CELLS = [
    ("q3-n12-gpuhip", 3, 12, "gpu_hip"),
    ("q3-n12-wavegf3", 3, 12, "wave-gf3"),
    ("q3-n12-foldgf3", 3, 12, "fold-gf3"),
    ("q3-n20-gpuhip", 3, 20, "gpu_hip"),
    ("q3-n20-wavegf3", 3, 20, "wave-gf3"),
    ("q3-n20-foldgf3", 3, 20, "fold-gf3"),
    ("q3-n28-gpuhip", 3, 28, "gpu_hip"),
    ("q3-n28-wavegf3", 3, 28, "wave-gf3"),
    ("q3-n28-foldgf3", 3, 28, "fold-gf3"),
    ("q5-n12-gpuhip", 5, 12, "gpu_hip"),
    ("q5-n12-bytectl", 5, 12, "f5-byte-control"),
    ("q5-n12-threeplane", 5, 12, "f5-three-plane"),
    ("q5-n20-gpuhip", 5, 20, "gpu_hip"),
    ("q5-n20-bytectl", 5, 20, "f5-byte-control"),
    ("q5-n20-threeplane", 5, 20, "f5-three-plane"),
    ("q5-n24-bytectl", 5, 24, "f5-byte-control"),
    ("q5-n24-threeplane", 5, 24, "f5-three-plane"),
]
# The runtime's own copy kernel, present in every pass; not a permanent path.
RUNTIME_COPY_KERNEL = "__amd_rocclr_copyBuffer"


# ---------------------------------------------------------------------------
# readers


def read_grid(path: pathlib.Path) -> list[dict[str, str]]:
    """Read a campaign grid CSV, which has two unquoted free-text columns."""
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
            i for i, value in enumerate(fields)
            if value.startswith(GRID_SEED_ROOT_PREFIX)
        )
        head = fields[:note_at]
        note = ",".join(fields[note_at: anchor - (seed_at - note_at - 1)])
        middle = fields[anchor - (seed_at - note_at - 1): anchor + tail_columns]
        trailing = ",".join(fields[anchor + tail_columns:])
        values = head + [note] + middle + [trailing]
        assert len(values) == len(header), line
        rows.append(dict(zip(header, values)))
    return rows


def read_simple(path: pathlib.Path) -> list[dict[str, str]]:
    """Read a CSV whose only free-text column, if any, is the last one."""
    with path.open() as handle:
        body = [line for line in handle if not line.startswith("#")]
    header = body[0].rstrip("\n").split(",")
    rows = []
    for line in body[1:]:
        fields = line.rstrip("\n").split(",")
        if len(fields) > len(header):
            fields = fields[: len(header) - 1] + [",".join(fields[len(header) - 1:])]
        rows.append(dict(zip(header, fields)))
    return rows


def read_resource_log(path: pathlib.Path) -> list[dict[str, str]]:
    """Parse one `-Rpass-analysis=kernel-resource-usage` stderr capture."""
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


def number(row: dict[str, str], key: str) -> float | None:
    value = row.get(key, "")
    if value in ("", "NaN"):
        return None
    return float(value)


def grid_rows(q: int) -> list[dict[str, str]]:
    return read_grid(STUDIES / FIELD_STUDY[q] / f"{RUN}-q{q}-grid.csv")


def device_cells(q: int) -> list[dict[str, str]]:
    """Every grid row of a retained device path for one field, in (n, M) order."""
    names = {path.name for path in PATHS if path.q == q}
    rows = [row for row in grid_rows(q) if row["backend"] in names]
    return sorted(rows, key=lambda row: (int(row["n"]), row["backend"],
                                         int(row["batch_size"])))


def path_of(q: int, name: str) -> Path:
    return next(path for path in PATHS if path.q == q and path.name == name)


# ---------------------------------------------------------------------------
# the profiled evidence run


def counter_rows(pass_name: str) -> tuple[str, list[dict[str, str]]]:
    """This run's per-dispatch counter CSV for one pass, with its file name."""
    files = sorted((PROFILED / pass_name).glob("*_counter_collection.csv"))
    assert len(files) == 1, f"{pass_name}: expected one counter CSV, found {files}"
    with files[0].open() as handle:
        return files[0].name, list(csv.DictReader(handle))


def kernel_stats(pass_name: str) -> tuple[str, list[dict[str, str]]]:
    """rocprofv3's own per-kernel aggregation for one traced pass."""
    files = sorted((PROFILED / pass_name).glob("*_kernel_stats.csv"))
    assert len(files) == 1, f"{pass_name}: expected one kernel_stats CSV, found {files}"
    with files[0].open() as handle:
        return files[0].name, list(csv.DictReader(handle))


def agent_report() -> dict[str, str]:
    """The GPU agent row every pass of this run reports, checked identical."""
    seen: dict[str, str] | None = None
    count = 0
    for path in sorted(PROFILED.glob("*/*_agent_info.csv")):
        with path.open() as handle:
            for row in csv.DictReader(handle):
                if row.get("Agent_Type") != "GPU":
                    continue
                fields = {key: row[key] for key in AGENT_FIELDS}
                if seen is None:
                    seen = fields
                assert seen == fields, f"{path}: agent report differs: {fields}"
                count += 1
    assert seen is not None and count >= len(PROFILED_CELLS), count
    return seen


def dispatch_waves(row: dict[str, str], wave_front: int) -> int:
    """Waves this dispatch launches, from its own Grid_Size and Workgroup_Size."""
    grid = int(row["Grid_Size"])
    block = int(row["Workgroup_Size"])
    assert grid % block == 0, row
    waves_per_block = -(-block // wave_front)
    return (grid // block) * waves_per_block


def occupancy_readings(pass_name: str, kernel: Kernel, agent: dict[str, str]):
    """Per-dispatch occupancy readings for one kernel, split by admissibility.

    A reading is admissible under the policy committed in
    `dev/studies/6c7fcb38/receipts.md` section 13: `MeanOccupancyPerCU` inside
    (0, Max_Waves_Per_Cu] and resident waves no more than the dispatch's own
    launch geometry supplies, with one-percent rounding slack.
    """
    _, rows = counter_rows(pass_name)
    cu_count = int(agent["Cu_Count"])
    max_per_cu = int(agent["Max_Waves_Per_Cu"])
    wave_front = int(agent["Wave_Front_Size"])
    by_dispatch: dict[str, dict[str, object]] = {}
    for row in rows:
        if kernel.profiler not in row["Kernel_Name"]:
            continue
        entry = by_dispatch.setdefault(row["Dispatch_Id"], {
            "waves_launched": dispatch_waves(row, wave_front),
            "vgpr": int(row["VGPR_Count"]),
            "sgpr": int(row["SGPR_Count"]),
            "lds": int(row["LDS_Block_Size"]),
            "scratch": int(row["Scratch_Size"]),
            "block": int(row["Workgroup_Size"]),
        })
        entry[row["Counter_Name"]] = float(row["Counter_Value"])
    admissible, rejected = [], []
    for dispatch_id, entry in sorted(by_dispatch.items(), key=lambda kv: int(kv[0])):
        mean = entry.get("MeanOccupancyPerCU")
        if mean is None:
            continue
        resident = mean * cu_count
        launched = entry["waves_launched"]
        entry["dispatch_id"] = int(dispatch_id)
        entry["resident_waves"] = resident
        entry["residency"] = resident / launched
        entry["achieved_percent"] = 100.0 * mean / max_per_cu
        if 0.0 < mean <= max_per_cu and resident <= launched * (1.0 + LAUNCH_TOLERANCE):
            admissible.append(entry)
        else:
            rejected.append(entry)
    return admissible, rejected


def pass_exit_codes() -> dict[str, int]:
    codes = {}
    for line in (PROFILED / "run.log").read_text().splitlines():
        if line.startswith("EXIT["):
            name = line[len("EXIT["):line.index("]")]
            codes[name] = int(line.split(":", 1)[1].strip().split()[0])
    return codes


# ---------------------------------------------------------------------------
# sections


def heading(text: str) -> None:
    print(f"\n{'=' * 78}\n{text}\n{'=' * 78}")


def section_1_provenance() -> None:
    heading("1. Provenance and reproduction (REQ-09)")
    codes = pass_exit_codes()
    expected = {f"{kind}-{label}" for label, *_ in PROFILED_CELLS
                for kind in ("trace", "pmc")}
    assert set(codes) == expected, (
        f"pass inventory mismatch: missing {sorted(expected - set(codes))}, "
        f"unexpected {sorted(set(codes) - expected)}"
    )
    assert all(code == 0 for code in codes.values()), codes
    print(f"profiled run           : {PROFILED.name}")
    print(f"passes recorded        : {len(codes)} "
          f"({len(PROFILED_CELLS)} cells x trace + counters), all exit 0")
    directories = {entry.name for entry in PROFILED.iterdir() if entry.is_dir()}
    assert directories == expected, sorted(directories ^ expected)
    print(f"pass directories       : {len(directories)}, matching the log exactly")
    for line in (PROFILED / "run.log").read_text().splitlines():
        if line.startswith(("binary_sha256:", "git_revision:", "stamp:")):
            print(f"{line}")
    agent = agent_report()
    print("agent report (identical in every pass directory):")
    for key in AGENT_FIELDS:
        print(f"  {key:<20} {agent[key]}")
    slots = int(agent["Cu_Count"]) * int(agent["Max_Waves_Per_Cu"])
    print(f"  device wave slots    {slots} "
          f"(Cu_Count x Max_Waves_Per_Cu)")


def section_2_paths() -> None:
    heading("2. Retained GPU permanent paths and the kernels they dispatch")
    print(f"{'q':>2} {'path':<26} {'kernel':<38} {'lanes/block':>11} "
          f"{'dynamic shared B/block':>24}")
    for path in PATHS:
        for kernel in path.kernels:
            shared = ("0 (exact)" if path.shared_bytes(20) == 0
                      else f"{path.shared_bytes(12)}..{path.shared_bytes(28)}")
            if kernel.display == "prepare_three_plane_columns":
                shared = "0 (exact)"
            print(f"{path.q:>2} {path.name:<26} {kernel.display:<38} "
                  f"{path.block_lanes:>11} {shared:>24}")
    print()
    print("Per-order dynamic shared bytes per block, from the launch formulas:")
    header = "  " + "".join(f"{n:>10}" for n in ORDERS)
    print(f"  {'path':<26}{header}")
    for path in PATHS:
        if path.shared_bytes(20) == 0:
            continue
        row = "".join(f"{path.shared_bytes(n):>10}" for n in ORDERS)
        print(f"  {path.name:<26}  {row}")


def lane_step_budget(kernel: Kernel, n: int, lanes: int):
    """Useful and total lane-steps per block, per phase, from the source bounds.

    The Gray walk splits 2**n sequential indices across `lanes` lanes with
    `balanced_interval`; the staging loop is strided; the reduction runs
    `lanes` shuffle steps in which each lane sources its own partial once and
    lane zero performs every accumulate.
    """
    if lanes == 1:
        # One working thread in a one-thread block: the whole kernel occupies a
        # single lane of the wavefront.
        return None
    phases = []
    staged = kernel.staged_items(n) if kernel.staged_items else 0
    if staged:
        iterations = -(-staged // lanes)
        phases.append(("staging", staged, lanes * iterations))
    if kernel.whole_body_stride:
        return phases
    phases.append(("prefix", lanes * n, lanes * n))
    phases.append(("Gray walk", 2 ** n, 2 ** n))
    phases.append(("reduction (strict)", 2 * lanes, lanes * lanes))
    return phases


def section_3_wave_utilization() -> None:
    heading("3. Wave utilization of each execution mapping (REQ-01)")
    agent = agent_report()
    wave_front = int(agent["Wave_Front_Size"])
    print(f"Wavefront width from the agent report: {wave_front} lanes.\n")
    print("Launch-geometry share, exact by construction:")
    print(f"  {'q':>2} {'path':<26} {'lanes/block':>11} {'share of a wavefront':>21}")
    for path in PATHS:
        share = 100.0 * path.block_lanes / wave_front
        print(f"  {path.q:>2} {path.name:<26} {path.block_lanes:>11} {share:>20.4f}%")
    print()
    print("The source's lanes-per-block is checked against the profiled")
    print("dispatches' own Workgroup_Size, so the share above is not taken on")
    print("trust from the launch site alone:")
    checked = 0
    for label, q, name in ((label, q, name) for label, q, _, name in PROFILED_CELLS):
        path = path_of(q, name)
        _, rows = counter_rows(f"pmc-{label}")
        for kernel in path.kernels:
            widths = {int(row["Workgroup_Size"]) for row in rows
                      if kernel.profiler in row["Kernel_Name"]}
            if not widths:
                continue
            assert widths == {path.block_lanes}, (label, kernel.display, widths)
            checked += len(widths)
    print(f"  {checked} (pass, kernel) pairs agree with the source geometry")
    print()
    print("Tail-interval idleness of the lane-owns-interval partition: "
          "balanced_interval")
    print("splits 2**n indices across 32 lanes, so the remainder 2**n mod 32 is the")
    print("number of lanes that carry one extra index.")
    for n in ORDERS:
        remainder = (2 ** n) % PROTOTYPE_LANES
        assert remainder == 0, (n, remainder)
        print(f"  n = {n:>2}: 2**{n} mod {PROTOTYPE_LANES} = {remainder}, "
              f"every lane owns {2 ** n // PROTOTYPE_LANES} indices")
    print()
    print("Lane-step decomposition per block and per matrix, from the source loop")
    print("bounds. 'strict' counts a reduction lane-step as useful only where the")
    print("lane sources its own partial or lane zero accumulates; 'generous' counts")
    print("every lane's shuffle participation as useful.")
    for path in PATHS:
        if path.block_lanes == 1:
            continue
        for kernel in path.kernels:
            print(f"\n  {kernel.display} ({path.name}, q={path.q})")
            print(f"    {'n':>3} {'walk steps/lane':>16} {'strict share':>13} "
                  f"{'generous share':>15}")
            for n in ORDERS:
                phases = lane_step_budget(kernel, n, path.block_lanes)
                useful = sum(u for _, u, _ in phases)
                total = sum(t for _, _, t in phases)
                extra = 0
                for name, u, t in phases:
                    if name.startswith("reduction"):
                        extra = t - u
                walk = 2 ** n // path.block_lanes if not kernel.whole_body_stride else 0
                strict = 100.0 * useful / total
                generous = 100.0 * (useful + extra) / total
                print(f"    {n:>3} {walk:>16} {strict:>12.4f}% {generous:>14.4f}%")


def section_4_resources() -> None:
    heading("4. Compiler resources and the occupancy-limiting resource (REQ-02)")
    logs = {}
    for path in PATHS:
        for kernel in path.kernels:
            logs.setdefault(kernel.log, read_resource_log(RESOURCES / kernel.log))
    print(f"{'q':>2} {'kernel':<38} {'SGPR':>5} {'VGPR':>5} {'scratch':>8} "
          f"{'spill':>6} {'sLDS':>5} {'occ':>4} {'limiter':<24} log")
    entries = {}
    for path in PATHS:
        for kernel in path.kernels:
            entry = next(e for e in logs[kernel.log] if e["kernel"] == kernel.mangled)
            entries[kernel.display] = entry
            vgpr = int(entry["VGPRs"])
            occ = int(entry["Occupancy [waves/SIMD]"])
            spill = f"{entry['SGPRs Spill']}+{entry['VGPRs Spill']}"
            limiter = ("per-lane vector registers" if occ < 16
                       else "wave-slot ceiling")
            print(f"{path.q:>2} {kernel.display:<38} {entry['TotalSGPRs']:>5} "
                  f"{vgpr:>5} {entry['ScratchSize [bytes/lane]']:>8} {spill:>6} "
                  f"{entry['LDS Size [bytes/block]']:>5} {occ:>4} {limiter:<24} "
                  f"{entry['log']}")
    print()
    print("The occupancy field is a function of VGPRs alone across every entry of")
    print("the receipt directory: min(16, floor(1024 / (16 * ceil(VGPRs / 16)))).")
    checked = 0
    for log_path in sorted(RESOURCES.glob("*.resource.log")):
        for entry in read_resource_log(log_path):
            vgpr = int(entry["VGPRs"])
            predicted = min(16, 1024 // (16 * max(1, -(-vgpr // 16))))
            reported = int(entry["Occupancy [waves/SIMD]"])
            assert predicted == reported, (entry["kernel"], vgpr, predicted, reported)
            checked += 1
    print(f"  checked {checked} kernel entries, all agree")
    print()
    print("Per-block shared memory against the device's own LDS capacity. One")
    print("block is one wave, so the wave-slot cap fixes how many blocks a CU may")
    print("hold, and the LDS a full CU would then request is the product.")
    agent = agent_report()
    lds_bytes = int(agent["Lds_Size_In_Kb"]) * 1024
    blocks_per_cu = int(agent["Max_Waves_Per_Cu"])
    worst = 0
    for path in PATHS:
        if path.block_lanes == 1:
            continue
        for n in ORDERS:
            worst = max(worst, path.shared_bytes(n))
    request = worst * blocks_per_cu
    print(f"  largest per-block request over every retained path and order: "
          f"{worst} B")
    print(f"  {blocks_per_cu} blocks x {worst} B = {request} B against "
          f"{lds_bytes} B of LDS per CU")
    assert request < lds_bytes, (request, lds_bytes)
    print(f"  LDS-bound blocks per CU: {lds_bytes // worst}, above the "
          f"{blocks_per_cu}-wave slot cap, so shared memory never binds")
    print()
    print("Device-source sweep for a committed per-lane register model. Every HIP")
    print("translation unit under the two device-source roots is scanned; the ones")
    print("that state a model are the only ones the budget table can cite.")
    carriers = set()
    scanned = 0
    for root in HIP_SOURCE_ROOTS:
        for source in sorted((repo_root() / root).rglob("*")):
            if source.suffix not in (".hip", ".h"):
                continue
            scanned += 1
            text = source.read_text()
            if any(pattern in text for pattern in REGISTER_MODEL_PATTERNS):
                carriers.add(source.name)
    expected = {"wave_gf3_equivalence.hip", "f5_wave_equivalence.hip",
                "wave_gf7_equivalence.hip"}
    assert carriers == expected, sorted(carriers ^ expected)
    print(f"  scanned {scanned} translation units and headers; "
          f"{len(carriers)} state a model:")
    for name in sorted(carriers):
        print(f"    {name}")
    cited = {
        cite.split(":")[0]
        for budget, cite in COMMITTED_REGISTER_BUDGET.values() if budget is not None
    }
    assert cited == carriers, sorted(cited ^ carriers)
    print("  every budget the table cites comes from one of them, and every kernel")
    print("  in a unit that states none carries an explicit absence")
    print()
    print("What the committed per-lane register budget would imply, against what")
    print("the compiler allocated. The budget column is a source-level lower bound")
    print("on stable lane state; the implied occupancy applies the compiler's own")
    print("model to it.")
    print(f"  {'kernel':<40} {'budget (32-bit units)':>21} "
          f"{'implied occ':>11} {'measured VGPRs':>14} {'measured occ':>12} "
          f"budget source")
    for path in PATHS:
        for kernel in path.kernels:
            budget, cite = COMMITTED_REGISTER_BUDGET[kernel.display]
            entry = entries[kernel.display]
            vgpr = int(entry["VGPRs"])
            occ = int(entry["Occupancy [waves/SIMD]"])
            if budget is None:
                shown, implied = "none committed", "-"
            else:
                values = sorted({budget(n) for n in ORDERS})
                shown = (f"{values[0]:g}" if len(values) == 1
                         else f"{values[0]:g}-{values[-1]:g}")
                occupancies = {occupancy_from_vgprs(budget(n)) for n in ORDERS}
                assert occupancies == {WAVE_SLOT_CEILING}, (kernel.display,
                                                            occupancies)
                implied = f"{max(occupancies)}"
            print(f"  {kernel.display:<40} {shown:>21} {implied:>11} "
                  f"{vgpr:>14} {occ:>12} {cite}")
    print()
    print("Every committed budget implies the wave-slot ceiling under the")
    print("compiler's own model, so no stated budget is the occupancy limiter of")
    print("any retained path; where occupancy falls below the ceiling it is the")
    print("compiler's realised allocation that takes it there.")
    for path in PATHS:
        for kernel in path.kernels:
            budget, _ = COMMITTED_REGISTER_BUDGET[kernel.display]
            if budget is None:
                continue
            entry = entries[kernel.display]
            vgpr = int(entry["VGPRs"])
            ratios = sorted({vgpr / budget(n) for n in ORDERS})
            shown = (f"{ratios[0]:.2f}x" if len(ratios) == 1
                     else f"{ratios[0]:.2f}-{ratios[-1]:.2f}x")
            print(f"  {kernel.display:<40} allocated {vgpr:>4} VGPRs, "
                  f"{shown} its stated lower bound")
    return entries


def section_5_occupancy(entries) -> None:
    heading("5. Achieved occupancy from profiler counters (REQ-03)")
    agent = agent_report()
    slots = int(agent["Cu_Count"]) * int(agent["Max_Waves_Per_Cu"])
    print(f"Achieved occupancy is the reading against the device's {slots} wave "
          f"slots.\n")
    print(f"{'pass':<20} {'kernel':<40} {'width':>7} {'disp':>5} "
          f"{'mean w/CU':>10} {'min-max':>21} {'residency':>10} {'achieved':>9}")
    admissible_kernels = set()
    rejected_report = []
    for label, q, n, name in PROFILED_CELLS:
        path = path_of(q, name)
        for kernel in path.kernels:
            groups: dict[int, list] = {}
            admissible, rejected = occupancy_readings(f"pmc-{label}", kernel, agent)
            for entry in admissible:
                groups.setdefault(entry["waves_launched"], []).append(entry)
            for width, group in sorted(groups.items()):
                means = [e["MeanOccupancyPerCU"] for e in group]
                residency = [e["residency"] for e in group]
                mean = sum(means) / len(means)
                achieved = 100.0 * mean / int(agent["Max_Waves_Per_Cu"])
                admissible_kernels.add(kernel.display)
                print(f"pmc-{label:<16} {kernel.display:<40} {width:>7} "
                      f"{len(group):>5} {mean:>10.6f} "
                      f"{min(means):>9.6f}-{max(means):<11.6f} "
                      f"{min(residency):>4.4f}-{max(residency):<5.4f} "
                      f"{achieved:>8.4f}%")
            if rejected:
                rejected_report.append((label, kernel.display, rejected))
    counted = sum(
        len(occupancy_readings(f"pmc-{label}", kernel, agent)[0])
        + len(occupancy_readings(f"pmc-{label}", kernel, agent)[1])
        for label, q, _, name in PROFILED_CELLS
        for kernel in path_of(q, name).kernels
    )
    print()
    print(f"Counted dispatches over all counter passes: {counted}")
    print("Readings rejected by the admissibility policy, recorded rather than "
          "dropped:")
    if not rejected_report:
        print("  none")
    for label, display, rejected in rejected_report:
        for entry in rejected:
            print(f"  pmc-{label:<16} {display:<40} dispatch "
                  f"{entry['dispatch_id']:>3} width {entry['waves_launched']:>6} "
                  f"MeanOccupancyPerCU {entry['MeanOccupancyPerCU']:.6f} "
                  f"-> {entry['residency']:.1f}x its launch")
    print()
    print("Per-dispatch resource fields the runtime reports, beside the compiler's:")
    print(f"  {'kernel':<40} {'VGPR_Count':>10} {'compiler VGPRs':>14} "
          f"{'Scratch_Size':>12} {'compiler scratch':>16} {'LDS_Block_Size':>14}")
    seen = set()
    for label, q, n, name in PROFILED_CELLS:
        path = path_of(q, name)
        for kernel in path.kernels:
            if kernel.display in seen:
                continue
            admissible, rejected = occupancy_readings(f"pmc-{label}", kernel, agent)
            readings = admissible + rejected
            if not readings:
                continue
            seen.add(kernel.display)
            entry = readings[0]
            compiler = entries[kernel.display]
            vgpr = int(compiler["VGPRs"])
            assert entry["vgpr"] == -(-vgpr // 8) * 8, (kernel.display, entry, vgpr)
            print(f"  {kernel.display:<40} {entry['vgpr']:>10} {vgpr:>14} "
                  f"{entry['scratch']:>12} "
                  f"{compiler['ScratchSize [bytes/lane]']:>16} "
                  f"{entry['lds']:>14}")
    print()
    print("Kernels with at least one admissible reading in this run: "
          f"{len(admissible_kernels)}")
    for display in sorted(admissible_kernels):
        print(f"  {display}")
    return admissible_kernels


def section_6_limiter(entries, admissible_kernels) -> None:
    heading("6. What limits occupancy across the study's kernels (REQ-04)")
    agent = agent_report()
    slots = int(agent["Cu_Count"]) * int(agent["Max_Waves_Per_Cu"])
    print("Level 1 — the mapping's stated per-lane register budget and per-block")
    print("shared-memory allocation. Section 4 shows every committed budget")
    print("implies the wave-slot ceiling under the compiler's own model, and that")
    print("the largest per-block request leaves the device's LDS capacity with")
    print("headroom above the wave-slot cap. Neither limits occupancy anywhere.")
    print()
    print("Level 2 — the compiler's realised allocation, which does limit four")
    print("kernels, all by per-lane vector registers:")
    for path in PATHS:
        for kernel in path.kernels:
            entry = entries[kernel.display]
            occ = int(entry["Occupancy [waves/SIMD]"])
            if occ >= WAVE_SLOT_CEILING:
                continue
            print(f"  q={path.q} {kernel.display:<40} {entry['VGPRs']:>4} VGPRs "
                  f"-> {occ} waves/SIMD against the {WAVE_SLOT_CEILING}-wave "
                  f"ceiling")
    print()
    print("Level 3 — achieved occupancy at runtime, where the limiter is the grid")
    print("width rather than any per-thread resource. Highest achieved reading in")
    print("this run against the per-SIMD ceiling's share of the device's slots:")
    # Compared per launch-width group, as the receipt's table reports them: a
    # group's achieved figure is the mean over its dispatches, not the single
    # highest dispatch.
    best = {}
    for label, q, n, name in PROFILED_CELLS:
        path = path_of(q, name)
        for kernel in path.kernels:
            admissible, _ = occupancy_readings(f"pmc-{label}", kernel, agent)
            groups: dict[int, list] = {}
            for entry in admissible:
                groups.setdefault(entry["waves_launched"], []).append(entry)
            for width, group in groups.items():
                achieved = sum(e["achieved_percent"] for e in group) / len(group)
                key = kernel.display
                if key not in best or achieved > best[key][0]:
                    best[key] = (achieved, label, width, len(group),
                                 min(e["residency"] for e in group),
                                 max(e["residency"] for e in group))
    for display, (achieved, label, width, count, lo, hi) in sorted(best.items()):
        entry = entries[display]
        ceiling = 100.0 * int(entry["Occupancy [waves/SIMD]"]) / WAVE_SLOT_CEILING
        print(f"  {display:<40} best {achieved:>8.4f}% of {slots} slots "
              f"(pmc-{label}, width {width}, {count} dispatches, residency "
              f"{lo:.4f}-{hi:.4f}) against a {ceiling:.0f}% per-SIMD ceiling")
    print()
    print("Residency of the waves each dispatch launches, across every admissible")
    print("reading of this run — the quantity that separates 'few waves' from")
    print("'waves that cannot become resident':")
    residencies = []
    for label, q, n, name in PROFILED_CELLS:
        path = path_of(q, name)
        for kernel in path.kernels:
            admissible, _ = occupancy_readings(f"pmc-{label}", kernel, agent)
            residencies.extend(entry["residency"] for entry in admissible)
    assert residencies, "no admissible readings"
    print(f"  {len(residencies)} dispatches, residency "
          f"{min(residencies):.4f}-{max(residencies):.4f}")


def section_7_phases() -> None:
    heading("7. Transfer and launch overhead, separate from kernel time (REQ-05)")
    print(f"{'q':>2} {'n':>3} {'path':<26} {'M':>5} {'outcome':<9} {'reps':>6} "
          f"{'kernel s':>12} {'H2D us/l':>9} {'D2H us/l':>9} {'sub us/l':>9} "
          f"{'host us/l':>12}")
    launch = []
    for q in sorted(FIELD_STUDY):
        for row in device_cells(q):
            reps = number(row, "reps")
            kernel = number(row, "kernel_device_s")
            if not reps or kernel is None:
                print(f"{q:>2} {row['n']:>3} {row['backend']:<26} "
                      f"{row['batch_size']:>5} {row['outcome']:<9} "
                      f"{row['reps']:>6} "
                      f"{'no spans: ' + row['phase_timing_note']:<12}")
                continue
            h2d = number(row, "h2d_device_s") / reps * 1e6
            d2h = number(row, "d2h_device_s") / reps * 1e6
            sub = number(row, "device_submission_to_kernel_s") / reps * 1e6
            host = number(row, "host_submission_s") / reps * 1e6
            launch.append(sub)
            print(f"{q:>2} {row['n']:>3} {row['backend']:<26} "
                  f"{row['batch_size']:>5} {row['outcome']:<9} {int(reps):>6} "
                  f"{kernel:>12.6f} {h2d:>9.2f} {d2h:>9.2f} {sub:>9.4f} "
                  f"{host:>12.1f}")
    print()
    print(f"device_submission_to_kernel_s over all {len(launch)} device cells: "
          f"{min(launch):.4f}-{max(launch):.4f} us per launch")


def section_8_duration() -> None:
    heading("8. Sustained-kernel duration against order and per-launch work "
            "(REQ-06)")
    print(f"{'q':>2} {'n':>3} {'path':<26} {'M':>5} {'M*2^n':>12} "
          f"{'M*n*2^n':>12} {'s/launch':>12} {'ns per M*n*2^n':>16}")
    rows_by_path: dict[tuple[int, str], list] = {}
    for q in sorted(FIELD_STUDY):
        for row in device_cells(q):
            reps = number(row, "reps")
            kernel = number(row, "kernel_device_s")
            if not reps or kernel is None:
                continue
            n = int(row["n"])
            m = int(row["batch_size"])
            span = kernel / reps
            work = m * 2 ** n
            ryser = m * n * 2 ** n
            rows_by_path.setdefault((q, row["backend"]), []).append(
                (n, m, work, ryser, span))
            print(f"{q:>2} {n:>3} {row['backend']:<26} {m:>5} {work:>12.4e} "
                  f"{ryser:>12.4e} {span:>12.6f} {1e9 * span / ryser:>16.6f}")
    print()
    print("Batch scaling at fixed order, where a path has two batch sizes: the")
    print("span step against the batch step.")
    for (q, name), rows in sorted(rows_by_path.items()):
        by_order: dict[int, list] = {}
        for n, m, _, _, span in rows:
            by_order.setdefault(n, []).append((m, span))
        for n, cells in sorted(by_order.items()):
            if len(cells) < 2:
                continue
            cells.sort()
            (m0, s0), (m1, s1) = cells[0], cells[-1]
            print(f"  q={q} {name:<26} n={n:>2}: M {m0:>5} -> {m1:>5} "
                  f"({m1 / m0:.2f}x) spans {s0:.6f} -> {s1:.6f} s "
                  f"({s1 / s0:.4f}x)")
    print()
    print("Span at a fixed per-launch work, across paths: the work axis alone")
    print("does not determine the span.")
    target = 268435456  # 2**28
    matches = []
    for (q, name), rows in sorted(rows_by_path.items()):
        for n, m, work, _, span in rows:
            if work == target:
                matches.append((q, name, n, m, span))
    for q, name, n, m, span in sorted(matches, key=lambda item: item[4]):
        print(f"  q={q} {name:<26} n={n:>2} M={m:>5} work {target:.4e} "
              f"span {span:>12.6f} s")
    if matches:
        spans = [item[4] for item in matches]
        print(f"  spread at one work value: {max(spans) / min(spans):.1f}x")


def section_9_watchdog() -> None:
    heading("9. Watchdog-safe per-launch work bound and its reconciliation "
            "(REQ-07, REQ-08)")
    print("Largest cleanly completed launch per field, from this study's own")
    print("committed grids. A censored cell that executed retains its device")
    print("spans, and its launches completed; the censoring is the stopping rule.")
    print()
    summary = {}
    for q in sorted(FIELD_STUDY):
        best_work = best_span = None
        measured_work = None
        for row in device_cells(q):
            reps = number(row, "reps")
            kernel = number(row, "kernel_device_s")
            if not reps or kernel is None:
                continue
            n = int(row["n"])
            m = int(row["batch_size"])
            span = kernel / reps
            work = m * 2 ** n
            cell = (n, row["backend"], m, row["outcome"], work, span, int(reps))
            if best_work is None or work > best_work[4]:
                best_work = cell
            if best_span is None or span > best_span[5]:
                best_span = cell
            if row["outcome"] == "measured" and (
                    measured_work is None or work > measured_work[4]):
                measured_work = cell
        summary[q] = (best_work, best_span, measured_work)
        budget = ARCHIVED_WORK_BUDGET[q]
        for label, cell in (("largest work", best_work),
                            ("largest work, measured cells only", measured_work),
                            ("longest span", best_span)):
            n, backend, m, outcome, work, span, reps = cell
            print(f"  q={q} {label:<34} n={n:>2} {backend:<26} M={m:>5} "
                  f"{outcome:<9} {reps:>4} launches  work {work:.6e}  "
                  f"span {span:>11.6f} s")
        print(f"       archived q={q} budget {budget:.3e}: largest observed work "
              f"is {best_work[4] / budget:.4f} of it; "
              f"largest measured-cell work is {measured_work[4] / budget:.4f} of it")
        print(f"       archived span boundary {ARCHIVED_SPAN_BOUNDARY_S:.0f} s: "
              f"longest observed span is {best_span[5] / ARCHIVED_SPAN_BOUNDARY_S:.4f} "
              f"of it")
        print()
    print("Cells whose per-launch work exceeds their field's archived budget:")
    over = 0
    for q in sorted(FIELD_STUDY):
        for row in device_cells(q):
            reps = number(row, "reps")
            kernel = number(row, "kernel_device_s")
            if not reps or kernel is None:
                continue
            n = int(row["n"])
            m = int(row["batch_size"])
            work = m * 2 ** n
            if work <= ARCHIVED_WORK_BUDGET[q]:
                continue
            over += 1
            print(f"  q={q} n={n:>2} {row['backend']:<26} M={m:>5} "
                  f"{row['outcome']:<9} work {work:.4e} = "
                  f"{work / ARCHIVED_WORK_BUDGET[q]:>8.2f}x budget, span "
                  f"{kernel / reps:>11.6f} s")
    print(f"  {over} cells over budget, none of which faulted")
    print()
    print("The one recorded device fault, against this study's own cells:")
    fault_work = FAULT_M * 2 ** FAULT_N
    fault_ryser = FAULT_M * FAULT_N * 2 ** FAULT_N
    print(f"  fault: q={FAULT_Q} n={FAULT_N} gpu_hip M={FAULT_M}, "
          f"work {fault_work:.6e} (M*2^n), {fault_ryser:.6e} (M*n*2^n)")
    for row in device_cells(FAULT_Q):
        reps = number(row, "reps")
        kernel = number(row, "kernel_device_s")
        if not reps or kernel is None or row["backend"] != "gpu_hip":
            continue
        n = int(row["n"])
        m = int(row["batch_size"])
        work = m * 2 ** n
        if work < fault_work:
            continue
        ryser = m * n * 2 ** n
        print(f"  completed: q={FAULT_Q} n={n:>2} gpu_hip M={m:>5} "
              f"{row['outcome']:<9} work {work:.6e} "
              f"({work / fault_work:.2f}x the fault), {ryser:.6e} "
              f"({ryser / fault_ryser:.2f}x in M*n*2^n), span "
              f"{kernel / reps:.6f} s")
    print()
    print("ESTIMATE, not a bound: the faulted configuration is unmeasured, so any")
    print("span figure for it comes from scaling measured neighbours under an")
    print("assumption those measurements do not settle. Route 1 scales this")
    print("study's device spans at that order linearly in M; the step it actually")
    print("measures there is sublinear, so linear scaling is a model and not a")
    print("ceiling.")
    same = [(int(row["batch_size"]),
             number(row, "kernel_device_s") / number(row, "reps"))
            for row in device_cells(FAULT_Q)
            if row["backend"] == "gpu_hip" and int(row["n"]) == FAULT_N
            and number(row, "reps")]
    for m, span in sorted(same):
        print(f"  measured M={m:>5} span {span:.6f} s -> M={FAULT_M} "
              f"estimated {span * FAULT_M / m:.4f} s under linear scaling")
    if len(same) >= 2:
        (m0, s0), (m1, s1) = sorted(same)[0], sorted(same)[-1]
        print(f"  measured step at that order: M {m0} -> {m1} ({m1 / m0:.2f}x) "
              f"costs {s1 / s0:.4f}x, i.e. sublinear")
    rows = [row for row in read_simple(SUSTAINED)
            if int(row["q"]) == FAULT_Q and int(row["n"]) == FAULT_N
            and row["backend"] == "gpu_hip"]
    print()
    print("Route 2, also an ESTIMATE: the committed sustained receipt's wall time")
    print("per shard at the same cell, a composite host+device quantity, scaled by")
    print("the batch ratio. Its own two points step super-linearly, so extending")
    print("them assumes a scaling they do not settle.")
    wall_per_shard = []
    for row in rows:
        m = int(row["batch_size"])
        shards = int(row["shards"])
        wall = float(row["wall_s"])
        wall_per_shard.append((m, wall / shards))
        print(f"  M={m:>5} {shards:>4} shards over {wall:.3f} s -> "
              f"{wall / shards:.4f} s per launch -> M={FAULT_M} estimated "
              f"{wall / shards * FAULT_M / m:.4f} s")
    if len(wall_per_shard) >= 2:
        (m0, w0), (m1, w1) = sorted(wall_per_shard)[0], sorted(wall_per_shard)[-1]
        print(f"  measured step on this route: M {m0} -> {m1} ({m1 / m0:.2f}x) "
              f"costs {w1 / w0:.4f}x, i.e. super-linear")
    print()
    print("Neither route bounds the unmeasured configuration, and the receipt does")
    print("not use either as a bound: the reconciliation rests on the measured")
    print("completions above.")


def section_10_prose_checks() -> None:
    heading("10. The receipt's stated counts, checked against the data")
    receipt = (STUDY / "receipt.md").read_text()
    agent = agent_report()
    codes = pass_exit_codes()
    dispatches = 0
    groups = 0
    width_one = 0
    residencies = []
    for label, q, _, name in PROFILED_CELLS:
        for kernel in path_of(q, name).kernels:
            admissible, rejected = occupancy_readings(f"pmc-{label}", kernel, agent)
            dispatches += len(admissible) + len(rejected)
            widths: dict[int, list] = {}
            for entry in admissible:
                widths.setdefault(entry["waves_launched"], []).append(entry)
            groups += len(widths)
            width_one += 1 if 1 in widths else 0
            residencies.extend(entry["residency"] for entry in admissible)
    device_cell_count = sum(
        1 for q in FIELD_STUDY for row in device_cells(q)
        if number(row, "reps") and number(row, "kernel_device_s") is not None
    )
    kernels = sum(len(path.kernels) for path in PATHS)
    words = {9: "Nine", 11: "eleven", 34: "Thirty-four", 17: "seventeen",
             35: "35", 23: "23", 135: "135"}
    stated = [
        (f"{words[len(PATHS)]} paths are retained", len(PATHS)),
        (f"dispatching {words[kernels]} kernels", kernels),
        (f"{words[len(codes)]} passes", len(codes)),
        (f"{words[len(PROFILED_CELLS)]} cells", len(PROFILED_CELLS)),
        (f"over the {device_cell_count} device cells", device_cell_count),
        (f"all {dispatches} counted dispatches", dispatches),
        (f"{groups} launch-width", groups),
        ("the seventeen\nwidth-1 groups", width_one),
    ]
    for phrase, value in stated:
        assert phrase in receipt, f"receipt does not state {phrase!r} (value {value})"
        print(f"  receipt states {phrase!r:<48} -> derived {value}")
    span = f"{min(residencies):.4f} to {max(residencies):.4f}"
    assert span in receipt, f"receipt does not state residency range {span}"
    print(f"  receipt states residency range {span!r:<26} -> derived from "
          f"{len(residencies)} dispatches")
    assert width_one == 17, width_one


def section_11_f7_citation() -> None:
    heading("11. The committed F_7 occupancy evidence this receipt cites")
    counters = F7_PROFILED / "counters-aggregate.csv"
    assert counters.exists(), counters
    provenance = (F7_PROFILED / "provenance.txt").read_text()
    assert "amendment (2026-08-16)" in provenance, "amendment missing"
    print(f"  {counters.relative_to(STUDIES.parent.parent)}")
    print(f"  {(F7_PROFILED / 'provenance.txt').relative_to(STUDIES.parent.parent)}"
          " (including its 2026-08-16 amendment)")
    pmc2 = sorted(F7_PROFILED.glob("pmc2-*/*_counter_collection.csv"))
    print(f"  {len(pmc2)} committed round-2 per-dispatch counter CSVs")
    assert len(pmc2) == 9, pmc2


def main() -> int:
    section_1_provenance()
    section_2_paths()
    section_3_wave_utilization()
    entries = section_4_resources()
    admissible_kernels = section_5_occupancy(entries)
    section_6_limiter(entries, admissible_kernels)
    section_7_phases()
    section_8_duration()
    section_9_watchdog()
    section_10_prose_checks()
    section_11_f7_citation()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
