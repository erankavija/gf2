#!/usr/bin/env python3
"""Check the outcome projection and preserve unavailable and adverse evidence."""

import copy
from pathlib import Path
import shutil
import tempfile
import unittest

import make_outcome as outcome


class Outcome(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.campaigns = outcome.campaigns()
        cls.fill = outcome.batch_fill_record()

    def test_document_regenerates(self):
        self.assertEqual(outcome.render(self.campaigns, self.fill), outcome.OUTPUT.read_text())

    def test_every_recorded_cell_keeps_its_interval_pairs_and_verdict(self):
        for (route, stage), campaign in self.campaigns.items():
            rows = outcome.result_rows(campaign)
            self.assertEqual(len(rows), len(campaign["summary"]["cells"]))
            for row, cell in zip(rows, campaign["summary"]["cells"]):
                with self.subTest(route=route, stage=stage, cell=cell["cell_id"]):
                    text = " | ".join(row)
                    self.assertIn(f"`{cell['cell_id']}`", text)
                    self.assertIn(str(cell["pairs"]), row)
                    self.assertIn(f"{cell['interval']['estimate']:.4f}", row)
                    self.assertIn(f"[{cell['interval']['lower']:.4f}, "
                                  f"{cell['interval']['upper']:.4f}]", row)
                    self.assertIn(f"`{cell['decision']}`", row)
                    self.assertIn(f"`{cell['outcome']}`", row)
                    self.assertIn(f"{cell['margins']['improvement']:g} / "
                                  f"{cell['margins']['equivalence']:g}", row)

    def test_adverse_outcomes_are_projected_without_reclassification(self):
        campaign = copy.deepcopy(self.campaigns["core", "confirmation"])
        for decision, verdict in (("regressed", "fail"), ("inconclusive", "inconclusive")):
            cell = campaign["cells"][0]["summary"]
            cell.update(decision=decision, outcome=verdict)
            row = outcome.result_rows(campaign)[0]
            self.assertIn(f"`{decision}`", row)
            self.assertIn(f"`{verdict}`", row)

    def test_unprobed_zero_is_not_a_latency_measurement(self):
        for stage in ("pilot", "confirmation"):
            campaign = self.campaigns["decoder", stage]
            rows = outcome.measurement_rows(campaign)
            self.assertEqual(len(rows), 2 * len(campaign["cells"]))
            for row, (cell, side) in zip(rows, outcome.tables.arm_rows(campaign["cells"])):
                with self.subTest(stage=stage, cell=cell["summary"]["cell_id"], side=side["side"]):
                    self.assertIn(str(len(side["times"])), row)
                    if side["batch_fill"] is None:
                        self.assertEqual(row[-1], "unmeasured; recorded 0")
                    else:
                        self.assertEqual(row[-1], outcome.tables.spread(side["batch_fill"])[0])

    def test_incomplete_batch_fill_stage_yields_no_record(self):
        with tempfile.TemporaryDirectory() as empty:
            self.assertIsNone(outcome.batch_fill_record(Path(empty)))

    def test_committed_batch_fill_record_is_complete_and_probed(self):
        confirmation = self.campaigns["decoder", "confirmation"]
        build = outcome.batch_fill.read(outcome.batch_fill.BUILD_RECORD)
        probed = build["arms"][outcome.batch_fill.ARM]["executable_sha256"]
        self.assertEqual(set(self.fill["cells"]),
                         {cell["cell_id"] for cell in confirmation["plan"]["cells"]})
        for identifier, cell in self.fill["cells"].items():
            with self.subTest(cell=identifier):
                self.assertEqual(len(cell["fill"]), 2 * outcome.batch_fill.REPLICATES)
                self.assertTrue(all(value > 0 for value in cell["fill"]))
                self.assertEqual(cell["executables"], {probed})
                self.assertNotIn(0, cell["probes"])
        text = outcome.render(self.campaigns, self.fill)
        self.assertIn("### Tuned-arm batch fill", text)
        self.assertNotIn("evidence gap", text)

    def test_pilot_configurations_are_confirmed_or_exploratory_from_the_receipts(self):
        pilot = self.campaigns["decoder", "pilot"]
        confirmation = self.campaigns["decoder", "confirmation"]
        rows, counts, outside = outcome.pilot_coverage(pilot, confirmation)
        self.assertEqual(len(rows), len(pilot["cells"]))
        confirmed = {cell["summary"]["cell_id"] for cell in confirmation["cells"]}
        for row, cell in zip(rows, pilot["cells"]):
            identifier = cell["summary"]["cell_id"]
            with self.subTest(cell=identifier):
                self.assertEqual(row[-1].startswith("confirmed"), identifier in confirmed)
                if identifier not in confirmed:
                    self.assertEqual(row[-1], "exploratory pilot evidence")
        # REQ-02: a confirmed cell at one core, six and twelve physical cores, 24 logical CPUs.
        self.assertTrue({1, 6, 12, 24} <= set(counts))
        self.assertEqual(set(outside), confirmed - {cell["summary"]["cell_id"]
                                                    for cell in pilot["cells"]})
        text = outcome.render(self.campaigns, self.fill)
        self.assertIn("### Pilot configurations and confirmation", text)
        for cell in pilot["cells"]:
            self.assertIn(f"`{cell['summary']['cell_id']}`", text)

    def test_coverage_marks_a_dropped_confirmation_cell_exploratory(self):
        confirmation = copy.deepcopy(self.campaigns["decoder", "confirmation"])
        dropped = confirmation["cells"].pop(0)["summary"]["cell_id"]
        rows, counts, _ = outcome.pilot_coverage(self.campaigns["decoder", "pilot"], confirmation)
        row = next(row for row in rows if row[0] == f"`{dropped}`")
        self.assertEqual(row[-1], "exploratory pilot evidence")

    def test_batch_fill_record_reports_every_confirmation_cell(self):
        # The untimed smoke's log has the record's shape; its values are no measurement.
        with tempfile.TemporaryDirectory() as stage:
            stage = Path(stage)
            shutil.copy(outcome.batch_fill.DIRECTORY / "untimed-execution.log",
                        stage / "execution.log")
            record = outcome.batch_fill_record(stage)
        confirmation = self.campaigns["decoder", "confirmation"]
        rows = outcome.batch_fill_rows(record, confirmation)
        self.assertEqual(len(rows), len(confirmation["plan"]["cells"]))
        build = outcome.batch_fill.read(outcome.batch_fill.BUILD_RECORD)
        probed = build["arms"][outcome.batch_fill.ARM]["executable_sha256"]
        for row, planned in zip(rows, confirmation["plan"]["cells"]):
            cell = record["cells"][planned["cell_id"]]
            with self.subTest(cell=planned["cell_id"]):
                self.assertTrue(all(value > 0 for value in cell["fill"]))
                self.assertEqual(row[5], str(2 * outcome.batch_fill.REPLICATES))
                self.assertEqual(row[8], outcome.tables.spread(cell["fill"])[0])
                self.assertEqual(row[-1], f"`{probed}`")
        text = outcome.render(self.campaigns, record)
        self.assertIn("### Tuned-arm batch fill", text)
        self.assertIn(build["confirmed_arm"]["executable_sha256"], text)
        self.assertIn(outcome.batch_fill.RULING, text)

    def test_confirmation_count_comes_from_roles(self):
        for route in ("core", "decoder"):
            campaign = self.campaigns[route, "confirmation"]
            confirmatory, holdout = outcome.counts(campaign)
            self.assertEqual(confirmatory + holdout, campaign["summary"]["family"]["comparisons"])
            self.assertLessEqual(confirmatory + holdout, 6)


if __name__ == "__main__":
    unittest.main()
