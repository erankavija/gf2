#!/usr/bin/env python3
"""Behavioral tests for the all-cell determinant-cost receipt."""

from __future__ import annotations

import copy
import csv
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import determinant_cost_v5 as receipt  # noqa: E402


def measured_row(
    q: int,
    n: int,
    process_index: int,
) -> dict[str, str]:
    starts = [5 * (process_index - 1) + offset for offset in range(5)]
    repetition_elapsed = [1_000_000 + process_index + offset for offset in range(5)]
    calls = 100
    elapsed = sum(repetition_elapsed)
    sample_count = calls * len(repetition_elapsed)
    process_argv = [
        "/tmp/determinant_companion-deadbeef",
        "--execution",
        str(process_index),
        "--output",
        f"/tmp/process-{process_index}.csv",
    ]
    invocation = json.dumps(process_argv, separators=(",", ":"))
    return {
        "schema_version": receipt.SCHEMA_VERSION,
        "process_index": str(process_index),
        "q": str(q),
        "n": str(n),
        "outcome": "measured",
        "process_exit_code": "0",
        "backend": "fieldmatrix_det_ple",
        "rng_algorithm": receipt.RNG_ALGORITHM,
        "rng_version": receipt.RNG_VERSION,
        "rng_entry_mapping": receipt.RNG_ENTRY_MAPPING,
        "seed_root": f"0x{receipt.SEED_ROOT:016x}",
        "cell_seed": f"0x{receipt.cell_seed(q, n):016x}",
        "fixture_count": "32",
        "fixture_starts": ";".join(str(value) for value in starts),
        "warmup_policy": receipt.WARMUP_POLICY,
        "warmup_calls": "127",
        "warmup_elapsed_ns": "20000000",
        "target_ms": "250",
        "timed_repetitions": "5",
        "calls_per_repetition": str(calls),
        "repetition_elapsed_ns": ";".join(str(value) for value in repetition_elapsed),
        "sample_count": str(sample_count),
        "elapsed_determinant_ns": str(elapsed),
        "ns_per_matrix": f"{elapsed / sample_count:.9f}",
        "started_unix_ns": "1787990000000000000",
        "finished_unix_ns": "1787990001000000000",
        "baseline_path": receipt.BASELINE_PATH,
        "baseline_git_revision": receipt.BASELINE_GIT_REVISION,
        "baseline_sha256": receipt.BASELINE_SHA256,
        "cell_ceiling_seconds": str(receipt.CELL_CEILING_SECONDS),
        "productive_compute_seconds": str(receipt.PRODUCTIVE_COMPUTE_SECONDS),
        "reserve_fraction": receipt.RESERVE_FRACTION,
        "git_revision": "a" * 40,
        "source_dirty": "false",
        "rustc": "rustc 1.95.0 (test)",
        "cargo_profile": "release-bench",
        "binary_sha256": "b" * 64,
        "hostname": "test-host",
        "cpu_model": "test-cpu",
        "kernel": "test-kernel",
        "governor": "powersave",
        "boost": "enabled",
        "affinity": "6-11",
        "process_count_per_cell": "5",
        "worker_count": "1",
        "benchmark_wrapper": "dev/scripts/ccx1-bench-flock.sh:ccx1",
        "cohort_invocation": json.dumps(
            [
                "python3",
                "determinant_cost_v5.py",
                "run",
            ],
            separators=(",", ":"),
        ),
        "invocation": invocation,
        "stdout_sha256": hashlib.sha256(b"").hexdigest(),
        "stderr_sha256": hashlib.sha256(b"").hexdigest(),
    }


def complete_rows() -> list[dict[str, str]]:
    return [
        measured_row(q, n, process_index)
        for q, n in receipt.campaign_cells()
        for process_index in range(1, 6)
    ]


def scratch_row(q: int, n: int, process_index: int) -> dict[str, str]:
    row = measured_row(q, n, process_index)
    return {field: row[field] for field in receipt.SCRATCH_FIELDNAMES}


def complete_scratch_rows(process_index: int) -> list[dict[str, str]]:
    return [scratch_row(q, n, process_index) for q, n in receipt.campaign_cells()]


def read_scratch_records(records: list[list[str]]) -> list[dict[str, str]]:
    with tempfile.TemporaryDirectory() as temporary_directory:
        scratch_path = Path(temporary_directory) / "scratch.csv"
        with scratch_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.writer(handle, lineterminator="\n")
            writer.writerow(receipt.SCRATCH_FIELDNAMES)
            writer.writerows(records)
        return receipt.read_scratch_csv(scratch_path)


def scratch_values(row: dict[str, str], through: str | None = None) -> list[str]:
    fields = receipt.SCRATCH_FIELDNAMES
    if through is not None:
        fields = fields[: fields.index(through) + 1]
    return [row[field] for field in fields]


class GridTests(unittest.TestCase):
    def test_campaign_grid_and_fixed_counts_match_protocol_boundaries(self) -> None:
        cells = receipt.campaign_cells()
        self.assertEqual(len(cells), 63)
        self.assertEqual(len(set(cells)), 63)
        self.assertEqual(cells[0], (3, 4))
        self.assertEqual(cells[-1], (7, 20))
        self.assertEqual(receipt.fixed_sample_count(3, 20), 20_000_000)
        self.assertEqual(receipt.fixed_sample_count(3, 21), 222_223)
        self.assertEqual(receipt.fixed_sample_count(5, 16), 16_000_000)
        self.assertEqual(receipt.fixed_sample_count(5, 17), 160_000)
        self.assertEqual(receipt.fixed_sample_count(7, 16), 12_244_898)
        self.assertEqual(receipt.fixed_sample_count(7, 17), 122_449)


class ProvenanceTests(unittest.TestCase):
    def test_measurement_source_closure_is_canonical_and_complete(self) -> None:
        self.assertEqual(
            set(receipt.RELEVANT_SOURCE_PATHS),
            {
                "Cargo.toml",
                "Cargo.lock",
                "crates/gf2-core",
                "crates/gf2-algebra",
                receipt.BENCHMARK_WRAPPER_PATH,
                receipt.RUNNER_PATH,
                receipt.PREREGISTRATION_PATH,
                receipt.BASELINE_PATH,
            },
        )

    def test_benchmark_wrapper_participates_in_source_dirty_detection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            repository = Path(temporary_directory)
            wrapper = repository / "dev/scripts/ccx1-bench-flock.sh"
            wrapper.parent.mkdir(parents=True)
            wrapper.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
            subprocess.run(["git", "init", "--quiet"], cwd=repository, check=True)
            subprocess.run(
                ["git", "config", "user.email", "receipt-test@example.invalid"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Receipt Test"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "add", str(wrapper.relative_to(repository))],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "commit", "--quiet", "--no-gpg-sign", "-m", "test fixture"],
                cwd=repository,
                check=True,
            )

            git = ["git", "-C", str(repository)]
            self.assertEqual(receipt.relevant_source_status(git), "")

            wrapper.write_text("#!/usr/bin/env bash\nexit 1\n", encoding="utf-8")

            self.assertIn(
                "dev/scripts/ccx1-bench-flock.sh",
                receipt.relevant_source_status(git),
            )

    def test_exact_v5_preregistration_participates_in_dirty_detection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            repository = Path(temporary_directory)
            paths = [
                receipt.BENCHMARK_WRAPPER_PATH,
                receipt.PREREGISTRATION_PATH,
            ]
            for relative in paths:
                path = repository / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(f"frozen {relative}\n", encoding="utf-8")
            subprocess.run(["git", "init", "--quiet"], cwd=repository, check=True)
            subprocess.run(
                ["git", "config", "user.email", "receipt-test@example.invalid"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Receipt Test"],
                cwd=repository,
                check=True,
            )
            subprocess.run(["git", "add", *paths], cwd=repository, check=True)
            subprocess.run(
                ["git", "commit", "--quiet", "--no-gpg-sign", "-m", "test fixture"],
                cwd=repository,
                check=True,
            )

            git = ["git", "-C", str(repository)]
            self.assertEqual(receipt.relevant_source_status(git), "")

            preregistration = repository / receipt.PREREGISTRATION_PATH
            preregistration.write_text("changed contract\n", encoding="utf-8")

            self.assertIn(
                receipt.PREREGISTRATION_PATH,
                receipt.relevant_source_status(git),
            )


class ValidationTests(unittest.TestCase):
    def test_complete_receipt_validates_and_renders_every_cell(self) -> None:
        rows = complete_rows()
        summaries = receipt.validate_rows(rows)
        self.assertEqual(len(summaries), 63)
        self.assertTrue(all(summary.measured_processes == 5 for summary in summaries))
        rendered = receipt.render_report(rows, "c" * 64)
        self.assertEqual(rendered.count("\n| 3 |"), 25)
        self.assertEqual(rendered.count("\n| 5 |"), 21)
        self.assertEqual(rendered.count("\n| 7 |"), 17)
        self.assertIn("No failed or censored process outcome was observed", rendered)

    def test_v5_report_binds_v5_preregistration_and_machine_receipt(self) -> None:
        rendered = receipt.render_report(complete_rows(), "e" * 64)
        self.assertIn(f"`{receipt.PREREGISTRATION_PATH}`", rendered)
        self.assertIn(f"`{receipt.RECEIPT_PATH}`", rendered)
        self.assertIn(f"`{receipt.SCHEMA_VERSION}`", rendered)
        self.assertIn(f"`{receipt.RNG_ALGORITHM}`", rendered)
        self.assertIn(f"`{receipt.RNG_VERSION}`", rendered)
        self.assertIn(f"`{receipt.BASELINE_PATH}`", rendered)
        self.assertIn(f"`{receipt.BASELINE_GIT_REVISION}`", rendered)
        self.assertIn(f"`{receipt.BASELINE_SHA256}`", rendered)

    def test_superseded_schema_is_rejected(self) -> None:
        rows = complete_rows()
        for schema in ("determinant-companion-v3", "determinant-companion-v4"):
            with self.subTest(schema=schema):
                changed = copy.deepcopy(rows)
                changed[0]["schema_version"] = schema
                with self.assertRaisesRegex(receipt.ReceiptError, "schema version differs"):
                    receipt.validate_rows(changed)

    def test_rng_and_baseline_identities_fail_closed(self) -> None:
        cases = {
            "rng_algorithm": "another_rng",
            "rng_version": "another-version",
            "rng_entry_mapping": "another-mapping",
            "baseline_path": "another/protocol.md",
            "baseline_git_revision": "f" * 40,
            "baseline_sha256": "f" * 64,
            "cell_ceiling_seconds": "1",
            "productive_compute_seconds": "1",
            "reserve_fraction": "0.10",
        }
        for field, value in cases.items():
            with self.subTest(field=field):
                rows = complete_rows()
                rows[0][field] = value
                with self.assertRaisesRegex(receipt.ReceiptError, field.replace("_", " ")):
                    receipt.validate_rows(rows)

    def test_process_mean_interval_and_projection_are_exact(self) -> None:
        rows = complete_rows()
        cell_rows = [row for row in rows if row["q"] == "3" and row["n"] == "4"]
        process_values = [8_000.0, 9_000.0, 10_000.0, 11_000.0, 12_000.0]
        for row, value in zip(cell_rows, process_values, strict=True):
            samples = int(row["sample_count"])
            elapsed = int(value * samples)
            repetitions = [elapsed // 5] * 5
            row["repetition_elapsed_ns"] = ";".join(map(str, repetitions))
            row["elapsed_determinant_ns"] = str(sum(repetitions))
            row["ns_per_matrix"] = f"{sum(repetitions) / samples:.9f}"

        summary = receipt.validate_rows(rows)[0]
        expected_mean = 10_000.0
        expected_stddev = (2_500_000.0) ** 0.5
        expected_half_width = (
            receipt.STUDENT_T_975_DF4 * expected_stddev / (5.0**0.5)
        )
        self.assertAlmostEqual(summary.process_mean_ns_per_matrix, expected_mean)
        self.assertAlmostEqual(summary.process_stddev_ns_per_matrix, expected_stddev)
        self.assertAlmostEqual(
            summary.ci95_lower_ns_per_matrix,
            expected_mean - expected_half_width,
        )
        self.assertAlmostEqual(
            summary.ci95_upper_ns_per_matrix,
            expected_mean + expected_half_width,
        )
        self.assertAlmostEqual(
            summary.projected_upper_seconds,
            (expected_mean + expected_half_width) * 20_000_000 / 1e9,
        )

        rendered = receipt.render_report(rows, "f" * 64)
        self.assertIn("process mean 95% CI", rendered)
        self.assertIn("pooled audit", rendered)
        self.assertIn("projected 95% CI", rendered)

    def test_censored_cell_has_no_process_interval_or_ceiling_verdict(self) -> None:
        rows = complete_rows()
        first = rows[0]
        first["outcome"] = "signal_censored"
        for field in receipt.TIMING_FIELDS:
            first[field] = ""
        summary = receipt.validate_rows(rows)[0]
        self.assertIsNotNone(summary.pooled_ns_per_matrix)
        self.assertIsNone(summary.process_mean_ns_per_matrix)
        self.assertIsNone(summary.ci95_lower_ns_per_matrix)
        self.assertIsNone(summary.ci95_upper_ns_per_matrix)
        self.assertIsNone(summary.projected_upper_seconds)
        rendered = receipt.render_report(rows, "1" * 64)
        self.assertIn("not estimable", rendered)

    def test_duplicate_or_missing_process_address_fails_closed(self) -> None:
        rows = complete_rows()
        with self.assertRaisesRegex(receipt.ReceiptError, "duplicate address"):
            receipt.validate_rows(rows + [copy.deepcopy(rows[0])])
        with self.assertRaisesRegex(receipt.ReceiptError, "missing address"):
            receipt.validate_rows(rows[1:])

    def test_inconsistent_pooled_arithmetic_fails_closed(self) -> None:
        rows = complete_rows()
        rows[0]["elapsed_determinant_ns"] = "1"
        with self.assertRaisesRegex(receipt.ReceiptError, "elapsed sum"):
            receipt.validate_rows(rows)

    def test_nonmeasured_row_cannot_carry_fabricated_timing(self) -> None:
        rows = complete_rows()
        rows[0]["outcome"] = "signal_censored"
        with self.assertRaisesRegex(receipt.ReceiptError, "non-measured timing"):
            receipt.validate_rows(rows)

    def test_malformed_build_identity_fails_closed(self) -> None:
        rows = complete_rows()
        for row in rows:
            row["binary_sha256"] = "not-a-digest"
        with self.assertRaisesRegex(receipt.ReceiptError, "binary SHA-256"):
            receipt.validate_rows(rows)

    def test_missing_scratch_rows_become_retained_process_outcomes(self) -> None:
        scratch = [scratch_row(3, 4, 2)]
        provenance_row = measured_row(3, 4, 2)
        merged = receipt.merge_process_rows(
            process_index=2,
            exit_code=-15,
            scratch_rows=scratch,
            provenance=receipt.provenance_from_row(provenance_row),
            invocation=json.loads(provenance_row["invocation"]),
            stdout=b"partial output",
            stderr=b"terminated",
        )
        self.assertEqual(len(merged), 63)
        self.assertEqual(merged[0]["outcome"], "measured")
        self.assertEqual(merged[1]["outcome"], "signal_censored")
        self.assertEqual(merged[1]["process_exit_code"], "-15")
        self.assertEqual(merged[1]["elapsed_determinant_ns"], "")

    def test_signal_truncated_address_becomes_censored_in_complete_receipt(self) -> None:
        rows: list[dict[str, str]] = []
        for process_index in range(1, receipt.PROCESS_COUNT + 1):
            provenance_row = measured_row(3, 4, process_index)
            if process_index == 2:
                scratch = [scratch_row(3, 4, process_index)]
                truncated = scratch_row(3, 5, process_index)
                scratch.extend(
                    read_scratch_records([scratch_values(truncated, "sample_count")])
                )
                exit_code = -15
            else:
                scratch = complete_scratch_rows(process_index)
                exit_code = 0
            rows.extend(
                receipt.merge_process_rows(
                    process_index=process_index,
                    exit_code=exit_code,
                    scratch_rows=scratch,
                    provenance=receipt.provenance_from_row(provenance_row),
                    invocation=json.loads(provenance_row["invocation"]),
                    stdout=b"partial output" if exit_code else b"",
                    stderr=b"terminated" if exit_code else b"",
                )
            )

        self.assertEqual(len(rows), 315)
        process_two = {
            (int(row["q"]), int(row["n"])): row
            for row in rows
            if row["process_index"] == "2"
        }
        self.assertEqual(process_two[(3, 4)]["outcome"], "measured")
        self.assertEqual(process_two[(3, 5)]["outcome"], "signal_censored")
        self.assertEqual(process_two[(3, 5)]["elapsed_determinant_ns"], "")
        self.assertEqual(len(receipt.validate_rows(rows)), 63)

        with tempfile.TemporaryDirectory() as temporary_directory:
            receipt_path = Path(temporary_directory) / "receipt.csv"
            receipt.write_csv_exclusive(receipt_path, rows)
            persisted = receipt.read_csv(receipt_path)
            self.assertEqual(len(persisted), 315)
            self.assertEqual(len(receipt.validate_rows(persisted)), 63)

    def test_successful_process_with_early_tail_is_harness_censored(self) -> None:
        provenance_row = measured_row(3, 4, 1)
        early_tail = read_scratch_records([[receipt.SCHEMA_VERSION]])
        scratch = [scratch_row(3, 4, 1), *early_tail]
        merged = receipt.merge_process_rows(
            process_index=1,
            exit_code=0,
            scratch_rows=scratch,
            provenance=receipt.provenance_from_row(provenance_row),
            invocation=json.loads(provenance_row["invocation"]),
            stdout=b"",
            stderr=b"",
        )
        self.assertEqual(merged[0]["outcome"], "measured")
        self.assertEqual(merged[1]["outcome"], "harness_censored")
        self.assertTrue(any(row["outcome"] != "measured" for row in merged))
        self.assertEqual(receipt.cohort_result_code(False, merged), 7)

    def test_early_unparseable_tail_retains_complete_315_row_receipt(self) -> None:
        rows: list[dict[str, str]] = []
        for process_index in range(1, receipt.PROCESS_COUNT + 1):
            provenance_row = measured_row(3, 4, process_index)
            if process_index == 2:
                early_tail = read_scratch_records([[receipt.SCHEMA_VERSION]])
                scratch = [scratch_row(3, 4, process_index), *early_tail]
                exit_code = -15
            else:
                scratch = complete_scratch_rows(process_index)
                exit_code = 0
            rows.extend(
                receipt.merge_process_rows(
                    process_index=process_index,
                    exit_code=exit_code,
                    scratch_rows=scratch,
                    provenance=receipt.provenance_from_row(provenance_row),
                    invocation=json.loads(provenance_row["invocation"]),
                    stdout=b"partial output" if exit_code else b"",
                    stderr=b"terminated" if exit_code else b"",
                )
            )

        self.assertEqual(len(rows), 315)
        process_two = [row for row in rows if row["process_index"] == "2"]
        self.assertEqual(process_two[0]["outcome"], "measured")
        self.assertTrue(
            all(row["outcome"] == "signal_censored" for row in process_two[1:])
        )
        self.assertEqual(len(receipt.validate_rows(rows)), 63)
        with tempfile.TemporaryDirectory() as temporary_directory:
            receipt_path = Path(temporary_directory) / "receipt.csv"
            receipt.write_csv_exclusive(receipt_path, rows)
            persisted = receipt.read_csv(receipt_path)
            self.assertEqual(len(persisted), 315)
            self.assertEqual(len(receipt.validate_rows(persisted)), 63)

    def test_malformed_non_tail_scratch_row_is_rejected(self) -> None:
        provenance_row = measured_row(3, 4, 1)
        malformed = read_scratch_records([[receipt.SCHEMA_VERSION]])[0]
        scratch = [malformed, scratch_row(3, 4, 1)]
        with self.assertRaisesRegex(receipt.ReceiptError, "incomplete non-tail"):
            receipt.merge_process_rows(
                process_index=1,
                exit_code=-15,
                scratch_rows=scratch,
                provenance=receipt.provenance_from_row(provenance_row),
                invocation=json.loads(provenance_row["invocation"]),
                stdout=b"",
                stderr=b"terminated",
            )

    def test_duplicate_complete_scratch_address_is_rejected(self) -> None:
        provenance_row = measured_row(3, 4, 1)
        duplicate = scratch_row(3, 4, 1)
        with self.assertRaisesRegex(receipt.ReceiptError, "duplicate cell"):
            receipt.merge_process_rows(
                process_index=1,
                exit_code=0,
                scratch_rows=[duplicate, copy.deepcopy(duplicate)],
                provenance=receipt.provenance_from_row(provenance_row),
                invocation=json.loads(provenance_row["invocation"]),
                stdout=b"",
                stderr=b"",
            )

    def test_empty_or_header_only_scratch_censors_every_missing_address(self) -> None:
        provenance_row = measured_row(3, 4, 1)
        self.assertEqual(read_scratch_records([]), [])
        with tempfile.TemporaryDirectory() as temporary_directory:
            empty_path = Path(temporary_directory) / "empty.csv"
            empty_path.touch()
            self.assertEqual(receipt.read_scratch_csv(empty_path), [])
        merged = receipt.merge_process_rows(
            process_index=1,
            exit_code=0,
            scratch_rows=[],
            provenance=receipt.provenance_from_row(provenance_row),
            invocation=json.loads(provenance_row["invocation"]),
            stdout=b"",
            stderr=b"",
        )
        self.assertEqual(len(merged), 63)
        self.assertTrue(all(row["outcome"] == "harness_censored" for row in merged))

    def test_scratch_header_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            scratch_path = Path(temporary_directory) / "scratch.csv"
            scratch_path.write_text("schema_version,q\nv5,3\n", encoding="utf-8")
            with self.assertRaisesRegex(receipt.ReceiptError, "scratch schema columns"):
                receipt.read_scratch_csv(scratch_path)

    def test_rendered_receipt_comparison_is_byte_exact(self) -> None:
        rows = complete_rows()
        expected = receipt.render_report(rows, "d" * 64)
        receipt.validate_rendered_report(rows, "d" * 64, expected)
        with self.assertRaisesRegex(receipt.ReceiptError, "rendered receipt mismatch"):
            receipt.validate_rendered_report(rows, "d" * 64, expected + "drift\n")


if __name__ == "__main__":
    unittest.main()
