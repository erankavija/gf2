#!/usr/bin/env python3
"""Checks the per-cell outcome generator on a constructed receipt."""

import importlib.util
import json
import re
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("make_collection_outcome",
                                              SURVEY / "make-collection-outcome.py")
outcome = importlib.util.module_from_spec(spec)
spec.loader.exec_module(outcome)
ROOT = outcome.smoke.ROOT
TITLE = re.compile("^" + re.escape(outcome.TITLE).replace(r"\{\}", "([^`]+)") + "$")


class ConstructedReceiptTest(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.directory = Path(scratch.name) / "campaign"
        self.directory.mkdir(parents=True)
        root = mock.patch.object(outcome.smoke, "ROOT", self.directory.parent)
        root.start()
        self.addCleanup(root.stop)
        self.cells = [
            ("gap", "comparator-gap", "not-worse", "not-material", "b-0", "c-0"),
            ("speed", "improvement", "improved", "pass", "b-1", "c-1"),
            ("guard", "non-regression", "regressed", "fail", "b-2", "c-2"),
        ]
        self.write("receipt.json", {"addendum": {"path": "campaign/addendum.json"}})
        self.write("addendum.json", {"cells": [
            {"cell_id": cell, "objective": objective, "role": "confirmatory"}
            for cell, objective, *_ in reversed(self.cells)]})
        self.write("plan.json", {"cells": [
            {"cell_id": cell, "baseline_arm": baseline, "candidate_arm": candidate}
            for cell, _, _, _, baseline, candidate in self.cells]})
        self.summary = {"campaign_id": "campaign", "verdict": "accepted", "qualifies": False,
                        "findings": [], "cells": [
                            {"cell_id": cell, "core_arm": "single-core", "decision": decision,
                             "outcome": recorded}
                            for cell, _, decision, recorded, _, _ in self.cells]}
        self.write("acceptance-summary.json", self.summary)

    def write(self, name, document):
        (self.directory / name).write_text(json.dumps(document), encoding="utf-8")

    def rows(self):
        text = outcome.render(self.directory, self.directory.parent / "outcome.md")
        table = [line for line in text.splitlines() if line.startswith("| `")]
        return [[field.strip() for field in line.strip("|").split("|")] for line in table]

    def test_rows_follow_the_summary_with_addendum_objectives_and_plan_arms(self):
        rows = self.rows()
        self.assertEqual([row[0] for row in rows], [f"`{cell[0]}`" for cell in self.cells])
        for row, (_, objective, decision, recorded, baseline, candidate) in zip(rows, self.cells):
            self.assertEqual(row[2], objective)
            self.assertEqual(row[4:8], [f"`{baseline}`", f"`{candidate}`", f"`{decision}`",
                                        f"`{recorded}`"])
            self.assertEqual(row[8], outcome.INTERPRETATION[(objective, recorded)])

    def test_only_a_receipt_with_holdout_cells_gets_the_builds_table(self):
        text = outcome.render(self.directory, self.directory.parent / "outcome.md")
        self.assertNotIn("## Builds and holdout", text)
        self.write("addendum.json", {"cells": [
            {"cell_id": cell, "objective": objective,
             "role": "confirmatory" if cell == "gap" else "holdout"}
            for cell, objective, *_ in self.cells]})
        arms = {arm: {"build": f"build-{arm}"}
                for *_, baseline, candidate in self.cells for arm in (baseline, candidate)}
        plan = json.loads((self.directory / "plan.json").read_text(encoding="utf-8"))
        self.write("plan.json", {**plan, "arms": arms})
        text = outcome.render(self.directory, self.directory.parent / "outcome.md")
        section = text.split("## Builds and holdout\n", 1)[1]
        rows = [[field.strip() for field in line.strip("|").split("|")]
                for line in section.splitlines() if line.startswith("| `")]
        self.assertEqual([row[0] for row in rows], [f"`{cell[0]}`" for cell in self.cells])
        for row, (cell, _, _, recorded, baseline, candidate) in zip(rows, self.cells):
            self.assertEqual(row[2:5], [f"`build-{baseline}`", f"`build-{candidate}`",
                                        f"`{recorded}`"])
            if cell == "gap":
                self.assertEqual(row[5], "not a holdout cell")
            else:
                self.assertEqual(row[5], "holds" if recorded == "pass" else "does not hold")

    def test_an_outcome_without_a_declared_interpretation_is_refused(self):
        self.summary["cells"][1]["outcome"] = "unstable"
        self.write("acceptance-summary.json", self.summary)
        with self.assertRaises(KeyError):
            self.rows()


if __name__ == "__main__":
    unittest.main()
