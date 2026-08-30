#!/usr/bin/env python3
"""Derive, render, and validate accelerator launch-cost evidence v1."""

from __future__ import annotations

import argparse
import csv
import dataclasses
import hashlib
import json
import math
import re
import sys
from collections import defaultdict
from decimal import ROUND_CEILING, Decimal, InvalidOperation
from pathlib import Path
from typing import Callable, Hashable, Sequence, TypeVar


ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = (
    "dev/simulation_results/permanent-zero-fraction/"
    "permanent-zero-fraction-20260829/manifest.json"
)
COST_TABLE_PATH = (
    "dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv"
)
RECEIPT_PATH = (
    "dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.md"
)
VALIDATOR_PATH = (
    "dev/benchmarks/permanent_campaign/accelerator_launch_costs_v1.py"
)
LEDGER_PATH = "dev/benchmarks/permanent_campaign/premeasure-v1-ledger.csv"
CANDIDATES_PATH = (
    "dev/benchmarks/permanent_campaign/premeasure-v1-candidates.csv"
)
FRONTIER_PATH = "dev/benchmarks/permanent_campaign/backend-ordering.csv"

SCHEMA_VERSION = "accelerator-launch-cost-receipt-v1"
COST_HEADER = "q,n,per_matrix_us"
ACCELERATOR_BACKEND = "accelerator"
HARNESS_BACKEND = "gpu_hip"
BATCH_SIZE = 1024
PLANNED_PROCESSES = 12

BOUND_INPUTS = (
    (
        MANIFEST_PATH,
        "5caa384d9c87f24562ee6d91c61c44dbc04674512b3dbe63e761ca0b9480ae57",
        "frozen campaign cell and backend map",
    ),
    (
        "dev/benchmarks/permanent_campaign/backend-selection-v1.md",
        "fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe",
        "complete-cohort eligibility and backend-selection receipt",
    ),
    (
        LEDGER_PATH,
        "d1efd9dcfa39b8498db1e04ba0720de2bf96677fc6b820ffadc3f1919848d046",
        "all 1,440 premeasure-v1 terminal process outcomes",
    ),
    (
        CANDIDATES_PATH,
        "272a524185c14515394a24a5e7385e07b7dc5620c0a488a12438668b72b8c6b8",
        "premeasure-v1 structurally valid timing projection",
    ),
    (
        "dev/benchmarks/permanent_campaign/premeasure-v1-receipt.md",
        "46826730ec186963260d55df8b19d53229ffb98b768343611bb4b5a537c21664",
        "premeasure-v1 cohort identity and censor account",
    ),
    (
        "dev/benchmarks/permanent_campaign/premeasure-v1-deviations.md",
        "b5e8401a6523e2c05912fa720fe4b5b74491c28b77a7bf2154d2f5a36b1956eb",
        "native-M accelerator row projection rule",
    ),
    (
        FRONTIER_PATH,
        "57c2fafbb4050d4eacf65837c41bf4c3fe1ab69fd49eac622486bb6282bc8dd2",
        "all 48 frontier execution outcomes and four summaries",
    ),
    (
        "dev/benchmarks/permanent_campaign/backend-ordering.md",
        "7e50381daf157a5f045717c92932e37edd92fe54ca7d1a63ac387c44cfff13ae",
        "frontier protocol, arithmetic, and provenance receipt",
    ),
    (
        "dev/benchmarks/permanent_campaign/backend-selection-v1-rng-addendum.md",
        "bb33bde423edb4918beb92abf134055157ca6950e67dfa45c8e1054791158314",
        "measurement RNG and rebuild provenance",
    ),
)

PREMEASURE_REQUIRED_FIELDS = {
    "schedule_position",
    "plan_q",
    "plan_n",
    "plan_manifest_backend",
    "plan_harness_backend",
    "plan_batch_size",
    "ledger_state",
    "candidate_emitted",
    "identity_valid",
    "provenance_complete",
    "row_valid",
    "session_resolution",
    "scratch_binary_matches_session_actual",
    "recovery_plan_matches_session",
    "session_source_revision",
    "observed_running_binary_sha256",
    "observed_harness_source_sha",
    "observed_deps_source_sha",
    "observed_rustc",
    "observed_cargo",
    "observed_cpu",
    "observed_logical_cpus",
    "observed_gpu",
    "observed_rocm",
    "observed_kernel",
    "scratch_q",
    "scratch_n",
    "scratch_backend",
    "scratch_outcome",
    "scratch_batch_size",
    "scratch_matrices",
    "scratch_total_s",
}

OPTIONAL_SOURCE_PROBES = (
    "observed_harness_source_sha",
    "observed_deps_source_sha",
    "observed_rustc",
    "observed_cargo",
)

FRONTIER_REQUIRED_FIELDS = {
    "record_type",
    "execution_id",
    "schedule_position",
    "config_id",
    "q",
    "n",
    "backend",
    "batch_size",
    "outcome",
    "fresh_process",
    "matrices",
    "total_s",
    "git_sha",
    "harness_source_sha",
    "deps_source_sha",
    "binary_sha256",
    "rust_build_toolchain",
    "cargo_build_toolchain",
    "cpu",
    "logical_cpus",
    "gpu",
    "rocm",
    "kernel",
    "execution_count",
    "pooled_matrices",
    "pooled_total_s",
    "pooled_composite_matrices_per_s",
}


class ReceiptError(ValueError):
    """Committed launch-cost evidence violates its frozen contract."""


@dataclasses.dataclass
class CellCost:
    """One recomputed same-cell launch-sizing value."""

    q: int
    n: int
    cohort: str
    planned_processes: int
    measured_processes: int
    harness_censored: int
    signal_censored: int
    batch_size: int
    total_matrices: int
    total_seconds: Decimal
    pooled_us: Decimal
    rounded_us: int


T = TypeVar("T")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ReceiptError(message)


def require_unique(
    rows: Sequence[T], key: Callable[[T], Hashable], label: str
) -> dict[Hashable, T]:
    by_key: dict[Hashable, T] = {}
    for row in rows:
        identity = key(row)
        if identity in by_key:
            raise ReceiptError(f"duplicate {label} identity {identity!r}")
        by_key[identity] = row
    return by_key


def read_csv(path: Path, required_fields: set[str]) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        fields = set(reader.fieldnames or [])
        missing = required_fields - fields
        if missing:
            raise ReceiptError(f"{path}: missing fields {sorted(missing)}")
        return [dict(row) for row in reader]


def canonical_integer(value: str, field: str, line: int) -> int:
    if re.fullmatch(r"0|[1-9][0-9]*", value) is None:
        raise ReceiptError(f"line {line}: {field} {value!r} is not an integer")
    return int(value)


def parse_cost_table_text(
    text: str, manifest_cells: dict[tuple[int, int], str]
) -> dict[tuple[int, int], int]:
    lines = text.splitlines()
    require(bool(lines), "cost table is empty")
    require(
        lines[0] == COST_HEADER,
        f"cost table header is {lines[0]!r}, expected {COST_HEADER!r}",
    )
    rows: dict[tuple[int, int], int] = {}
    for line, raw in enumerate(lines[1:], start=2):
        require(bool(raw), f"line {line}: blank row")
        fields = raw.split(",")
        require(len(fields) == 3, f"line {line}: expected 3 fields")
        q = canonical_integer(fields[0], "q", line)
        n = canonical_integer(fields[1], "n", line)
        microseconds = canonical_integer(fields[2], "per_matrix_us", line)
        require(microseconds > 0, f"line {line}: per_matrix_us must be positive")
        key = (q, n)
        require(key not in rows, f"line {line}: duplicate cost-table key {key}")
        if key not in manifest_cells:
            raise ReceiptError(f"line {line}: {key} is not a manifest cell")
        if manifest_cells[key] != ACCELERATOR_BACKEND:
            raise ReceiptError(f"line {line}: {key} is processor-backed")
        rows[key] = microseconds

    expected = {
        key for key, backend in manifest_cells.items() if backend == ACCELERATOR_BACKEND
    }
    missing = expected - set(rows)
    if missing:
        raise ReceiptError(f"missing accelerator cost-table key {sorted(missing)[0]}")
    return rows


def pooled_microseconds(
    elapsed_seconds: Sequence[Decimal], matrix_counts: Sequence[int]
) -> tuple[Decimal, int]:
    require(bool(elapsed_seconds), "pool requires at least one elapsed duration")
    require(
        len(elapsed_seconds) == len(matrix_counts),
        "elapsed and matrix input counts differ",
    )
    require(
        all(value.is_finite() and value > 0 for value in elapsed_seconds),
        "elapsed durations must be positive finite decimals",
    )
    require(all(value > 0 for value in matrix_counts), "matrix counts must be positive")
    pooled = sum(elapsed_seconds, Decimal(0)) * Decimal(1_000_000) / sum(
        matrix_counts
    )
    rounded = int(pooled.to_integral_value(rounding=ROUND_CEILING))
    require(rounded > 0, "rounded per-matrix duration must be positive")
    return pooled, rounded


def validate_bindings() -> list[tuple[str, str, str]]:
    bindings: list[tuple[str, str, str]] = []
    for relative, expected, role in BOUND_INPUTS:
        actual = sha256_file(ROOT / relative)
        require(actual == expected, f"bound input digest changed: {relative}")
        require(re.fullmatch(r"[0-9a-f]{64}", actual) is not None, "invalid SHA-256")
        bindings.append((relative, actual, role))
    validator_hash = sha256_file(ROOT / VALIDATOR_PATH)
    bindings.append((VALIDATOR_PATH, validator_hash, "derivation and validator source"))
    return bindings


def read_manifest(path: Path) -> dict[tuple[int, int], str]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReceiptError(f"cannot parse manifest: {error}") from error
    require(isinstance(document, dict), "manifest root is not an object")
    cells = document.get("cells")
    require(isinstance(cells, list), "manifest cells is not an array")
    parsed: list[tuple[tuple[int, int], str]] = []
    for index, cell in enumerate(cells):
        require(isinstance(cell, dict), f"manifest cell {index} is not an object")
        q = cell.get("q")
        n = cell.get("n")
        backend = cell.get("backend")
        require(
            isinstance(q, int) and not isinstance(q, bool),
            f"manifest cell {index} q is not an integer",
        )
        require(
            isinstance(n, int) and not isinstance(n, bool),
            f"manifest cell {index} n is not an integer",
        )
        require(isinstance(backend, str), f"manifest cell {index} backend is not text")
        parsed.append(((q, n), backend))
    require_unique(parsed, lambda item: item[0], "manifest cell")
    return dict(parsed)


def parse_positive_decimal(row: dict[str, str], field: str, address: str) -> Decimal:
    try:
        value = Decimal(row[field])
    except (KeyError, InvalidOperation) as error:
        raise ReceiptError(f"{address}: {field} is not a decimal") from error
    require(value.is_finite() and value > 0, f"{address}: {field} is not positive")
    return value


def parse_positive_int(row: dict[str, str], field: str, address: str) -> int:
    try:
        value = int(row[field])
    except (KeyError, ValueError) as error:
        raise ReceiptError(f"{address}: {field} is not an integer") from error
    require(value > 0, f"{address}: {field} is not positive")
    return value


def one_value(rows: Sequence[dict[str, str]], field: str, label: str) -> str:
    values = {row[field] for row in rows}
    require(len(values) == 1, f"{label} {field} is not uniform")
    return next(iter(values))


def validate_premeasure(
    manifest_cells: dict[tuple[int, int], str],
) -> tuple[dict[tuple[int, int], CellCost], dict[str, str | int]]:
    ledger = read_csv(ROOT / LEDGER_PATH, PREMEASURE_REQUIRED_FIELDS)
    candidates = read_csv(ROOT / CANDIDATES_PATH, PREMEASURE_REQUIRED_FIELDS)
    require(len(ledger) == 1_440, "premeasure ledger must have 1,440 outcomes")
    require(len(candidates) == 1_439, "premeasure projection must have 1,439 rows")
    ledger_by_position = require_unique(
        ledger, lambda row: int(row["schedule_position"]), "premeasure ledger"
    )
    require(
        set(ledger_by_position) == set(range(1_440)),
        "premeasure schedule-position set differs",
    )
    candidate_by_position = require_unique(
        candidates, lambda row: int(row["schedule_position"]), "premeasure candidate"
    )
    require(
        set(candidate_by_position) <= set(ledger_by_position),
        "candidate position is absent from the ledger",
    )
    require(
        all(row["candidate_emitted"] == "true" for row in candidates),
        "candidate projection contains a non-candidate row",
    )

    ledger_signal = [row for row in ledger if row["ledger_state"] == "censored"]
    measured = [row for row in candidates if row["scratch_outcome"] == "measured"]
    harness_censored = [
        row for row in candidates if row["scratch_outcome"] == "censored"
    ]
    require(len(ledger_signal) == 1, "premeasure signal-censored count differs")
    require(len(measured) == 1_396, "premeasure measured count differs")
    require(len(harness_censored) == 43, "premeasure harness-censored count differs")

    common = {
        "source_revision": one_value(ledger, "session_source_revision", "premeasure"),
        "binary_sha256": one_value(
            candidates, "observed_running_binary_sha256", "premeasure"
        ),
        "cpu": one_value(candidates, "observed_cpu", "premeasure"),
        "logical_cpus": one_value(
            candidates, "observed_logical_cpus", "premeasure"
        ),
        "gpu": one_value(candidates, "observed_gpu", "premeasure"),
        "rocm": one_value(candidates, "observed_rocm", "premeasure"),
        "kernel": one_value(candidates, "observed_kernel", "premeasure"),
        "cohort_planned": len(ledger),
        "cohort_measured": len(measured),
        "harness_censored": len(harness_censored),
        "signal_censored": len(ledger_signal),
    }
    require(
        re.fullmatch(r"[0-9a-f]{40}", str(common["source_revision"])) is not None,
        "premeasure source revision is malformed",
    )
    require(
        re.fullmatch(r"[0-9a-f]{64}", str(common["binary_sha256"])) is not None,
        "premeasure binary SHA-256 is malformed",
    )

    ledger_groups: dict[tuple[int, int], list[dict[str, str]]] = defaultdict(list)
    candidate_groups: dict[tuple[int, int], list[dict[str, str]]] = defaultdict(list)
    for row in ledger:
        if row["plan_manifest_backend"] == ACCELERATOR_BACKEND:
            ledger_groups[(int(row["plan_q"]), int(row["plan_n"]))].append(row)
    for row in candidates:
        if row["plan_manifest_backend"] == ACCELERATOR_BACKEND:
            candidate_groups[(int(row["plan_q"]), int(row["plan_n"]))].append(row)

    costs: dict[tuple[int, int], CellCost] = {}
    for key, backend in sorted(manifest_cells.items()):
        if backend != ACCELERATOR_BACKEND or key not in ledger_groups:
            continue
        planned = ledger_groups[key]
        outcomes = candidate_groups.get(key, [])
        require(
            len(planned) == PLANNED_PROCESSES,
            f"{key}: premeasure planned-process count differs",
        )
        require(
            len(outcomes) == PLANNED_PROCESSES,
            f"{key}: premeasure candidate count differs",
        )
        require(
            all(row["ledger_state"] == "completed" for row in planned),
            f"{key}: premeasure accelerator arm is signal-censored",
        )
        for row in outcomes:
            address = f"premeasure position {row['schedule_position']} cell {key}"
            require(
                row["scratch_outcome"] == "measured",
                f"{address}: censored outcomes are ineligible",
            )
            require(
                row["plan_harness_backend"] == HARNESS_BACKEND
                and row["scratch_backend"] == HARNESS_BACKEND,
                f"{address}: harness backend differs",
            )
            require(
                row["plan_batch_size"] == str(BATCH_SIZE)
                and row["scratch_batch_size"] == str(BATCH_SIZE),
                f"{address}: batch size differs",
            )
            require(
                (row["scratch_q"], row["scratch_n"]) == (str(key[0]), str(key[1])),
                f"{address}: scratch cell differs",
            )
            for field in (
                "identity_valid",
                "row_valid",
                "scratch_binary_matches_session_actual",
                "recovery_plan_matches_session",
            ):
                require(row[field] == "true", f"{address}: {field} is false")
            require(
                row["session_resolution"] == "exact",
                f"{address}: session resolution is not exact",
            )
            unavailable = tuple(
                field for field in OPTIONAL_SOURCE_PROBES if row[field] == "unavailable"
            )
            if row["provenance_complete"] == "true":
                require(not unavailable, f"{address}: complete provenance has unavailable probes")
            else:
                require(
                    key in {(7, 17), (7, 18), (7, 19)},
                    f"{address}: unexpected incomplete provenance cell",
                )
                require(
                    unavailable == OPTIONAL_SOURCE_PROBES,
                    f"{address}: unavailable provenance probes differ",
                )

        seconds = [
            parse_positive_decimal(row, "scratch_total_s", f"premeasure {key}")
            for row in outcomes
        ]
        matrices = [
            parse_positive_int(row, "scratch_matrices", f"premeasure {key}")
            for row in outcomes
        ]
        pooled, rounded = pooled_microseconds(seconds, matrices)
        costs[key] = CellCost(
            q=key[0],
            n=key[1],
            cohort="premeasure-v1",
            planned_processes=len(planned),
            measured_processes=len(outcomes),
            harness_censored=0,
            signal_censored=0,
            batch_size=BATCH_SIZE,
            total_matrices=sum(matrices),
            total_seconds=sum(seconds, Decimal(0)),
            pooled_us=pooled,
            rounded_us=rounded,
        )
    contributing_rows = [
        row
        for key in costs
        for row in candidate_groups[key]
    ]
    common["contributing_provenance_complete"] = sum(
        row["provenance_complete"] == "true" for row in contributing_rows
    )
    common["contributing_provenance_supplemented"] = sum(
        row["provenance_complete"] == "false" for row in contributing_rows
    )
    common["supplemented_cells"] = sorted(
        {
            (int(row["plan_q"]), int(row["plan_n"]))
            for row in contributing_rows
            if row["provenance_complete"] == "false"
        }
    )
    common["unavailable_source_probes"] = OPTIONAL_SOURCE_PROBES
    return costs, common


def validate_frontier_summaries(
    executions: Sequence[dict[str, str]], summaries: Sequence[dict[str, str]]
) -> None:
    summary_by_config = require_unique(
        summaries, lambda row: row["config_id"], "frontier summary"
    )
    groups: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in executions:
        groups[row["config_id"]].append(row)
    require(set(groups) == set(summary_by_config), "frontier summary config set differs")
    for config, rows in groups.items():
        summary = summary_by_config[config]
        matrices = sum(int(row["matrices"]) for row in rows)
        seconds = sum((Decimal(row["total_s"]) for row in rows), Decimal(0))
        require(summary["execution_count"] == str(len(rows)), f"{config}: execution count")
        require(summary["pooled_matrices"] == str(matrices), f"{config}: pooled matrices")
        require(Decimal(summary["pooled_total_s"]) == seconds, f"{config}: pooled seconds")
        recorded_rate = Decimal(summary["pooled_composite_matrices_per_s"])
        expected_rate = Decimal(matrices) / seconds
        require(
            abs(recorded_rate - expected_rate) <= Decimal("1e-12"),
            f"{config}: pooled rate arithmetic differs",
        )


def validate_frontier(
    manifest_cells: dict[tuple[int, int], str],
) -> tuple[dict[tuple[int, int], CellCost], dict[str, str | int]]:
    rows = read_csv(ROOT / FRONTIER_PATH, FRONTIER_REQUIRED_FIELDS)
    executions = [row for row in rows if row["record_type"] == "execution"]
    summaries = [row for row in rows if row["record_type"] == "summary"]
    require(len(executions) == 48, "frontier receipt must have 48 executions")
    require(len(summaries) == 4, "frontier receipt must have four summaries")
    execution_by_id = require_unique(
        executions, lambda row: int(row["execution_id"]), "frontier execution"
    )
    require(set(execution_by_id) == set(range(48)), "frontier execution-id set differs")
    require_unique(
        executions, lambda row: int(row["schedule_position"]), "frontier schedule"
    )
    require(
        all(
            row["outcome"] == "measured" and row["fresh_process"] == "True"
            for row in executions
        ),
        "frontier receipt contains a non-measured or non-fresh execution",
    )
    validate_frontier_summaries(executions, summaries)

    common = {
        "source_revision": one_value(executions, "git_sha", "frontier"),
        "harness_source_revision": one_value(
            executions, "harness_source_sha", "frontier"
        ),
        "deps_source_revision": one_value(
            executions, "deps_source_sha", "frontier"
        ),
        "binary_sha256": one_value(executions, "binary_sha256", "frontier"),
        "rust": one_value(executions, "rust_build_toolchain", "frontier"),
        "cargo": one_value(executions, "cargo_build_toolchain", "frontier"),
        "cpu": one_value(executions, "cpu", "frontier"),
        "logical_cpus": one_value(executions, "logical_cpus", "frontier"),
        "gpu": one_value(executions, "gpu", "frontier"),
        "rocm": one_value(executions, "rocm", "frontier"),
        "kernel": one_value(executions, "kernel", "frontier"),
        "cohort_planned": len(executions),
        "cohort_measured": len(executions),
        "harness_censored": 0,
        "signal_censored": 0,
        "contributing_provenance_complete": 24,
        "contributing_provenance_supplemented": 0,
        "supplemented_cells": [],
        "unavailable_source_probes": OPTIONAL_SOURCE_PROBES,
    }
    for field in ("source_revision", "harness_source_revision", "deps_source_revision"):
        require(
            re.fullmatch(r"[0-9a-f]{40}", str(common[field])) is not None,
            f"frontier {field} is malformed",
        )
    require(
        re.fullmatch(r"[0-9a-f]{64}", str(common["binary_sha256"])) is not None,
        "frontier binary SHA-256 is malformed",
    )

    groups: dict[tuple[int, int], list[dict[str, str]]] = defaultdict(list)
    for row in executions:
        if row["backend"] == HARNESS_BACKEND and row["batch_size"] == str(BATCH_SIZE):
            groups[(int(row["q"]), int(row["n"]))].append(row)

    costs: dict[tuple[int, int], CellCost] = {}
    for key, backend in sorted(manifest_cells.items()):
        if backend != ACCELERATOR_BACKEND or key not in groups:
            continue
        outcomes = groups[key]
        require(
            len(outcomes) == PLANNED_PROCESSES,
            f"{key}: frontier accelerator outcome count differs",
        )
        seconds = [
            parse_positive_decimal(row, "total_s", f"frontier {key}")
            for row in outcomes
        ]
        matrices = [
            parse_positive_int(row, "matrices", f"frontier {key}")
            for row in outcomes
        ]
        pooled, rounded = pooled_microseconds(seconds, matrices)
        costs[key] = CellCost(
            q=key[0],
            n=key[1],
            cohort="296a41c9-frontier",
            planned_processes=PLANNED_PROCESSES,
            measured_processes=len(outcomes),
            harness_censored=0,
            signal_censored=0,
            batch_size=BATCH_SIZE,
            total_matrices=sum(matrices),
            total_seconds=sum(seconds, Decimal(0)),
            pooled_us=pooled,
            rounded_us=rounded,
        )
    return costs, common


def derive() -> tuple[list[CellCost], dict[str, object]]:
    bindings = validate_bindings()
    manifest_cells = read_manifest(ROOT / MANIFEST_PATH)
    accelerator_keys = {
        key for key, backend in manifest_cells.items() if backend == ACCELERATOR_BACKEND
    }
    require(len(accelerator_keys) == 15, "manifest accelerator-cell count differs")

    premeasure_costs, premeasure = validate_premeasure(manifest_cells)
    frontier_costs, frontier = validate_frontier(manifest_cells)
    overlap = set(premeasure_costs) & set(frontier_costs)
    require(not overlap, f"cells have two contributing cohorts: {sorted(overlap)}")
    combined = {**premeasure_costs, **frontier_costs}
    missing = accelerator_keys - set(combined)
    extra = set(combined) - accelerator_keys
    require(not missing, f"accelerator cells have no eligible same-cell cohort: {sorted(missing)}")
    require(not extra, f"derived non-accelerator cells: {sorted(extra)}")
    require(len(combined) == 15, "derived launch-cost row count differs")
    contributing = sum(row.measured_processes for row in combined.values())
    require(contributing == 180, "contributing process-outcome count differs")
    retained_censored = int(premeasure["harness_censored"]) + int(
        premeasure["signal_censored"]
    )
    require(retained_censored == 44, "retained censored-outcome count differs")
    provenance: dict[str, object] = {
        "schema": SCHEMA_VERSION,
        "bindings": bindings,
        "manifest_cells": manifest_cells,
        "premeasure": premeasure,
        "frontier": frontier,
        "contributing": contributing,
        "retained_censored": retained_censored,
    }
    return [combined[key] for key in sorted(combined)], provenance


def render_cost_table(costs: Sequence[CellCost]) -> str:
    lines = [COST_HEADER]
    lines.extend(f"{row.q},{row.n},{row.rounded_us}" for row in costs)
    return "\n".join(lines) + "\n"


def markdown_escape(value: object) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ")


def render_receipt(costs: Sequence[CellCost], provenance: dict[str, object]) -> str:
    bindings = provenance["bindings"]
    premeasure = provenance["premeasure"]
    frontier = provenance["frontier"]
    table_hash = provenance["table_sha256"]
    premeasure_costs = [row for row in costs if row.cohort == "premeasure-v1"]
    frontier_costs = [row for row in costs if row.cohort == "296a41c9-frontier"]
    supplemented_cells = ", ".join(
        f"$({q},{n})$" for q, n in premeasure["supplemented_cells"]
    )
    unavailable_probes = "`, `".join(premeasure["unavailable_source_probes"])
    lines = [
        "# Accelerator launch costs v1",
        "",
        "This versioned receipt freezes per-cell accelerator launch-sizing evidence",
        "for the permanent zero-fraction campaign. It is execution evidence only:",
        "it does not select a backend and it contains no campaign draw, scientific",
        "result, estimate, interval, or verdict.",
        "",
        "## Bound inputs",
        "",
        f"Receipt schema: `{provenance['schema']}`.",
        "",
        "Every input is a repository-relative path bound to lowercase SHA-256.",
        "The validator refuses changed bytes before deriving a value.",
        "",
        "| Path | SHA-256 | Role |",
        "|---|---|---|",
    ]
    for path, digest, role in bindings:
        lines.append(f"| `{path}` | `{digest}` | {role} |")
    lines.extend(
        [
            f"| `{COST_TABLE_PATH}` | `{table_hash}` | production launch-cost CSV |",
            "",
            "## Eligibility and arithmetic",
            "",
            "One outcome contributes only when its exact $(q,n)$ key is accelerator-backed",
            "in the frozen manifest, its measured backend is `gpu_hip` at $M=1024$ on",
            "that same cell, its identity and collector row-validity checks pass, and all twelve",
            "planned fresh processes in that arm have finite `measured` outcomes.",
            f"The {premeasure['contributing_provenance_supplemented']} contributing rows at {supplemented_cells} retain",
            "`provenance_complete=false`. Each records `unavailable` for exactly",
            f"`{unavailable_probes}`; their session source revision,",
            "executable SHA-256, CPU, GPU, ROCm, and kernel fields remain observed.",
            "The hash-bound backend-selection receipt states the source, build, host,",
            "and device values applicable to every candidate in this producing cohort.",
            "The hash-bound RNG addendum identifies that same cohort by source revision,",
            "executable SHA-256, and ledger/candidate identities. Those artifacts",
            "supplement the four unavailable probes without relabeling any raw flag.",
            "A signal or harness censor remains in the bound cohort but makes that arm",
            "ineligible; it is never converted, replaced, interpolated, or imputed.",
            "",
            "For eligible cell $(q,n)$ with process outcomes $e$, exact decimal seconds",
            "$T_e$ and integer matrix counts $C_e$ are pooled as",
            "",
            "$$",
            "u_{q,n}=10^6\\frac{\\sum_e T_e}{\\sum_e C_e}.",
            "$$",
            "",
            "The production integer is $\\lceil u_{q,n}\\rceil$. Rounding upward is",
            "conservative for launch sizing because the stored duration is never below",
            "the pooled observed composite draw-pack-evaluate-count time per matrix.",
            "",
            f"The bound premeasure cohort retains {provenance['retained_censored']} censored outcomes:",
            f"{premeasure['harness_censored']} harness-censored candidates and",
            f"{premeasure['signal_censored']} signal-censored ledger position. None belongs",
            "to a contributing arm. The frontier receipt has no censored outcome.",
            "",
            "## Cohort provenance",
            "",
            "| Item | `premeasure-v1` | `296a41c9` frontier |",
            "|---|---|---|",
            f"| Source revision | `{premeasure['source_revision']}` | `{frontier['source_revision']}` |",
            f"| Harness / dependency revision | bound by the selection and RNG receipts | `{frontier['harness_source_revision']}` / `{frontier['deps_source_revision']}` |",
            f"| Executable SHA-256 | `{premeasure['binary_sha256']}` | `{frontier['binary_sha256']}` |",
            f"| Build toolchain | Rust 1.95.0 release+HIP, bound by the selection receipt | {markdown_escape(frontier['rust'])}; {markdown_escape(frontier['cargo'])} |",
            f"| Host processor | {markdown_escape(premeasure['cpu'])}; {premeasure['logical_cpus']} logical CPUs | {markdown_escape(frontier['cpu'])}; {frontier['logical_cpus']} logical CPUs |",
            f"| Accelerator | {markdown_escape(premeasure['gpu'])} | {markdown_escape(frontier['gpu'])} |",
            f"| Runtime / kernel | ROCm {premeasure['rocm']}; {premeasure['kernel']} | ROCm {frontier['rocm']}; {frontier['kernel']} |",
            f"| Full receipt outcomes | {premeasure['cohort_measured']} measured + {premeasure['harness_censored']} harness-censored + {premeasure['signal_censored']} signal-censored / {premeasure['cohort_planned']} | {frontier['cohort_measured']} measured + 0 censored / {frontier['cohort_planned']} |",
            f"| Contributing accelerator outcomes | {sum(row.measured_processes for row in premeasure_costs)} across {len(premeasure_costs)} cells | {sum(row.measured_processes for row in frontier_costs)} across {len(frontier_costs)} cells |",
            f"| Native / supplemented contributing provenance | {premeasure['contributing_provenance_complete']} complete in-row / {premeasure['contributing_provenance_supplemented']} bound-receipt supplemented | {frontier['contributing_provenance_complete']} complete in-row / {frontier['contributing_provenance_supplemented']} supplemented |",
            "",
            "## Rounded launch-cost rows",
            "",
            "| $q$ | $n$ | Cohort | finite / planned | censored | $M$ | $\\sum C_e$ | $\\sum T_e$ (s) | pooled $u_{q,n}$ ($\\mu$s/matrix) | $\\lceil u_{q,n}\\rceil$ |",
            "|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for row in costs:
        lines.append(
            f"| {row.q} | {row.n} | `{row.cohort}` | "
            f"{row.measured_processes} / {row.planned_processes} | "
            f"{row.harness_censored + row.signal_censored} | {row.batch_size} | "
            f"{row.total_matrices} | {row.total_seconds} | "
            f"{row.pooled_us:.9f} | {row.rounded_us} |"
        )
    lines.extend(
        [
            "",
            "## Validation",
            "",
            "The fail-closed validator checks every bound digest; unique manifest, ledger,",
            "candidate, and frontier identities; the exact manifest accelerator-key set;",
            "complete same-cell eligibility; pooled arithmetic; conservative rounding;",
            "the exact stable CSV header and positive integer rows; and byte-for-byte",
            "agreement between this rendered receipt and the production CSV.",
            "",
            "```sh",
            f"python3 {VALIDATOR_PATH} validate \\",
            f"  --manifest {MANIFEST_PATH} \\",
            f"  --cost-table {COST_TABLE_PATH} \\",
            f"  --receipt {RECEIPT_PATH}",
            "```",
            "",
        ]
    )
    return "\n".join(lines)


def require_artifact_path(path: Path, expected: str, kind: str) -> None:
    if path.resolve() != (Path.cwd() / expected).resolve():
        raise ReceiptError(f"{kind} path must be {expected}")


def render_command(args: argparse.Namespace) -> int:
    require_artifact_path(args.manifest, MANIFEST_PATH, "manifest")
    require_artifact_path(args.cost_table, COST_TABLE_PATH, "cost table")
    require_artifact_path(args.receipt, RECEIPT_PATH, "receipt")
    require(not args.cost_table.exists(), f"refusing to overwrite {args.cost_table}")
    require(not args.receipt.exists(), f"refusing to overwrite {args.receipt}")
    costs, provenance = derive()
    table = render_cost_table(costs)
    table_hash = hashlib.sha256(table.encode("utf-8")).hexdigest()
    provenance["table_sha256"] = table_hash
    rendered = render_receipt(costs, provenance)
    args.cost_table.write_text(table, encoding="utf-8")
    args.receipt.write_text(rendered, encoding="utf-8")
    print(f"wrote {args.cost_table} sha256 {table_hash}")
    print(f"wrote {args.receipt}")
    return 0


def validate_command(args: argparse.Namespace) -> int:
    require_artifact_path(args.manifest, MANIFEST_PATH, "manifest")
    require_artifact_path(args.cost_table, COST_TABLE_PATH, "cost table")
    require_artifact_path(args.receipt, RECEIPT_PATH, "receipt")
    costs, provenance = derive()
    table_text = args.cost_table.read_text(encoding="utf-8")
    manifest_cells = provenance["manifest_cells"]
    actual = parse_cost_table_text(table_text, manifest_cells)
    expected = {(row.q, row.n): row.rounded_us for row in costs}
    require(actual == expected, "production CSV differs from recomputed values")
    require(table_text == render_cost_table(costs), "production CSV rendering differs")
    table_hash = sha256_file(args.cost_table)
    provenance["table_sha256"] = table_hash
    rendered = args.receipt.read_text(encoding="utf-8")
    require(rendered == render_receipt(costs, provenance), "rendered receipt mismatch")
    print(
        f"PASS: {len(costs)} accelerator cells, "
        f"{provenance['contributing']} contributing outcomes, "
        f"{provenance['retained_censored']} retained censored outcomes, "
        f"table sha256 {table_hash}"
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command, function in (("render", render_command), ("validate", validate_command)):
        subparser = subparsers.add_parser(command)
        subparser.add_argument("--manifest", type=Path, required=True)
        subparser.add_argument("--cost-table", type=Path, required=True)
        subparser.add_argument("--receipt", type=Path, required=True)
        subparser.set_defaults(function=function)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    try:
        return args.function(args)
    except (OSError, ReceiptError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
