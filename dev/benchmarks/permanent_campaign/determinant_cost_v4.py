#!/usr/bin/env python3
"""Run, validate, and render the all-cell determinant-cost cohort."""

from __future__ import annotations

import argparse
import csv
import dataclasses
import hashlib
import json
import math
import os
import re
import socket
import subprocess
import sys
from pathlib import Path
from typing import Iterable, Sequence

PROCESS_COUNT = 5
TIMED_REPETITIONS = 5
FIXTURE_COUNT = 32
TARGET_MS = 250
WORKER_COUNT = 1
WARMUP_POLICY = "doubling-probe-to-min-20ms-target-250ms-max-2^32"
BACKEND = "fieldmatrix_det_ple"
MEASURED = "measured"
OUTCOMES = {MEASURED, "process_failed", "signal_censored", "harness_censored"}
BENCHMARK_WRAPPER_PATH = "dev/scripts/ccx1-bench-flock.sh"
BENCHMARK_WRAPPER = f"{BENCHMARK_WRAPPER_PATH}:ccx1"
SCHEMA_VERSION = "determinant-companion-v4"
SEED_ROOT = 0xEC22_205E_0000_0001
PREREGISTRATION_PATH = (
    "dev/benchmarks/permanent_campaign/determinant-cost-preregistration-v4.md"
)
RECEIPT_PATH = "dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv"
REPORT_PATH = "dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.md"
RUNNER_PATH = "dev/benchmarks/permanent_campaign/determinant_cost_v4.py"

FIELDNAMES = [
    "schema_version",
    "process_index",
    "q",
    "n",
    "outcome",
    "process_exit_code",
    "backend",
    "seed_root",
    "cell_seed",
    "fixture_count",
    "fixture_starts",
    "warmup_policy",
    "warmup_calls",
    "warmup_elapsed_ns",
    "target_ms",
    "timed_repetitions",
    "calls_per_repetition",
    "repetition_elapsed_ns",
    "sample_count",
    "elapsed_determinant_ns",
    "ns_per_matrix",
    "started_unix_ns",
    "finished_unix_ns",
    "git_revision",
    "source_dirty",
    "rustc",
    "cargo_profile",
    "binary_sha256",
    "hostname",
    "cpu_model",
    "kernel",
    "governor",
    "boost",
    "affinity",
    "process_count_per_cell",
    "worker_count",
    "benchmark_wrapper",
    "cohort_invocation",
    "invocation",
    "stdout_sha256",
    "stderr_sha256",
]

PROVENANCE_FIELDS = [
    "git_revision",
    "source_dirty",
    "rustc",
    "cargo_profile",
    "binary_sha256",
    "hostname",
    "cpu_model",
    "kernel",
    "governor",
    "boost",
    "affinity",
    "process_count_per_cell",
    "worker_count",
    "benchmark_wrapper",
    "cohort_invocation",
]

TIMING_FIELDS = [
    "warmup_calls",
    "warmup_elapsed_ns",
    "calls_per_repetition",
    "repetition_elapsed_ns",
    "sample_count",
    "elapsed_determinant_ns",
    "ns_per_matrix",
    "started_unix_ns",
    "finished_unix_ns",
]

RELEVANT_SOURCE_PATHS = [
    "Cargo.toml",
    "Cargo.lock",
    "crates/gf2-core",
    "crates/gf2-algebra",
    BENCHMARK_WRAPPER_PATH,
    RUNNER_PATH,
    PREREGISTRATION_PATH,
]


class ReceiptError(ValueError):
    """The receipt violates its frozen schema or arithmetic contract."""


@dataclasses.dataclass(frozen=True)
class CellSummary:
    q: int
    n: int
    measured_processes: int
    nonmeasured_processes: int
    elapsed_ns: int
    sample_count: int
    ns_per_matrix: float | None
    fixed_sample_count: int
    projected_seconds: float | None


def campaign_cells() -> list[tuple[int, int]]:
    return [
        *((3, n) for n in range(4, 29)),
        *((5, n) for n in range(4, 25)),
        *((7, n) for n in range(4, 21)),
    ]


def fixed_sample_count(q: int, n: int) -> int:
    if q == 3 and 4 <= n <= 28:
        return 20_000_000 if n <= 20 else 222_223
    if q == 5 and 4 <= n <= 24:
        return 16_000_000 if n <= 16 else 160_000
    if q == 7 and 4 <= n <= 20:
        return 12_244_898 if n <= 16 else 122_449
    raise ReceiptError(f"cell ({q},{n}) is outside the frozen campaign universe")


def cell_seed(q: int, n: int) -> int:
    return SEED_ROOT ^ (q << 48) ^ (n << 32)


def fixture_starts(process_index: int) -> list[int]:
    return [TIMED_REPETITIONS * (process_index - 1) + offset for offset in range(5)]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def command_output(args: Sequence[str]) -> str:
    completed = subprocess.run(args, check=True, capture_output=True, text=True)
    return completed.stdout.strip()


def relevant_source_status(
    git_command: Sequence[str] = ("git",),
) -> str:
    """Return tracked modifications in the measurement source closure."""
    return command_output(
        [
            *git_command,
            "status",
            "--porcelain",
            "--untracked-files=no",
            "--",
            *RELEVANT_SOURCE_PATHS,
        ]
    )


def read_text(path: Path, default: str = "unknown") -> str:
    try:
        return path.read_text(encoding="utf-8").strip()
    except OSError:
        return default


def cpu_model() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("model name\t: "):
                return line.removeprefix("model name\t: ")
    except OSError:
        pass
    return "unknown"


def format_affinity(cpus: Iterable[int]) -> str:
    ordered = sorted(cpus)
    if not ordered:
        return "unknown"
    ranges: list[str] = []
    start = previous = ordered[0]
    for cpu in ordered[1:]:
        if cpu == previous + 1:
            previous = cpu
            continue
        ranges.append(str(start) if start == previous else f"{start}-{previous}")
        start = previous = cpu
    ranges.append(str(start) if start == previous else f"{start}-{previous}")
    return ",".join(ranges)


def runtime_provenance(
    binary: Path,
    cohort_invocation: Sequence[str],
) -> dict[str, str]:
    source_status = relevant_source_status()
    boost_value = read_text(Path("/sys/devices/system/cpu/cpufreq/boost"))
    boost = {"0": "disabled", "1": "enabled"}.get(boost_value, boost_value)
    try:
        affinity = format_affinity(os.sched_getaffinity(0))
    except AttributeError:
        affinity = "unknown"
    return {
        "git_revision": command_output(["git", "rev-parse", "HEAD"]),
        "source_dirty": str(bool(source_status)).lower(),
        "rustc": command_output(["rustc", "+1.95.0", "--version"]),
        "cargo_profile": "release-bench",
        "binary_sha256": sha256_file(binary),
        "hostname": socket.gethostname(),
        "cpu_model": cpu_model(),
        "kernel": command_output(["uname", "-srvmo"]),
        "governor": read_text(
            Path("/sys/devices/system/cpu/cpu6/cpufreq/scaling_governor")
        ),
        "boost": boost,
        "affinity": affinity,
        "process_count_per_cell": str(PROCESS_COUNT),
        "worker_count": str(WORKER_COUNT),
        "benchmark_wrapper": BENCHMARK_WRAPPER,
        "cohort_invocation": json.dumps(list(cohort_invocation), separators=(",", ":")),
    }


def provenance_from_row(row: dict[str, str]) -> dict[str, str]:
    return {field: row[field] for field in PROVENANCE_FIELDS}


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        if reader.fieldnames != FIELDNAMES:
            raise ReceiptError(
                f"schema columns differ: expected {FIELDNAMES}, got {reader.fieldnames}"
            )
        return [dict(row) for row in reader]


def read_scratch_csv(path: Path) -> list[dict[str, str]]:
    if not path.exists():
        return []
    with path.open(newline="", encoding="utf-8") as handle:
        return [dict(row) for row in csv.DictReader(handle)]


def write_csv_exclusive(path: Path, rows: Sequence[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=FIELDNAMES, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def empty_outcome_row(
    q: int,
    n: int,
    process_index: int,
    outcome: str,
    exit_code: int,
) -> dict[str, str]:
    row = {field: "" for field in FIELDNAMES}
    row.update(
        {
            "schema_version": SCHEMA_VERSION,
            "process_index": str(process_index),
            "q": str(q),
            "n": str(n),
            "outcome": outcome,
            "process_exit_code": str(exit_code),
            "backend": BACKEND,
            "seed_root": f"0x{SEED_ROOT:016x}",
            "cell_seed": f"0x{cell_seed(q, n):016x}",
            "fixture_count": str(FIXTURE_COUNT),
            "fixture_starts": ";".join(str(value) for value in fixture_starts(process_index)),
            "warmup_policy": WARMUP_POLICY,
            "target_ms": str(TARGET_MS),
            "timed_repetitions": str(TIMED_REPETITIONS),
        }
    )
    return row


def merge_process_rows(
    *,
    process_index: int,
    exit_code: int,
    scratch_rows: Sequence[dict[str, str]],
    provenance: dict[str, str],
    invocation: Sequence[str],
    stdout: bytes,
    stderr: bytes,
) -> list[dict[str, str]]:
    by_cell: dict[tuple[int, int], dict[str, str]] = {}
    for source in scratch_rows:
        try:
            address = (int(source["q"]), int(source["n"]))
        except (KeyError, ValueError) as error:
            raise ReceiptError(f"malformed scratch address: {error}") from error
        if address not in set(campaign_cells()):
            raise ReceiptError(f"scratch row has extra cell {address}")
        if address in by_cell:
            raise ReceiptError(f"scratch row has duplicate cell {address}")
        if int(source["process_index"]) != process_index:
            raise ReceiptError(f"scratch process index differs at cell {address}")
        by_cell[address] = dict(source)

    if exit_code < 0:
        missing_outcome = "signal_censored"
    elif exit_code > 0:
        missing_outcome = "process_failed"
    else:
        missing_outcome = "harness_censored"

    invocation_json = json.dumps(list(invocation), separators=(",", ":"))
    stdout_hash = sha256_bytes(stdout)
    stderr_hash = sha256_bytes(stderr)
    merged: list[dict[str, str]] = []
    for q, n in campaign_cells():
        if (q, n) in by_cell:
            row = {field: by_cell[(q, n)].get(field, "") for field in FIELDNAMES}
            row["outcome"] = MEASURED
        else:
            row = empty_outcome_row(q, n, process_index, missing_outcome, exit_code)
        row["process_exit_code"] = str(exit_code)
        row.update(provenance)
        row["invocation"] = invocation_json
        row["stdout_sha256"] = stdout_hash
        row["stderr_sha256"] = stderr_hash
        merged.append(row)
    return merged


def parse_int(row: dict[str, str], field: str, address: tuple[int, int, int]) -> int:
    try:
        return int(row[field])
    except (KeyError, ValueError) as error:
        raise ReceiptError(f"{address}: invalid integer {field}") from error


def validate_measured_row(
    row: dict[str, str],
    address: tuple[int, int, int],
) -> None:
    q, n, process_index = address
    starts = [int(value) for value in row["fixture_starts"].split(";")]
    expected_starts = fixture_starts(process_index)
    if starts != expected_starts:
        raise ReceiptError(f"{address}: fixture starts differ from {expected_starts}")
    repetitions = [int(value) for value in row["repetition_elapsed_ns"].split(";")]
    if len(repetitions) != TIMED_REPETITIONS or any(value <= 0 for value in repetitions):
        raise ReceiptError(f"{address}: repetition elapsed values are invalid")
    calls = parse_int(row, "calls_per_repetition", address)
    sample_count = parse_int(row, "sample_count", address)
    elapsed = parse_int(row, "elapsed_determinant_ns", address)
    if calls <= 0 or sample_count != calls * TIMED_REPETITIONS:
        raise ReceiptError(f"{address}: sample count differs from calls times repetitions")
    if elapsed != sum(repetitions):
        raise ReceiptError(f"{address}: elapsed sum differs from raw repetitions")
    recorded_ns = float(row["ns_per_matrix"])
    expected_ns = elapsed / sample_count
    if not math.isfinite(recorded_ns) or not math.isclose(
        recorded_ns, expected_ns, rel_tol=5e-10, abs_tol=5e-7
    ):
        raise ReceiptError(f"{address}: ns_per_matrix differs from pooled arithmetic")
    if parse_int(row, "warmup_calls", address) <= 0:
        raise ReceiptError(f"{address}: warmup calls must be positive")
    if parse_int(row, "warmup_elapsed_ns", address) <= 0:
        raise ReceiptError(f"{address}: warmup elapsed time must be positive")
    if parse_int(row, "started_unix_ns", address) > parse_int(
        row, "finished_unix_ns", address
    ):
        raise ReceiptError(f"{address}: timestamps run backwards")
    if row["cell_seed"] != f"0x{cell_seed(q, n):016x}":
        raise ReceiptError(f"{address}: cell seed differs from frozen addressing")


def validate_rows(rows: Sequence[dict[str, str]]) -> list[CellSummary]:
    expected_addresses = {
        (q, n, process_index)
        for q, n in campaign_cells()
        for process_index in range(1, PROCESS_COUNT + 1)
    }
    by_address: dict[tuple[int, int, int], dict[str, str]] = {}
    reference_provenance: dict[str, str] | None = None
    for row in rows:
        missing_fields = set(FIELDNAMES) - set(row)
        if missing_fields:
            raise ReceiptError(f"row omits fields: {sorted(missing_fields)}")
        address = (
            parse_int(row, "q", (0, 0, 0)),
            parse_int(row, "n", (0, 0, 0)),
            parse_int(row, "process_index", (0, 0, 0)),
        )
        if address in by_address:
            raise ReceiptError(f"duplicate address {address}")
        if address not in expected_addresses:
            raise ReceiptError(f"extra address {address}")
        by_address[address] = row
        if row["schema_version"] != SCHEMA_VERSION:
            raise ReceiptError(f"{address}: schema version differs")
        if row["outcome"] not in OUTCOMES:
            raise ReceiptError(f"{address}: unknown outcome {row['outcome']!r}")
        q, n, process_index = address
        if row["backend"] != BACKEND:
            raise ReceiptError(f"{address}: backend differs")
        if row["seed_root"] != f"0x{SEED_ROOT:016x}":
            raise ReceiptError(f"{address}: seed root differs")
        if row["cell_seed"] != f"0x{cell_seed(q, n):016x}":
            raise ReceiptError(f"{address}: cell seed differs")
        if row["fixture_count"] != str(FIXTURE_COUNT):
            raise ReceiptError(f"{address}: fixture count differs")
        if row["fixture_starts"] != ";".join(
            str(value) for value in fixture_starts(process_index)
        ):
            raise ReceiptError(f"{address}: fixture starts differ")
        if row["warmup_policy"] != WARMUP_POLICY:
            raise ReceiptError(f"{address}: warmup policy differs")
        if row["target_ms"] != str(TARGET_MS):
            raise ReceiptError(f"{address}: target differs")
        if row["timed_repetitions"] != str(TIMED_REPETITIONS):
            raise ReceiptError(f"{address}: repetition count differs")
        if row["source_dirty"] != "false":
            raise ReceiptError(f"{address}: relevant measurement source is dirty")
        if re.fullmatch(r"[0-9a-f]{40}", row["git_revision"]) is None:
            raise ReceiptError(f"{address}: source revision is not a full Git object id")
        if re.fullmatch(r"[0-9a-f]{64}", row["binary_sha256"]) is None:
            raise ReceiptError(f"{address}: binary SHA-256 is malformed")
        if row["cargo_profile"] != "release-bench":
            raise ReceiptError(f"{address}: Cargo profile differs")
        if not row["rustc"].startswith("rustc 1.95.0 "):
            raise ReceiptError(f"{address}: Rust compiler differs from the MSRV")
        if row["affinity"] != "6-11":
            raise ReceiptError(f"{address}: benchmark affinity differs")
        for digest_field in ("stdout_sha256", "stderr_sha256"):
            if re.fullmatch(r"[0-9a-f]{64}", row[digest_field]) is None:
                raise ReceiptError(f"{address}: {digest_field} is malformed")
        if row["process_count_per_cell"] != str(PROCESS_COUNT):
            raise ReceiptError(f"{address}: process count differs")
        if row["worker_count"] != str(WORKER_COUNT):
            raise ReceiptError(f"{address}: worker count differs")
        if row["benchmark_wrapper"] != BENCHMARK_WRAPPER:
            raise ReceiptError(f"{address}: benchmark wrapper differs")
        try:
            invocation = json.loads(row["invocation"])
            cohort_invocation = json.loads(row["cohort_invocation"])
        except json.JSONDecodeError as error:
            raise ReceiptError(f"{address}: invocation is not JSON argv") from error
        if not isinstance(invocation, list) or str(process_index) not in invocation:
            raise ReceiptError(f"{address}: process invocation omits its index")
        if not isinstance(cohort_invocation, list):
            raise ReceiptError(f"{address}: cohort invocation is not an argv")
        if reference_provenance is None:
            reference_provenance = provenance_from_row(row)
        elif provenance_from_row(row) != reference_provenance:
            raise ReceiptError(f"{address}: cohort provenance differs")
        if row["outcome"] == MEASURED:
            validate_measured_row(row, address)
        elif any(row[field] for field in TIMING_FIELDS):
            raise ReceiptError(f"{address}: non-measured timing fields are populated")

    missing = expected_addresses - set(by_address)
    if missing:
        raise ReceiptError(f"missing address {sorted(missing)[0]}")
    if len(by_address) != 315:
        raise ReceiptError(f"expected 315 rows, found {len(by_address)}")

    summaries: list[CellSummary] = []
    for q, n in campaign_cells():
        cell_rows = [by_address[(q, n, index)] for index in range(1, PROCESS_COUNT + 1)]
        measured = [row for row in cell_rows if row["outcome"] == MEASURED]
        elapsed = sum(int(row["elapsed_determinant_ns"]) for row in measured)
        samples = sum(int(row["sample_count"]) for row in measured)
        pooled = elapsed / samples if samples else None
        fixed_n = fixed_sample_count(q, n)
        summaries.append(
            CellSummary(
                q=q,
                n=n,
                measured_processes=len(measured),
                nonmeasured_processes=PROCESS_COUNT - len(measured),
                elapsed_ns=elapsed,
                sample_count=samples,
                ns_per_matrix=pooled,
                fixed_sample_count=fixed_n,
                projected_seconds=(pooled * fixed_n / 1e9) if pooled is not None else None,
            )
        )
    return summaries


def markdown_escape(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def render_report(
    rows: Sequence[dict[str, str]],
    receipt_sha256: str,
) -> str:
    summaries = validate_rows(rows)
    first = rows[0]
    nonmeasured = [row for row in rows if row["outcome"] != MEASURED]
    starts = [int(row["started_unix_ns"]) for row in rows if row["started_unix_ns"]]
    finishes = [int(row["finished_unix_ns"]) for row in rows if row["finished_unix_ns"]]
    max_projection = max(
        summary.projected_seconds or 0.0 for summary in summaries
    )
    all_fit = all(
        summary.projected_seconds is not None and summary.projected_seconds <= 43_200
        for summary in summaries
    )
    verdict = (
        f"All 63 cells have five measured process outcomes. The largest projected "
        f"fixed-$N$ determinant addition is {max_projection:.6f} s, and every cell "
        f"fits the twelve-hour operational ceiling."
        if not nonmeasured and all_fit
        else f"The receipt retains {len(nonmeasured)} failed or censored process outcomes; "
        "the per-cell table reports only directly measured pooled totals."
    )
    lines = [
        "# Determinant companion cost for every campaign cell",
        "",
        "This receipt directly measures `FieldMatrix::det` at every cell in the",
        "permanent zero-fraction campaign. It reports determinant marginal cost",
        "only; it does not rank or time permanent backends.",
        "",
        "## Verdict",
        "",
        verdict,
        "",
        "## Protocol and provenance",
        "",
        "| Item | Recorded value |",
        "|---|---|",
        f"| Preregistration | `{PREREGISTRATION_PATH}` |",
        f"| Machine-readable receipt | `{RECEIPT_PATH}` |",
        f"| Receipt SHA-256 | `{receipt_sha256}` |",
        f"| Schema | `{SCHEMA_VERSION}` |",
        f"| Source revision | `{first['git_revision']}` |",
        f"| Relevant measurement source dirty | `{first['source_dirty']}` |",
        f"| Benchmark executable SHA-256 | `{first['binary_sha256']}` |",
        f"| Toolchain | `{markdown_escape(first['rustc'])}`; `{first['cargo_profile']}` |",
        f"| Host | `{markdown_escape(first['hostname'])}`; {markdown_escape(first['cpu_model'])} |",
        f"| Kernel | {markdown_escape(first['kernel'])} |",
        f"| Power policy | governor `{markdown_escape(first['governor'])}`; boost `{markdown_escape(first['boost'])}` |",
        f"| Isolation | `{first['benchmark_wrapper']}`; affinity `{first['affinity']}`; one serial worker |",
        f"| Process contract | {PROCESS_COUNT} fresh processes per cell; {TIMED_REPETITIONS} timed repetitions per process; {TARGET_MS} ms target |",
        f"| Recorded window | Unix ns `{min(starts) if starts else 'none'}` through `{max(finishes) if finishes else 'none'}` |",
        f"| Exact cohort argv | `{markdown_escape(first['cohort_invocation'])}` |",
        "| Exact process argv | Recorded per process in the CSV `invocation` column |",
        "",
        "The cohort ran with the command identity recorded above under the exclusive",
        "benchmark wrapper. Fixture generation and adaptive calibration are outside",
        "the timed windows. The preregistration fixes the address formula and the",
        "calibration stopping rule.",
        "",
        "## Pooled marginal cost and fixed-$N$ projection",
        "",
        "For measured processes $M_{q,n}$, each cell uses pooled raw totals:",
        "",
        "$$",
        "t_{q,n}=\\frac{\\sum_{e\\in M_{q,n}}T_e}{\\sum_{e\\in M_{q,n}}C_e}.",
        "$$",
        "",
        "No representative-order interpolation or mean of process means enters the",
        "table. The projected addition is $N_{q,n}t_{q,n}$ using the protocol's",
        "fixed sample count.",
        "",
        "| $q$ | $n$ | measured / planned | determinant ($\\mu$s/matrix) | fixed $N$ | projected addition (s) | projected addition (h) | fits 12 h |",
        "|---:|---:|---:|---:|---:|---:|---:|:---:|",
    ]
    for summary in summaries:
        if summary.ns_per_matrix is None or summary.projected_seconds is None:
            microseconds = projected_seconds = projected_hours = "not measured"
            fits = "no measurement"
        else:
            microseconds = f"{summary.ns_per_matrix / 1_000:.9f}"
            projected_seconds = f"{summary.projected_seconds:.6f}"
            projected_hours = f"{summary.projected_seconds / 3_600:.9f}"
            fits = "yes" if summary.projected_seconds <= 43_200 else "**no**"
        lines.append(
            f"| {summary.q} | {summary.n} | {summary.measured_processes} / {PROCESS_COUNT} | "
            f"{microseconds} | {summary.fixed_sample_count:,} | {projected_seconds} | "
            f"{projected_hours} | {fits} |"
        )
    lines.extend(["", "## Preserved failures and contradictions", ""])
    if not nonmeasured:
        lines.extend(
            [
                "No failed or censored process outcome was observed in the frozen cohort.",
                "All 315 preregistered process-cell outcomes are measured rows; none was",
                "discarded, replaced, or extended.",
            ]
        )
    else:
        lines.extend(
            [
                "Every non-measured outcome remains in the CSV and is listed here. No",
                "replacement or imputation enters the pooled values.",
                "",
                "| $q$ | $n$ | process | outcome | exit status | stderr SHA-256 |",
                "|---:|---:|---:|---|---:|---|",
            ]
        )
        for row in nonmeasured:
            lines.append(
                f"| {row['q']} | {row['n']} | {row['process_index']} | "
                f"`{row['outcome']}` | `{row['process_exit_code']}` | "
                f"`{row['stderr_sha256']}` |"
            )
    validation_argv_lines = [
        f"python3 {RUNNER_PATH} validate \\",
        f"  --receipt {RECEIPT_PATH} \\",
        f"  --report {REPORT_PATH}",
    ]
    lines.extend(
        [
            "",
            "The powersave governor and enabled boost state are material limitations.",
            "This receipt supports the determinant budget projection; it does not support",
            "close performance comparisons or a fixed-frequency claim.",
            "",
            "## Validation",
            "",
            "The committed validator proves the exact 63-cell and 315-row address set,",
            "uniform cohort provenance, raw-to-pooled arithmetic, fixture-address",
            "uniqueness, non-measured-row emptiness, and byte-for-byte agreement between",
            "this rendered document and the machine receipt.",
            "",
            "```sh",
            *validation_argv_lines,
            "```",
            "",
        ]
    )
    return "\n".join(lines)


def validate_rendered_report(
    rows: Sequence[dict[str, str]],
    receipt_sha256: str,
    rendered: str,
) -> None:
    expected = render_report(rows, receipt_sha256)
    if rendered != expected:
        raise ReceiptError("rendered receipt mismatch")


def require_artifact_path(path: Path, expected: str, kind: str) -> None:
    if path.resolve() != (Path.cwd() / expected).resolve():
        raise ReceiptError(f"{kind} path must be {expected}")


def run_cohort(args: argparse.Namespace) -> int:
    require_artifact_path(args.output, RECEIPT_PATH, "receipt")
    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ReceiptError(f"benchmark binary is not executable: {binary}")
    if args.output.exists():
        raise ReceiptError(f"refusing to overwrite receipt: {args.output}")
    if args.scratch_dir.exists():
        raise ReceiptError(f"refusing to reuse scratch directory: {args.scratch_dir}")
    args.scratch_dir.mkdir(parents=True)
    cohort_invocation = [sys.executable, str(Path(__file__).resolve()), *sys.argv[1:]]
    provenance = runtime_provenance(binary, cohort_invocation)
    if provenance["source_dirty"] != "false":
        raise ReceiptError("relevant measurement source is dirty; timing did not begin")
    if provenance["affinity"] != "6-11":
        raise ReceiptError(
            f"benchmark wrapper admission failed: expected affinity 6-11, got {provenance['affinity']}"
        )

    rows: list[dict[str, str]] = []
    any_nonzero = False
    for process_index in range(1, PROCESS_COUNT + 1):
        scratch = args.scratch_dir / f"process-{process_index}.csv"
        invocation = [
            str(binary),
            "--execution",
            str(process_index),
            "--output",
            str(scratch.resolve()),
        ]
        completed = subprocess.run(invocation, capture_output=True)
        any_nonzero |= completed.returncode != 0
        scratch_rows = read_scratch_csv(scratch)
        rows.extend(
            merge_process_rows(
                process_index=process_index,
                exit_code=completed.returncode,
                scratch_rows=scratch_rows,
                provenance=provenance,
                invocation=invocation,
                stdout=completed.stdout,
                stderr=completed.stderr,
            )
        )

    validate_rows(rows)
    write_csv_exclusive(args.output, rows)
    print(f"wrote {len(rows)} outcomes to {args.output}")
    if any_nonzero or any(row["outcome"] != MEASURED for row in rows):
        print("cohort retained non-measured outcomes", file=sys.stderr)
        return 7
    return 0


def render_command(args: argparse.Namespace) -> int:
    require_artifact_path(args.receipt, RECEIPT_PATH, "receipt")
    require_artifact_path(args.report, REPORT_PATH, "report")
    rows = read_csv(args.receipt)
    receipt_hash = sha256_file(args.receipt)
    rendered = render_report(rows, receipt_hash)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x", encoding="utf-8") as handle:
        handle.write(rendered)
    print(f"wrote {args.report}")
    return 0


def validate_command(args: argparse.Namespace) -> int:
    require_artifact_path(args.receipt, RECEIPT_PATH, "receipt")
    require_artifact_path(args.report, REPORT_PATH, "report")
    rows = read_csv(args.receipt)
    receipt_hash = sha256_file(args.receipt)
    rendered = args.report.read_text(encoding="utf-8")
    validate_rendered_report(rows, receipt_hash, rendered)
    summaries = validate_rows(rows)
    print(
        "PASS: "
        f"{len(summaries)} cells, {len(rows)} process outcomes, "
        f"receipt sha256 {receipt_hash}"
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    run = subparsers.add_parser("run", help="run the frozen five-process cohort")
    run.add_argument("--binary", type=Path, required=True)
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--scratch-dir", type=Path, required=True)
    run.set_defaults(function=run_cohort)

    render = subparsers.add_parser("render", help="render the validated receipt")
    render.add_argument("--receipt", type=Path, required=True)
    render.add_argument("--report", type=Path, required=True)
    render.set_defaults(function=render_command)

    validate = subparsers.add_parser("validate", help="validate CSV and rendered report")
    validate.add_argument("--receipt", type=Path, required=True)
    validate.add_argument("--report", type=Path, required=True)
    validate.set_defaults(function=validate_command)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    try:
        return args.function(args)
    except (OSError, ReceiptError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
