#!/usr/bin/env python3
"""Check prepared families, arm identities and the window guard without timing.

Usage: python3 test_preparation.py
"""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import prepare  # noqa: E402


class Preparation(unittest.TestCase):
    def test_family_contracts(self):
        prepare.self_test()

    def test_core_build_bytes(self):
        builder = prepare.load("31727c57/build-core.py", "core_builder")
        self.assertEqual(builder.document(False),
                         prepare.read(prepare.HERE / "core/arm-builds.json"))

    def test_batch_fill_plan_repeats_each_confirmation_cell_on_the_probed_arm(self):
        import batch_fill
        confirmation = prepare.read(batch_fill.CONFIRMATION / "plan.json")
        plan = prepare.read(batch_fill.DIRECTORY / "plan.json")
        probed = prepare.read(batch_fill.BUILD_RECORD)["arms"][batch_fill.ARM]
        self.assertTrue(batch_fill.executable_matches())
        self.assertEqual({arm["executable"] for arm in plan["arms"].values()}, {probed["path"]})
        for cell in confirmation["cells"]:
            copies = [entry for entry in plan["cells"]
                      if entry["cell_id"].startswith(cell["cell_id"] + "-r")]
            with self.subTest(cell=cell["cell_id"]):
                self.assertEqual(len(copies), batch_fill.REPLICATES)
                for entry in copies:
                    self.assertEqual(entry["case"], cell["case"])
                    self.assertEqual(entry["baseline_arm"], entry["candidate_arm"])
                    self.assertEqual(plan["arms"][entry["candidate_arm"]]["environment"],
                                     confirmation["arms"][cell["candidate_arm"]]["environment"])

    def test_launchers_require_window(self):
        environment = dict(os.environ)
        environment.pop("GF2_BENCH_WINDOW", None)
        for launcher in ("core", "decoder", "decoder/batch-fill", "decoder/batch-fill/pilot-cells"):
            command = ["bash", str((prepare.HERE / launcher / "window.sh").relative_to(prepare.ROOT))]
            result = subprocess.run(command, env=environment, capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("timed runs require the overnight benchmark window", result.stderr)

    def test_pilot_batch_fill_plan_is_pilot_minus_confirmation(self):
        import batch_fill
        pilot = prepare.read(batch_fill.PILOT / "plan.json")
        confirmation = prepare.read(batch_fill.CONFIRMATION / "plan.json")
        confirmed = {cell["cell_id"] for cell in confirmation["cells"]}
        expected = [cell for cell in pilot["cells"] if cell["cell_id"] not in confirmed]
        plan = prepare.read(batch_fill.PILOT_DIRECTORY / "plan.json")
        self.assertEqual(len(expected), 6)
        self.assertEqual([cell["cell_id"] for cell in plan["cells"]],
                         [f"{cell['cell_id']}-r{replicate}"
                          for replicate in range(1, batch_fill.REPLICATES + 1)
                          for cell in expected])
        probed = prepare.read(batch_fill.BUILD_RECORD)["arms"][batch_fill.ARM]
        self.assertEqual({arm["executable"] for arm in plan["arms"].values()}, {probed["path"]})
        self.assertEqual(plan["campaign_id"], batch_fill.PILOT_CAMPAIGN)
        original = prepare.read(batch_fill.DIRECTORY / "plan.json")
        self.assertEqual(plan["max_cells_per_session"], len(expected))
        self.assertEqual(plan["timing_override"], original["timing_override"])
        addendum = prepare.read(batch_fill.PILOT_DIRECTORY / "addendum.json")
        self.assertEqual([cell["cell_id"] for cell in addendum["cells"]],
                         [cell["cell_id"] for cell in plan["cells"]])
        self.assertTrue(all(cell["role"] == "exploratory" for cell in addendum["cells"]))
        for replicate in range(batch_fill.REPLICATES):
            for source, cell in zip(expected, plan["cells"][replicate * len(expected):]):
                self.assertEqual(cell["case"], source["case"])
                self.assertEqual(cell["baseline_arm"], cell["candidate_arm"])
                self.assertIsNone(cell["pilot_pairs"])
                self.assertEqual(plan["arms"][cell["candidate_arm"]]["environment"],
                                 pilot["arms"][source["candidate_arm"]]["environment"])

    def test_pilot_batch_fill_selection_follows_the_source_plans(self):
        import batch_fill
        with tempfile.TemporaryDirectory(dir=prepare.HERE) as temporary:
            directory = Path(temporary)
            pilot, confirmation, output = (directory / name for name in ("pilot", "confirmation", "out"))
            source = prepare.read(batch_fill.PILOT / "plan.json")
            declared = prepare.read(batch_fill.PILOT / "addendum.json")
            for cell in source["cells"]:
                cell["cell_id"] = "other-" + cell["cell_id"]
            for cell in declared["cells"]:
                cell["cell_id"] = "other-" + cell["cell_id"]
            batch_fill.write(pilot / "plan.json", source)
            batch_fill.write(pilot / "addendum.json", declared)
            batch_fill.write(confirmation / "plan.json", {"cells": source["cells"][::2]})
            with patch.object(batch_fill, "PILOT", pilot), patch.object(batch_fill, "CONFIRMATION", confirmation):
                batch_fill.plan(output, "derived-test", "2026-11-01T00:00:00Z", pilot_cells=True)
            planned = prepare.read(output / "plan.json")
            self.assertEqual([cell["cell_id"] for cell in planned["cells"]],
                             [f"{cell['cell_id']}-r{replicate}"
                              for replicate in range(1, batch_fill.REPLICATES + 1)
                              for cell in source["cells"][1::2]])

    def test_confirmation_batch_fill_evidence_matches_committed_bytes(self):
        import batch_fill
        paths = [batch_fill.BUILD_RECORD, batch_fill.DIRECTORY / "plan.json",
                 batch_fill.DIRECTORY / "addendum.json"]
        paths += [path for path in (batch_fill.DIRECTORY / batch_fill.CAMPAIGN).rglob("*")
                  if path.is_file()]
        for path in paths:
            relative = str(path.relative_to(prepare.ROOT))
            with self.subTest(path=relative):
                self.assertEqual(path.read_bytes(), subprocess.check_output(["git", "show", f"HEAD:{relative}"]))

    def test_batch_fill_is_reported_separately(self):
        diagnostics = prepare.read(prepare.HERE / "decoder/untimed-diagnostics.json")
        expected = sum(len(cell["arms"]) for phase in ("pilot", "confirmation")
                       for cell in prepare.read(prepare.HERE / "decoder" /
                                                f"{phase}-untimed-smoke.json")["cells"])
        self.assertEqual(len(diagnostics), expected)
        for row in diagnostics:
            self.assertIn("batch_fill_ns", row["batch-decoder-diagnostics"]["conversion"])


if __name__ == "__main__":
    unittest.main()
