"""Black-box schema-3 analysis coverage from tool-produced experiment fixtures."""
from __future__ import annotations

import csv
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

REPOSITORY = Path(__file__).resolve().parents[1]
SCRIPT = REPOSITORY / "scripts/permanent_zero_fraction_analysis.py"
FIXTURES = REPOSITORY / "experiments/permanent-statistics/tests/fixtures/analysis"
GOLDEN = FIXTURES / "golden/small-campaign"
SPEC = importlib.util.spec_from_file_location("permanent_zero_fraction_analysis", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
ANALYSIS = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = ANALYSIS
SPEC.loader.exec_module(ANALYSIS)


class PermanentZeroFractionAnalysisTests(unittest.TestCase):
    def run_analysis(self, dataset: Path) -> subprocess.CompletedProcess[str]:
        environment = dict(os.environ, CARGO_NET_OFFLINE="true")
        return subprocess.run([sys.executable, str(SCRIPT), str(dataset)], cwd=REPOSITORY,
                              text=True, capture_output=True, check=False, env=environment)

    def refresh_checksum(self, dataset: Path, relative_path: str) -> None:
        path = dataset / relative_path
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        checksum_path = dataset / "checksums.sha256"
        checksum_path.write_text("".join(
            f"{digest if recorded == relative_path else old}  {recorded}\n"
            for old, recorded in (line.split("  ", 1) for line in checksum_path.read_text().splitlines())
        ))

    def copy_fixture(self, parent: str, name: str = "small-campaign") -> Path:
        dataset = Path(parent) / name
        shutil.copytree(FIXTURES / name, dataset)
        return dataset

    def test_valid_dataset_matches_golden_bytes_and_reruns_identically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = self.copy_fixture(temporary)
            first = self.run_analysis(dataset)
            self.assertEqual(first.returncode, 0, first.stderr)
            output = dataset / "derived/small-campaign/zero-fraction-analysis"
            first_bytes = {p.name: p.read_bytes() for p in output.iterdir()}
            self.assertEqual(first_bytes, {p.name: p.read_bytes() for p in GOLDEN.iterdir()})
            second = self.run_analysis(dataset)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual(first_bytes, {p.name: p.read_bytes() for p in output.iterdir()})
            rows = list(csv.DictReader(first_bytes["cells.csv"].decode().splitlines()))
            self.assertEqual({int(r["q"]) for r in rows}, {3, 5, 7})
            self.assertTrue(all(r["terminal_state"] == "completed" for r in rows))
            claim = json.loads(first_bytes["literature-claim.json"])
            self.assertEqual(claim["fields"], [5, 7])
            self.assertTrue((REPOSITORY / claim["source"]).is_file())

    def test_halted_cells_retain_counts_and_no_estimates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = self.copy_fixture(temporary, "small-campaign-halted")
            result = self.run_analysis(dataset)
            self.assertEqual(result.returncode, 0, result.stderr)
            with (dataset / "derived/small-campaign-halted/zero-fraction-analysis/cells.csv").open() as handle:
                rows = list(csv.DictReader(handle))
            self.assertEqual(rows[0]["terminal_state"], "completed")
            for row in rows[1:]:
                self.assertEqual(row["terminal_state"], "halted")
                self.assertEqual(row["halt_reason"], "backend_unavailable")
                self.assertEqual(row["pooled_sample_count"], "0")
                self.assertEqual(row["estimate"], "")

    def test_checksum_mismatch_refuses_without_derived_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = self.copy_fixture(temporary, "checksum-mismatch")
            result = self.run_analysis(dataset)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("checksum mismatch", result.stderr)
            self.assertFalse((dataset / "derived").exists())

    def test_terminal_or_aggregate_mismatch_refuses_after_checksum_verification(self) -> None:
        for column, value in [("terminal_state", ""), ("permanent_zero_count", "1")]:
            with self.subTest(column=column), tempfile.TemporaryDirectory() as temporary:
                dataset = self.copy_fixture(temporary)
                path = dataset / "summary.csv"
                with path.open() as handle:
                    reader = csv.DictReader(handle)
                    fields, rows = reader.fieldnames, list(reader)
                rows[0][column] = value
                with path.open("w", newline="") as handle:
                    writer = csv.DictWriter(handle, fieldnames=fields, lineterminator="\n")
                    writer.writeheader()
                    writer.writerows(rows)
                self.refresh_checksum(dataset, "summary.csv")
                result = self.run_analysis(dataset)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("canonical dataset conformance refused", result.stderr)
                self.assertFalse((dataset / "derived").exists())

    def test_unsupported_field_fixture_refuses_after_its_checksums_verify(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = self.copy_fixture(temporary, "invalid-q")
            for line in (dataset / "checksums.sha256").read_text().splitlines():
                expected, path = line.split("  ", 1)
                self.assertEqual(hashlib.sha256((dataset / path).read_bytes()).hexdigest(), expected)
            result = self.run_analysis(dataset)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("invalid manifest cell", result.stderr)
            self.assertFalse((dataset / "derived").exists())

    def test_wilson_transliteration_matches_versioned_source_interval(self) -> None:
        estimate, lower, upper = ANALYSIS.wilson_95(35_456_365_448, 100_000_000_000)
        self.assertAlmostEqual(estimate, 0.354563654480, places=12)
        self.assertAlmostEqual(lower, 0.354560689505, places=12)
        self.assertAlmostEqual(upper, 0.354566619467, places=12)


if __name__ == "__main__":
    unittest.main()
