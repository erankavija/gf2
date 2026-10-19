#!/usr/bin/env python3
"""Checks the confirmatory bound recorder on the sample family declaration."""

import copy
import importlib.util
import json
import shutil
import sys
import unittest
from pathlib import Path
from unittest import mock

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("record_confirmatory_bound",
                                              SURVEY / "record-confirmatory-bound.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)
ROOT = recorder.smoke.ROOT
SCRATCH = ROOT / "target/record-confirmatory-bound-test"
next_attempt = recorder.next_attempt


class BoundTest(unittest.TestCase):
    def setUp(self):
        shutil.rmtree(SCRATCH, ignore_errors=True)
        SCRATCH.mkdir(parents=True)
        self.addCleanup(shutil.rmtree, SCRATCH, ignore_errors=True)
        self.declaration = json.loads((SURVEY / "test_make_family_sample.json")
                                      .read_text(encoding="utf-8"))
        self.ledger = SCRATCH / "ledger.jsonl"
        self.declaration["family_wise"]["ledger_path"] = str(self.ledger.relative_to(ROOT))

    def record(self, declaration):
        path = SCRATCH / "declaration.json"
        path.write_text(json.dumps(declaration), encoding="utf-8")
        out = SCRATCH / "bound.json"
        recorder.record(path, out)
        return json.loads(out.read_text(encoding="utf-8"))

    def admits(self, document, m):
        read = document["read"]
        comparisons = document["ledger"]["prior_comparisons"] + m
        corrected = document["attempt_alpha"]["value"] / comparisons
        return read["bootstrap_resamples"]["value"] * corrected / 2 >= read["tail_draws"]["value"]

    def test_the_bound_is_the_largest_count_with_tail_support(self):
        document = self.record(self.declaration)
        self.assertEqual(self.ledger.read_bytes(), b"")
        bound = document["max_confirmatory_cells"]["value"]
        self.assertTrue(self.admits(document, bound))
        self.assertFalse(self.admits(document, bound + 1))
        self.assertEqual(document["attempt"]["value"], 1)

    def test_a_spent_attempt_moves_the_next_attempt(self):
        first = self.record(self.declaration)
        spent = {"sequence": 0, "predecessor": "0" * 64,
                 "family": self.declaration["family"]["id"], "campaign": "spent-attempt",
                 "addendum_sha256": "a" * 64, "comparisons": 1, "protocol_version": 4,
                 "candidates": ["b" * 64]}
        self.ledger.write_text(json.dumps(spent) + "\n", encoding="utf-8")
        second = self.record(dict(self.declaration, cells=[]))
        self.assertEqual(second["attempt"]["value"], first["attempt"]["value"] + 1)
        self.assertEqual(second["ledger"]["prior_comparisons"], spent["comparisons"])
        self.assertLess(second["attempt_alpha"]["value"], first["attempt_alpha"]["value"])
        bound = second["max_confirmatory_cells"]["value"]
        self.assertTrue(bound == 0 or self.admits(second, bound))
        self.assertFalse(self.admits(second, bound + 1))

    def test_committed_bounds_regenerate_byte_identically(self):
        for declaration in sorted((SURVEY / "families").glob("*.json")):
            ledger = json.loads(declaration.read_text(encoding="utf-8"))["family_wise"]["ledger_path"]
            committed = (ROOT / ledger).with_name("confirmatory-bound.json")
            if not committed.exists():
                continue
            with self.subTest(declaration.name):
                # The committed record read the ledger prefix of its own reservation count.
                count = json.loads(committed.read_text(encoding="utf-8"))["ledger"]["reservations"]
                prefix = SCRATCH / f"{declaration.stem}.ledger.jsonl"
                lines = (ROOT / ledger).read_text(encoding="utf-8").splitlines(keepends=True)
                prefix.write_text("".join(lines[:count]), encoding="utf-8")
                out = SCRATCH / f"{declaration.stem}.json"
                with mock.patch.object(recorder, "next_attempt",
                                       lambda _, family, prefix=prefix: next_attempt(prefix, family)):
                    recorder.record(declaration, out)
                self.assertEqual(out.read_bytes(), committed.read_bytes())

    def test_a_declaration_beyond_the_bound_is_refused(self):
        bound = self.record(self.declaration)["max_confirmatory_cells"]["value"]
        crowded = copy.deepcopy(self.declaration)
        crowded["cells"] = [dict(self.declaration["cells"][0], cell_id=f"cell-{index}")
                            for index in range(bound + 1)]
        with self.assertRaises(SystemExit):
            self.record(crowded)


    def test_holdout_cells_count_against_the_bound(self):
        bound = self.record(self.declaration)["max_confirmatory_cells"]["value"]
        cell = self.declaration["cells"][0]
        held = copy.deepcopy(self.declaration)
        held["cells"] = [dict(cell, cell_id="confirmatory")]
        held["holdout_declaration"] = {
            "inventory": "unread", "rationale": "unread",
            "cells": [dict(cell, cell_id=f"holdout-{index}") for index in range(bound - 1)]}
        self.assertEqual(self.record(held)["declared_cells"]["value"], bound)
        held["holdout_declaration"]["cells"].append(dict(cell, cell_id="holdout-beyond"))
        with self.assertRaises(SystemExit):
            self.record(held)

if __name__ == "__main__":
    unittest.main()
