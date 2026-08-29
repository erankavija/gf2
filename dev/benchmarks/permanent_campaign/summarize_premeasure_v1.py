#!/usr/bin/env python3
"""Validate and summarize the immutable premeasure-v1 collector outputs."""

from __future__ import annotations

import csv
import hashlib
import math
import sys
from collections import defaultdict
from decimal import Decimal
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
HOME = ROOT / "dev/benchmarks/permanent_campaign"
LEDGER = HOME / "premeasure-v1-ledger.csv"
CANDIDATES = HOME / "premeasure-v1-candidates.csv"
OUTPUT = HOME / "premeasure-v1-cell-summary.csv"

EXPECTED_LEDGER_SHA256 = "d1efd9dcfa39b8498db1e04ba0720de2bf96677fc6b820ffadc3f1919848d046"
EXPECTED_CANDIDATES_SHA256 = "272a524185c14515394a24a5e7385e07b7dc5620c0a488a12438668b72b8c6b8"
EXPECTED_PLAN_SHA256 = "c268d554411c65bc6c3366247775e8d8d5b177b46f064e850a0da5b3d3b38640"
EXPECTED_SOURCE_REVISION = "1350d5b46cd541093882537a89ea35db05a7afc5"
EXPECTED_BINARY_SHA256 = "1198cce47de06a6793fd1b5d880d559f020acffebc3d237137fa5c1326775606"
PLANNED_PROCESSES = 12


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        return list(csv.DictReader(source))


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate_sources(
    ledger: list[dict[str, str]], candidates: list[dict[str, str]]
) -> None:
    require(digest(LEDGER) == EXPECTED_LEDGER_SHA256, "ledger digest changed")
    require(
        digest(CANDIDATES) == EXPECTED_CANDIDATES_SHA256,
        "candidate digest changed",
    )
    require(len(ledger) == 1_440, "ledger must cover 1,440 process positions")
    require(len(candidates) == 1_439, "candidate projection must contain 1,439 rows")
    positions = sorted(int(row["schedule_position"]) for row in ledger)
    require(positions == list(range(1_440)), "ledger schedule positions are not complete")
    require(
        {row["session_source_revision"] for row in ledger}
        == {EXPECTED_SOURCE_REVISION},
        "cohort source revision changed",
    )
    require(
        {row["session_plan_sha256"] for row in ledger} == {EXPECTED_PLAN_SHA256},
        "cohort plan digest changed",
    )
    terminal = [row for row in ledger if row["ledger_state"] == "censored"]
    require(len(terminal) == 1, "expected exactly one signal-censored process")
    censored = terminal[0]
    require(
        (
            censored["schedule_position"],
            censored["plan_q"],
            censored["plan_n"],
            censored["plan_manifest_backend"],
            censored["receipt_failure"],
        )
        == ("539", "3", "26", "accelerator", "interrupted by runner signal"),
        "signal-censored process identity changed",
    )
    require(
        censored["candidate_emitted"] == "false",
        "signal-censored process must not emit a candidate",
    )
    require(
        {row["observed_running_binary_sha256"] for row in candidates}
        == {EXPECTED_BINARY_SHA256},
        "candidate binary identity changed",
    )
    require(
        all(row["candidate_emitted"] == "true" for row in candidates),
        "candidate file contains a non-candidate row",
    )
    for row in candidates:
        rate = float(row["scratch_composite_matrices_per_s"])
        outcome = row["scratch_outcome"]
        require(
            (outcome == "measured" and math.isfinite(rate))
            or (outcome == "censored" and math.isnan(rate)),
            "candidate outcome and composite-rate state disagree at schedule position "
            + row["schedule_position"],
        )


def render_summary(
    ledger: list[dict[str, str]], candidates: list[dict[str, str]]
) -> None:
    ledger_groups: dict[tuple[int, int, str], list[dict[str, str]]] = defaultdict(list)
    candidate_groups: dict[tuple[int, int, str], list[dict[str, str]]] = defaultdict(list)
    for row in ledger:
        key = (int(row["plan_q"]), int(row["plan_n"]), row["plan_manifest_backend"])
        ledger_groups[key].append(row)
    for row in candidates:
        key = (int(row["plan_q"]), int(row["plan_n"]), row["plan_manifest_backend"])
        candidate_groups[key].append(row)

    require(len(ledger_groups) == 120, "ledger must contain 120 configurations")
    require(len(candidate_groups) == 120, "candidate file must contain 120 configurations")
    for key, group in ledger_groups.items():
        require(len(group) == PLANNED_PROCESSES, f"{key} does not have 12 planned processes")

    fieldnames = [
        "schema",
        "source_ledger_sha256",
        "source_candidates_sha256",
        "source_revision",
        "binary_sha256",
        "q",
        "n",
        "manifest_backend",
        "harness_backend",
        "batch_size",
        "planned_processes",
        "candidate_rows",
        "measured_rows",
        "harness_censored_rows",
        "signal_censored_rows",
        "observed_mean_of_finite_composite_rates",
        "selection_status",
    ]
    with OUTPUT.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fieldnames, lineterminator="\n")
        writer.writeheader()
        for key in sorted(ledger_groups):
            planned = ledger_groups[key]
            observed = candidate_groups[key]
            measured = [row for row in observed if row["scratch_outcome"] == "measured"]
            harness_censored = [
                row for row in observed if row["scratch_outcome"] == "censored"
            ]
            signal_censored = [
                row for row in planned if row["ledger_state"] == "censored"
            ]
            finite_rates = [
                Decimal(row["scratch_composite_matrices_per_s"]) for row in measured
            ]
            observed_mean = (
                format(sum(finite_rates) / Decimal(len(finite_rates)), ".10f")
                if finite_rates
                else ""
            )
            status = (
                "complete"
                if len(measured) == PLANNED_PROCESSES
                else "ranking_rule_required"
            )
            writer.writerow(
                {
                    "schema": "permanent-campaign-premeasure-summary-v1",
                    "source_ledger_sha256": EXPECTED_LEDGER_SHA256,
                    "source_candidates_sha256": EXPECTED_CANDIDATES_SHA256,
                    "source_revision": EXPECTED_SOURCE_REVISION,
                    "binary_sha256": EXPECTED_BINARY_SHA256,
                    "q": key[0],
                    "n": key[1],
                    "manifest_backend": key[2],
                    "harness_backend": planned[0]["plan_harness_backend"],
                    "batch_size": planned[0]["plan_batch_size"],
                    "planned_processes": len(planned),
                    "candidate_rows": len(observed),
                    "measured_rows": len(measured),
                    "harness_censored_rows": len(harness_censored),
                    "signal_censored_rows": len(signal_censored),
                    "observed_mean_of_finite_composite_rates": observed_mean,
                    "selection_status": status,
                }
            )


def main() -> int:
    try:
        ledger = rows(LEDGER)
        candidates = rows(CANDIDATES)
        validate_sources(ledger, candidates)
        render_summary(ledger, candidates)
    except (OSError, ValueError) as error:
        print(f"premeasure-v1 summary refused: {error}", file=sys.stderr)
        return 1
    print(f"wrote {OUTPUT.relative_to(ROOT)}")
    print(f"sha256={digest(OUTPUT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
