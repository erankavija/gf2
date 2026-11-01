#!/usr/bin/env python3
"""Behavioral fixtures for make-scorecard.py; run this file with python3 from the worktree root."""

import importlib.util
import json
import pathlib
import re
import sys
import unittest
from unittest import mock

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("make_scorecard", HERE / "make-scorecard.py")
scorecard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scorecard)
READABLE = "AGENTS.md"
INTEGRATED_ANCHORS = {
    "FI-1": "integrated-core-route", "FI-2": "integrated-decoder-route",
    "FI-3": "integrated-ldpc-aff3ct", "FI-4": "integrated-bch-encode"}


def row(cell="c", outcome="pass", estimate=2.0, threshold=1.25, primary=True,
        objective="comparator-gap"):
    return {"domain": "d", "family": "f", "cell": cell, "primary": primary, "outcome": outcome,
            "objective": objective, "decision": "improved", "receipt": "r",
            "interval": {"estimate": estimate}, "material_gap_threshold": threshold}


def entry(cell="c", **fields):
    return {"cells": [{"family": "f", "cell": cell}], "disposition": "preserved-experiment",
            "record": READABLE, **fields}


def three_figures(value):
    return f"{value:#.3g}".rstrip(".")


class GapRule(unittest.TestCase):
    def test_a_recorded_material_gap_is_a_gap(self):
        self.assertTrue(scorecard.is_gap(row(estimate=1.26)))

    def test_an_estimate_above_the_threshold_is_a_gap_and_keeps_its_outcome(self):
        cell = row(outcome="not-confirmatory", estimate=1.3)
        self.assertTrue(scorecard.is_gap(cell))
        self.assertIn("not-confirmatory", scorecard.gap_basis(cell))

    def test_an_estimate_at_or_below_the_threshold_without_pass_is_no_gap(self):
        self.assertFalse(scorecard.is_gap(row(outcome="not-material", estimate=1.25)))

    def test_cells_outside_the_primary_comparator_set_are_no_gaps(self):
        self.assertFalse(scorecard.is_gap(row(primary=False)))
        self.assertFalse(scorecard.is_gap(row(objective="non-regression")))


class Register(unittest.TestCase):
    def setUp(self):
        patcher = mock.patch.multiple(scorecard, stage=lambda receipt: {},
                                      search_budget=lambda record: {})
        patcher.start()
        self.addCleanup(patcher.stop)

    def refuses(self, rows, entries, phrase):
        with self.assertRaises(SystemExit) as caught:
            scorecard.register(rows, {"entries": entries})
        self.assertIn(phrase, str(caught.exception))

    def test_a_gap_cell_without_an_entry_refuses_the_write(self):
        self.refuses([row()], [], "refusing to write")

    def test_each_gap_cell_takes_one_entry(self):
        self.refuses([row()], [entry(), entry()], "twice")

    def test_an_entry_for_a_cell_recording_no_gap_is_refused(self):
        self.refuses([row(outcome="fail", estimate=0.5)], [entry()], "record no gap")

    def test_a_tracked_issue_needs_its_next_experiment(self):
        self.refuses([row()], [entry(disposition="tracked-issue", issue="1362381c")],
                     "next falsifiable experiment")

    def test_an_unknown_disposition_is_refused(self):
        self.refuses([row()], [entry(disposition="closed")], "is not one of")

    def test_a_pointer_must_resolve_to_an_existing_heading(self):
        self.refuses([row()], [entry(record=f"{READABLE}#no-such-heading")], "no heading")

    def test_a_complete_register_projects_the_disposition(self):
        projected = scorecard.register([row()], {"entries": [entry()]})
        self.assertEqual([item["disposition"] for item in projected], ["preserved-experiment"])


class TargetOutcome(unittest.TestCase):
    PLANNED = {"candidate_arm": "isal"}

    def outcome(self, decision, outcome="pass", status="measured"):
        return scorecard.target_outcome(True, ["isal"], self.PLANNED, {
            "cell_id": "c", "decision": decision, "outcome": outcome, "status": status})

    def test_the_decision_maps_to_the_target(self):
        self.assertEqual(self.outcome("regressed", "fail")["outcome"], "met")
        self.assertEqual(self.outcome("improved")["outcome"], "not met")
        self.assertEqual(self.outcome("not-worse", "not-material")["outcome"], "inconclusive")
        self.assertEqual(self.outcome("inconclusive", "inconclusive")["outcome"], "inconclusive")

    def test_not_confirmatory_never_reads_as_a_confirmed_loss(self):
        self.assertEqual(self.outcome("improved", "not-confirmatory")["outcome"], "inconclusive")

    def test_an_unmeasured_cell_is_unavailable(self):
        self.assertEqual(self.outcome(None, "unavailable", "unavailable")["outcome"], "unavailable")

    def test_a_cell_without_an_external_arm_has_no_target(self):
        self.assertIsNone(scorecard.target_outcome(False, [], self.PLANNED, {"cell_id": "c"}))

    def test_an_external_baseline_is_refused_rather_than_misread(self):
        with self.assertRaises(SystemExit):
            scorecard.target_outcome(True, ["isal"], {"candidate_arm": "gf2"}, {"cell_id": "c"})

    def test_an_unknown_external_arm_has_no_invented_citation(self):
        with self.assertRaises(SystemExit):
            scorecard.citation("unlisted-project")


class CommittedScorecard(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.record = scorecard.build()

    def test_the_committed_projection_is_current(self):
        self.assertEqual(json.loads(scorecard.OUTPUT_JSON.read_text()), json.loads(json.dumps(self.record)))
        self.assertEqual(scorecard.OUTPUT_MD.read_text(), scorecard.render(self.record))

    def test_one_row_per_workload_cell_and_none_besides(self):
        workload = scorecard.read_json(scorecard.WORKLOAD)["cells"]
        self.assertEqual(sorted((c["family"], c["cell"]) for c in workload),
                         sorted((c["family"], c["cell"]) for c in self.record["cells"]))

    def test_every_primary_cell_states_a_cited_target(self):
        for cell in self.record["cells"]:
            if cell["primary"]:
                self.assertTrue(cell["target"]["outcome"] and cell["target"]["citation"], cell["cell"])
            else:
                self.assertIsNone(cell["target"], cell["cell"])

    def test_every_consumed_row_carries_a_drift_class(self):
        for cell in self.record["cells"]:
            self.assertTrue(cell["drift_class"], cell["cell"])

    def test_every_integrated_family_is_selected(self):
        names = {cell["integrated_family"] for cell in self.record["cells"]} - {None}
        self.assertEqual(names, set(INTEGRATED_ANCHORS))

    def test_the_report_names_no_winner(self):
        self.assertNotRegex(scorecard.render(self.record).lower(), r"\bwinner\b|\baggregate\b")

    def test_claims_state_each_confirmatory_integrated_cell_as_measured(self):
        page = (scorecard.ROOT / "docs/reference/performance-evidence.md").read_text()
        for name, anchor in INTEGRATED_ANCHORS.items():
            section = page.split(f'<a id="{anchor}"></a>')[1].split('<a id="')[0]
            for cell in self.record["cells"]:
                if cell["integrated_family"] == name and cell["role"] == "confirmatory":
                    bounds = cell["interval"]
                    figure = (f"{three_figures(bounds['estimate'])}\\ "
                              f"[{three_figures(bounds['lower'])}, {three_figures(bounds['upper'])}]")
                    self.assertIn(figure, section, cell["cell"])
                    self.assertIn(cell["receipt"], section, cell["cell"])
                elif cell["integrated_family"] == name:
                    self.assertNotIn(f"`{cell['cell']}`", section)


if __name__ == "__main__":
    unittest.main()
