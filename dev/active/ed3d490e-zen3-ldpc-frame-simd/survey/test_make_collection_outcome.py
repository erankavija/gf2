#!/usr/bin/env python3
"""Checks the per-cell outcome generator on a constructed receipt and the committed tables."""

import importlib.util
import json
import re
import sys
import tempfile
import unittest
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
        (self.directory / "inputs").mkdir(parents=True)
        self.cells = [
            ("gap", "comparator-gap", "not-worse", "not-material", "b-0", "c-0"),
            ("speed", "improvement", "improved", "pass", "b-1", "c-1"),
            ("guard", "non-regression", "regressed", "fail", "b-2", "c-2"),
        ]
        self.write("receipt.json", {"addendum": {"snapshot": "inputs/frozen.json"}})
        self.write("inputs/frozen.json", {"cells": [
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

    def test_an_outcome_without_a_declared_interpretation_is_refused(self):
        self.summary["cells"][1]["outcome"] = "unstable"
        self.write("acceptance-summary.json", self.summary)
        with self.assertRaises(KeyError):
            self.rows()


class CommittedTablesTest(unittest.TestCase):
    def test_committed_tables_regenerate_byte_identically(self):
        for path in outcome.smoke.listed("outcome.md"):
            committed = (ROOT / path).read_text(encoding="utf-8")
            title = TITLE.match(committed.split("\n", 1)[0])
            if not title:
                continue
            with self.subTest(path):
                directory = outcome.receipt_directory(title.group(1))
                self.assertEqual(outcome.render(directory, ROOT / path), committed)


if __name__ == "__main__":
    unittest.main()
