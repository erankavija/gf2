#!/usr/bin/env python3
"""Behavioral tests for the all-cell determinant-cost receipt."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import determinant_cost_v4 as receipt  # noqa: E402


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
                "determinant_cost_v4.py",
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

    def test_exact_v4_preregistration_participates_in_dirty_detection(self) -> None:
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

    def test_v4_report_binds_v4_preregistration_and_machine_receipt(self) -> None:
        rendered = receipt.render_report(complete_rows(), "e" * 64)
        self.assertIn(f"`{receipt.PREREGISTRATION_PATH}`", rendered)
        self.assertIn(f"`{receipt.RECEIPT_PATH}`", rendered)
        self.assertIn(f"`{receipt.SCHEMA_VERSION}`", rendered)

    def test_superseded_schema_is_rejected(self) -> None:
        rows = complete_rows()
        rows[0]["schema_version"] = "determinant-companion-v3"
        with self.assertRaisesRegex(receipt.ReceiptError, "schema version differs"):
            receipt.validate_rows(rows)

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
        scratch = [measured_row(3, 4, 2)]
        merged = receipt.merge_process_rows(
            process_index=2,
            exit_code=-15,
            scratch_rows=scratch,
            provenance=receipt.provenance_from_row(scratch[0]),
            invocation=json.loads(scratch[0]["invocation"]),
            stdout=b"partial output",
            stderr=b"terminated",
        )
        self.assertEqual(len(merged), 63)
        self.assertEqual(merged[0]["outcome"], "measured")
        self.assertEqual(merged[1]["outcome"], "signal_censored")
        self.assertEqual(merged[1]["process_exit_code"], "-15")
        self.assertEqual(merged[1]["elapsed_determinant_ns"], "")

    def test_rendered_receipt_comparison_is_byte_exact(self) -> None:
        rows = complete_rows()
        expected = receipt.render_report(rows, "d" * 64)
        receipt.validate_rendered_report(rows, "d" * 64, expected)
        with self.assertRaisesRegex(receipt.ReceiptError, "rendered receipt mismatch"):
            receipt.validate_rendered_report(rows, "d" * 64, expected + "drift\n")


if __name__ == "__main__":
    unittest.main()
