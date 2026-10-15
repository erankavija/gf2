#!/usr/bin/env python3
"""Check the family smoke runner over the sample declaration of the family generator.

The real arms live in the build tree, so the runner is driven with the sample's projected plan
whose arms are the runner package's synthetic workload.
"""

import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SURVEY = Path(__file__).resolve().parent
SAMPLE = SURVEY / "test_make_family_sample.json"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("smoke_family", SURVEY / "smoke-family.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
smoke, generator = runner.smoke, runner.generator
SCRATCH = smoke.ROOT / "target/smoke-family-test"
FAMILY = json.loads(SAMPLE.read_text(encoding="utf-8"))["family"]["id"]


def workload():
    binary, _ = smoke.build_runner("ab-smoke-workload")
    return Path(binary).with_name("ab-smoke-workload")


def fake_arms(plan_path, executable):
    """Rewrites the projected plan to run the synthetic workload in every arm."""
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    for arm in plan["arms"].values():
        arm["executable"], arm["environment"] = str(executable), {}
    for index, cell in enumerate(plan["cells"]):
        cell["case"] = {"words": 64, "seed": index}
    plan_path.write_text(json.dumps(plan), encoding="utf-8")


class SmokeFamilyTest(unittest.TestCase):
    def setUp(self):
        SCRATCH.mkdir(parents=True, exist_ok=True)
        self.directory = Path(tempfile.mkdtemp(dir=SCRATCH))
        self.addCleanup(shutil.rmtree, self.directory, ignore_errors=True)
        self.addCleanup(shutil.rmtree, SCRATCH / "family", ignore_errors=True)

    def project(self):
        plan = runner.project(SAMPLE, self.directory)
        fake_arms(plan, workload())
        return plan

    def test_the_sample_plan_passes_check_and_smoke_with_one_dispatch_per_arm(self):
        plan = self.project()
        record = runner.smoke_plan(plan, self.directory, self.directory / "smoke.json")
        cells = json.loads(plan.read_text(encoding="utf-8"))["cells"]
        self.assertEqual([cell["cell_id"] for cell in record["cells"]],
                         [cell["cell_id"] for cell in cells])
        for cell in record["cells"]:
            self.assertEqual(len(cell["arms"]), 2)
            self.assertEqual([entry["windows"] for entry in cell["arms"]], [0, 0])
        self.assertEqual(runner.timed(record), [])
        self.assertFalse(any(self.directory.rglob("receipt.json")))
        family = json.loads(Path(plan.parent / "addendum.json").read_text(encoding="utf-8"))
        self.assertFalse((smoke.ROOT / family["family_wise"]["ledger_path"]).exists())

    def test_an_arm_that_reports_a_timing_window_fails_the_smoke(self):
        plan = self.project()
        opened = {"cells": [{"cell_id": "c", "arms": [{"arm": "a", "windows": 0},
                                                      {"arm": "b", "windows": 1}]}]}
        self.assertEqual(runner.timed(opened), ["c/b"])
        with mock.patch.object(runner.validation, "staged_smoke",
                               lambda _r, _e, _p, _s, record: record.write_text(
                                   json.dumps(opened), encoding="utf-8")):
            with self.assertRaises(SystemExit) as raised:
                runner.smoke_plan(plan, self.directory, self.directory / "smoke.json")
        self.assertIn("c/b", str(raised.exception))

    def test_a_family_declaration_is_projected_and_its_record_written_where_named(self):
        out = self.directory / "named" / "record.jsonl"
        real = runner.smoke_plan

        def driven(plan, scratch, record):
            fake_arms(plan, workload())
            return real(plan, scratch, record)

        with mock.patch.object(runner, "smoke_plan", driven), \
                mock.patch.object(runner, "SCRATCH", SCRATCH.relative_to(smoke.ROOT) / "family"):
            written = runner.smoke_family(SAMPLE, out)
        self.assertEqual(written, out)
        (row,) = [json.loads(line) for line in out.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(row["family"], FAMILY)
        self.assertEqual(runner.timed(row["smoke"]), [])
        self.assertFalse((SCRATCH / "family" / FAMILY).exists(), "the stage was not removed")


if __name__ == "__main__":
    unittest.main()
