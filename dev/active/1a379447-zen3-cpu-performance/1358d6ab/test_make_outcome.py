#!/usr/bin/env python3
"""Checks the outcome projection against the committed evidence (jit:1358d6ab).

Usage (from the worktree root): python3 -B test_make_outcome.py
"""

import importlib.util
import json
import pathlib
import sys
import unittest

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("make_outcome", HERE / "make-outcome.py")
outcome = importlib.util.module_from_spec(spec)
spec.loader.exec_module(outcome)


class OutcomeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = outcome.render()
        cls.rows = [line for line in cls.text.splitlines() if line.startswith("| `")]

    def test_committed_document_is_the_projection(self):
        self.assertEqual(outcome.OUTPUT.read_text(encoding="utf-8"), self.text)

    def test_every_confirmatory_cell_carries_its_recorded_outcome(self):
        for family in outcome.FAMILIES:
            summary = outcome.campaign(family["campaign"] + "-confirmation")["summary"]
            for cell in summary["cells"]:
                row = [line for line in self.rows if line.startswith(f"| `{cell['cell_id']}` |")
                       and "confirmatory" in line]
                self.assertEqual(len(row), 1, cell["cell_id"])
                self.assertIn(f"`{cell['outcome']}`", row[0])
                self.assertIn(f"{cell['interval']['lower']:.4f}", row[0])

    def test_consumed_cells_are_the_primary_cells_outside_the_families(self):
        workload = outcome.read(outcome.live("1362381c/workload-set.json"))
        integrated = {family["family"] for family in outcome.FAMILIES}
        expected = sorted((cell["family"], cell["cell"]) for cell in workload["cells"]
                          if cell["primary"] and cell["family"] not in integrated)
        section = self.text.split("## Consumed primary cells", 1)[1]
        listed = sorted(tuple(field.strip(" `") for field in line.split("|")[2:4])
                        for line in section.splitlines() if line.count("|") > 6
                        and not line.startswith(("| Domain", "|---")))
        self.assertEqual(listed, expected)

    def test_measured_external_workloads_are_bit_exact(self):
        section = self.text.split("### Bit-exact equivalence", 1)[1].split("##", 1)[0]
        measured = [line for line in section.splitlines()
                    if line.startswith("| `") and not line.endswith("not measured |")]
        self.assertTrue(measured)
        for line in measured:
            self.assertIn("| `equal` |", line)
            for count in line.rsplit("|", 2)[1].split(";"):
                done, total = count.split()[-1].split("/")
                self.assertEqual(done, total, line)

    def test_margin_changes_name_each_replaced_margin(self):
        effect = {name: 1.1 for name in outcome.RATIONALE}
        effect.update({key: "reason" for key in outcome.RATIONALE.values()})
        replaced = json.loads(json.dumps(effect))
        replaced["equivalence_margin"] = 1.14
        changes = outcome.margin_changes({"addendum": {"effect": effect}},
                                         {"addendum": {"effect": replaced}})
        self.assertEqual([change[:3] for change in changes], [("equivalence_margin", 1.1, 1.14)])


if __name__ == "__main__":
    unittest.main()
