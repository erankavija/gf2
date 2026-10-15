#!/usr/bin/env python3
"""Checks the confirmatory bound recorder on the sample family declaration."""

import copy
import importlib.util
import json
import shutil
import sys
import unittest
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("record_confirmatory_bound",
                                              SURVEY / "record-confirmatory-bound.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)
ROOT = recorder.smoke.ROOT
SCRATCH = ROOT / "target/record-confirmatory-bound-test"


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
        corrected = document["attempt_alpha"]["value"] / m
        return read["bootstrap_resamples"]["value"] * corrected / 2 >= read["tail_draws"]["value"]

    def test_the_bound_is_the_largest_count_with_tail_support(self):
        document = self.record(self.declaration)
        self.assertEqual(self.ledger.read_bytes(), b"")
        bound = document["max_confirmatory_cells"]["value"]
        self.assertTrue(self.admits(document, bound))
        self.assertFalse(self.admits(document, bound + 1))
        self.assertEqual(document["attempt"]["value"], 1)

    def test_a_spending_reservation_moves_the_next_attempt(self):
        first = self.record(self.declaration)
        self.ledger.write_text(json.dumps({"comparisons": 1}) + "\n" +
                               json.dumps({"comparisons": 0}) + "\n", encoding="utf-8")
        second = self.record(self.declaration)
        self.assertEqual(second["attempt"]["value"], first["attempt"]["value"] + 1)
        self.assertLess(second["attempt_alpha"]["value"], first["attempt_alpha"]["value"])
        self.assertLessEqual(second["max_confirmatory_cells"]["value"],
                             first["max_confirmatory_cells"]["value"])

    def test_every_cited_line_is_in_its_source(self):
        document = self.record(self.declaration)
        for cited in document["cited"]:
            self.assertIn(cited["verbatim"], (ROOT / cited["path"]).read_text(encoding="utf-8"))

    def test_a_declaration_beyond_the_bound_is_refused(self):
        bound = self.record(self.declaration)["max_confirmatory_cells"]["value"]
        crowded = copy.deepcopy(self.declaration)
        crowded["cells"] = [dict(self.declaration["cells"][0], cell_id=f"cell-{index}")
                            for index in range(bound + 1)]
        with self.assertRaises(SystemExit):
            self.record(crowded)


if __name__ == "__main__":
    unittest.main()
