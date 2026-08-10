"""Black-box fixture coverage for permanent-zero-fraction analysis."""

from __future__ import annotations

import csv
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import importlib.util
import hashlib
import re
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
SCRIPT = REPOSITORY / "scripts/permanent_zero_fraction_analysis.py"
FIXTURES = REPOSITORY / "dev/simulation_results/permanent-zero-fraction/fixtures"
GOLDEN = FIXTURES / "golden/valid-completed-and-halted"
SPEC = importlib.util.spec_from_file_location("permanent_zero_fraction_analysis", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
ANALYSIS = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = ANALYSIS
SPEC.loader.exec_module(ANALYSIS)


class PermanentZeroFractionAnalysisTests(unittest.TestCase):
    def run_analysis(self, dataset: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(dataset)],
            cwd=REPOSITORY,
            text=True,
            capture_output=True,
            check=False,
        )

    def refresh_checksum(self, dataset: Path, relative_path: str) -> None:
        path = dataset / relative_path
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        checksum_path = dataset / "checksums.sha256"
        lines = []
        for line in checksum_path.read_text(encoding="utf-8").splitlines():
            _, recorded_path = line.split("  ", 1)
            lines.append(f"{digest}  {recorded_path}" if recorded_path == relative_path else line)
        checksum_path.write_text("\n".join(lines) + "\n", encoding="utf-8")

    def assert_fixture_checksums_match(self, dataset: Path) -> None:
        for line in (dataset / "checksums.sha256").read_text(encoding="utf-8").splitlines():
            expected, relative_path = line.split("  ", 1)
            self.assertEqual(hashlib.sha256((dataset / relative_path).read_bytes()).hexdigest(), expected)

    def test_valid_dataset_matches_golden_bytes_and_reruns_identically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = Path(temporary) / "valid-completed-and-halted"
            shutil.copytree(FIXTURES / "valid-completed-and-halted", dataset)
            first = self.run_analysis(dataset)
            self.assertEqual(first.returncode, 0, first.stderr)
            output = dataset / "derived/valid-completed-and-halted/zero-fraction-analysis"
            first_bytes = {path.name: path.read_bytes() for path in sorted(output.iterdir())}
            self.assertEqual(first_bytes["cells.csv"], (GOLDEN / "cells.csv").read_bytes())
            self.assertEqual(first_bytes["curve-q3.svg"], (GOLDEN / "curve-q3.svg").read_bytes())

            second = self.run_analysis(dataset)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual(first_bytes, {path.name: path.read_bytes() for path in sorted(output.iterdir())})

            table = first_bytes["cells.csv"].decode("utf-8")
            completed, halted = list(csv.DictReader(table.splitlines()))
            self.assertEqual(completed["dataset_permanent_verdict"], "accepted")
            self.assertEqual(completed["prior_zero_count"], "17116353")
            self.assertEqual(completed["prior_sample_count"], "43046721")
            self.assertEqual(completed["prior_point_estimate"], "0.397622690007")
            self.assertEqual(completed["prior_interval_kind"], "exact_no_sampling_uncertainty")
            self.assertEqual(completed["precision_classification"], "prior_exact")
            self.assertEqual(completed["prior_interval_relation"], "disjoint")
            self.assertEqual(completed["interval_excludes_published"], "true")
            self.assertEqual(halted["halt_reason"], "backend_unavailable")
            self.assertEqual(halted["pooled_sample_count"], "8")
            self.assertEqual(halted["pooled_permanent_zero_count"], "2")
            self.assertEqual(
                [halted[field] for field in list(halted)[8:]],
                [""] * 16,
            )

            svg = first_bytes["curve-q3.svg"].decode("utf-8")
            self.assertEqual(re.findall(r"<text[^>]*>([^<]*)</text>", svg), ["n", "p_hat"])
            self.assertEqual(
                re.findall(r"<title>([^<]*)</title>", svg),
                [
                    "campaign_id=valid-completed-and-halted; q=3; 95% Wilson error bars",
                    "n=4; N=10; p_hat=0.000000000000; "
                    "wilson_95=[0.000000000000,0.277532799863]",
                ],
            )
            self.assertIn('data-sample-count="10"', svg)
            self.assertIn('data-wilson-95-upper="0.277532799863"', svg)

    def test_checksum_mismatch_refuses_without_derived_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = Path(temporary) / "checksum-mismatch"
            shutil.copytree(FIXTURES / "checksum-mismatch", dataset)
            result = self.run_analysis(dataset)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("checksum mismatch", result.stderr)
            self.assertFalse((dataset / "derived").exists())

    def test_missing_terminal_state_refuses_after_checksum_verification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = Path(temporary) / "valid-completed-and-halted"
            shutil.copytree(FIXTURES / "valid-completed-and-halted", dataset)
            summary_path = dataset / "summary.csv"
            summary_path.write_text(
                summary_path.read_text(encoding="utf-8").replace(",completed,\n", ",,\n", 1),
                encoding="utf-8",
            )
            self.refresh_checksum(dataset, "summary.csv")

            result = self.run_analysis(dataset)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("lacks a valid terminal_state", result.stderr)
            self.assertFalse((dataset / "derived").exists())

    def test_unsupported_field_fixture_refuses_after_its_checksums_verify(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            dataset = Path(temporary) / "invalid-q"
            shutil.copytree(FIXTURES / "invalid-q", dataset)
            self.assert_fixture_checksums_match(dataset)
            result = self.run_analysis(dataset)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("unsupported field order q=9", result.stderr)
            self.assertFalse((dataset / "derived").exists())

    def test_symlinked_shard_parent_refuses_without_derived_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary)
            dataset = temporary_path / "valid-completed-and-halted"
            shutil.copytree(FIXTURES / "valid-completed-and-halted", dataset)
            outside = temporary_path / "outside-q3"
            shutil.move(str(dataset / "shards/q3"), outside)
            os.symlink(outside, dataset / "shards/q3", target_is_directory=True)

            result = self.run_analysis(dataset)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("symlinked component", result.stderr)
            self.assertFalse((dataset / "derived").exists())

    def test_wilson_transliteration_matches_versioned_source_interval(self) -> None:
        estimate, lower, upper = ANALYSIS.wilson_95(35_456_365_448, 100_000_000_000)
        self.assertAlmostEqual(estimate, 0.354563654480, places=12)
        self.assertAlmostEqual(lower, 0.354560689505, places=12)
        self.assertAlmostEqual(upper, 0.354566619467, places=12)


if __name__ == "__main__":
    unittest.main()
