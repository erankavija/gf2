#!/usr/bin/env python3
"""Behavioral tests for the accelerator launch-cost v1 receipt."""

from __future__ import annotations

import copy
import subprocess
import sys
import unittest
from decimal import Decimal
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))

import accelerator_launch_costs_v1 as receipt  # noqa: E402


class ArithmeticTests(unittest.TestCase):
    def test_pooling_uses_exact_totals_and_rounds_up_conservatively(self) -> None:
        pooled, rounded = receipt.pooled_microseconds(
            [Decimal("1.000001"), Decimal("2.000002")],
            [2, 4],
        )
        self.assertEqual(pooled, Decimal("500000.5"))
        self.assertEqual(rounded, 500_001)

    def test_pooling_rejects_nonpositive_or_mismatched_inputs(self) -> None:
        cases = [
            ([], []),
            ([Decimal("1")], []),
            ([Decimal("0")], [1]),
            ([Decimal("1")], [0]),
        ]
        for seconds, matrices in cases:
            with self.subTest(seconds=seconds, matrices=matrices):
                with self.assertRaises(receipt.ReceiptError):
                    receipt.pooled_microseconds(seconds, matrices)


class CostTableTests(unittest.TestCase):
    MANIFEST_CELLS = {
        (3, 2): "generic_ryser",
        (3, 3): "accelerator",
        (5, 2): "accelerator",
    }

    def test_complete_exact_table_is_accepted(self) -> None:
        rows = receipt.parse_cost_table_text(
            "q,n,per_matrix_us\n3,3,17\n5,2,19\n",
            self.MANIFEST_CELLS,
        )
        self.assertEqual(rows, {(3, 3): 17, (5, 2): 19})

    def test_missing_duplicate_zero_malformed_processor_and_wrong_rows_fail(self) -> None:
        cases = {
            "missing": "q,n,per_matrix_us\n3,3,17\n",
            "duplicate": "q,n,per_matrix_us\n3,3,17\n3,3,18\n5,2,19\n",
            "positive": "q,n,per_matrix_us\n3,3,17\n5,2,0\n",
            "integer": "q,n,per_matrix_us\n3,3,17\n5,2,nope\n",
            "processor-backed": "q,n,per_matrix_us\n3,2,13\n3,3,17\n5,2,19\n",
            "not a manifest cell": "q,n,per_matrix_us\n3,3,17\n5,2,19\n7,2,23\n",
        }
        for expected, text in cases.items():
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(receipt.ReceiptError, expected):
                    receipt.parse_cost_table_text(text, self.MANIFEST_CELLS)


class IdentityTests(unittest.TestCase):
    def test_duplicate_input_identity_is_rejected(self) -> None:
        rows = [{"position": "1"}, {"position": "2"}, {"position": "1"}]
        with self.assertRaisesRegex(receipt.ReceiptError, "duplicate test input identity"):
            receipt.require_unique(rows, lambda row: row["position"], "test input")

    def test_receipt_rendering_changes_when_a_rounded_row_changes(self) -> None:
        derived = receipt.synthetic_cell_costs_for_test()
        rendered = receipt.render_receipt(derived, receipt.synthetic_provenance_for_test())
        changed = copy.deepcopy(derived)
        changed[0] = copy.copy(changed[0])
        changed[0].rounded_us += 1
        self.assertNotEqual(
            rendered,
            receipt.render_receipt(changed, receipt.synthetic_provenance_for_test()),
        )


class ProductionValidationTests(unittest.TestCase):
    def test_committed_table_and_receipt_validate_from_repository_root(self) -> None:
        completed = subprocess.run(
            [
                sys.executable,
                str(HERE / "accelerator_launch_costs_v1.py"),
                "validate",
                "--manifest",
                receipt.MANIFEST_PATH,
                "--cost-table",
                receipt.COST_TABLE_PATH,
                "--receipt",
                receipt.RECEIPT_PATH,
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertRegex(
            completed.stdout,
            r"PASS: 15 accelerator cells, 180 contributing outcomes, "
            r"44 retained censored outcomes, table sha256 [0-9a-f]{64}",
        )


if __name__ == "__main__":
    unittest.main()
