#!/usr/bin/env python3
"""Checks the outcome-table generator on constructed campaigns."""

import importlib.util
import json
import statistics
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("make_outcome_tables",
                                              SURVEY / "make-outcome-tables.py")
tables = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tables)
ROOT = tables.smoke.ROOT
PAIRS = 6
QUALITY = {"frames": 8, "frame_errors": 0, "fer": 0.0, "fer_interval": [0.0, 0.3],
           "interval_method": "constructed",
           "iterations": {"mean": 3.0, "p50": 3, "p90": 4, "max": 5},
           "settings": {"precision": "f32", "schedule": "flooding",
                        "normalization": {"kind": "normalized-min-sum", "factor": 0.75},
                        "stopping": {"kind": "syndrome"}, "iteration_cap": 10}}
MATCHED, FASTEST, BEFORE_AFTER, SELECTOR = tables.PARTS


def write(path, document):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document), encoding="utf-8")


def constructed(directory, cells, arms, purpose="decoder-family"):
    """A campaign directory of a family of `purpose` whose cells are (cell_id, arm_kind, baseline,
    candidate) and whose arms map a name to (build, reports stage probes)."""
    def execution(arm, index):
        return {"arm": arm, "ns_per_call": 1000.0 + index, "workers_observed": 1,
                "conversion": {"batch_fill_ns": 10 + index}}

    write(directory / "receipt.json", {
        "addendum": {"path": f"{directory.name}/addendum.json"},
        "settings": {"pilot_min_pairs": PAIRS, "confirmatory_pairs": PAIRS},
        "arms": {name: {"build": build, "executable_sha256": name, "environment": {}}
                 for name, (build, _) in arms.items()},
        "cells": [{"cell_id": cell, "role": "exploratory", "decoder_quality": {"baseline": QUALITY, "candidate": QUALITY},
                   "pairs": [{"baseline": execution(baseline, index),
                              "candidate": execution(candidate, index)} for index in range(PAIRS)]}
                  for cell, _, baseline, candidate in cells]})
    write(directory / "addendum.json", {"family": {"purpose": purpose}, "cells": [
        {"cell_id": cell, "objective": "improvement", "role": "exploratory",
         "decoder": {"arm_kind": kind}} for cell, kind, _, _ in cells]})
    write(directory / "plan.json", {
        "arms": {name: {"build": build} for name, (build, _) in arms.items()},
        "cells": [{"cell_id": cell, "baseline_arm": baseline, "candidate_arm": candidate,
                   "case": {"code": "code", "batch_size": 1}, "pilot_pairs": PAIRS}
                  for cell, _, baseline, candidate in cells]})
    write(directory / "acceptance-summary.json", {
        "campaign_id": directory.name, "label": "pilot", "verdict": "accepted",
        "qualifies": False, "findings": [], "sessions": 1, "family": {"family_id": "family"},
        "cells": [{"cell_id": cell, "core_arm": "single-core", "resolved_cpus": [0],
                   "decision": None, "outcome": "pilot",
                   "interval": {"estimate": 1.0, "lower": 0.9, "upper": 1.1, "confidence": 0.95,
                                "pairs": PAIRS}} for cell, _, _, _ in cells]})
    events = []
    for cell, _, baseline, candidate in cells:
        events.append({"event": "cell-start", "case": {"cell_id": cell}})
        for arm in (baseline, candidate):
            report = {"kind": tables.DECODER_REPORT, "workspace_bytes": [64],
                      "conversion": {"batch_fill_ns": 0}}
            if arms[arm][1]:
                report["stage_probes"] = 3
            events.append({"event": "child-diagnostic", "case": {"cell_id": cell, "arm": arm},
                           "details": {"stderr": json.dumps(report) + "\n"}})
        events.append({"event": "checkpoint-accepted", "case": {"cell_id": cell}})
        events.append({"event": "cell-complete", "case": {"cell_id": cell},
                       "details": {"status": "measured", "pairs": PAIRS}})
    events.append({"event": "complete"})
    (directory / "execution.log").write_text(
        "".join(json.dumps(event) + "\n" for event in events), encoding="utf-8")
    return directory


class ConstructedCampaignTest(unittest.TestCase):
    ARMS = {"conservative": ("conservative-portable", True), "tuned": ("tuned-portable", False),
            "native": ("native", False), "external": ("external", False)}
    CELLS = [("tuned-cell", MATCHED, "conservative", "tuned"),
             ("native-cell", MATCHED, "tuned", "native"),
             ("matched-cell", MATCHED, "native", "external"),
             ("fastest-cell", FASTEST, "native", "external")]
    PARTED = {"tuned-cell": BEFORE_AFTER, "native-cell": BEFORE_AFTER,
              "matched-cell": MATCHED, "fastest-cell": FASTEST}
    # One record row per arm, each measured at a batch no constructed cell decodes.
    MEMORY = {"method": {"quantity": "peak", "scope": "process", "position": "validation"},
              "path": SURVEY / "peak-memory.json",
              "rows": [{"arm": f"record-{name}", "batch": 97, "declared_by": "record scope",
                        "identity": (name, "{}", "code"),
                        "samples": [987000 + 100 * index + launch for launch in range(20)]}
                       for index, name in enumerate(("conservative", "native", "external"))]}

    def setUp(self):
        interval = mock.patch.object(tables, "median_interval", side_effect=lambda values:
                                     (statistics.median(values), min(values), max(values), 1.0))
        interval.start()
        self.addCleanup(interval.stop)
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.scratch = Path(scratch.name)
        self.directory = constructed(self.scratch / "campaign", self.CELLS, self.ARMS)

    def part(self, kind, cells):
        return "\n".join(tables.part(self.directory / "tables.md", kind, cells, self.MEMORY))

    def rows(self, text, heading):
        section = text.split(f"### {heading}\n", 1)[1].split("\n### ", 1)[0]
        return [[field.strip() for field in line.strip("|").split("|")]
                for line in section.splitlines() if line.startswith("| `")]

    def test_a_log_that_does_not_verify_stops_the_campaign(self):
        tables.campaign(self.directory, root=self.scratch)
        log = self.directory / "execution.log"
        lines = log.read_text(encoding="utf-8").splitlines(keepends=True)
        log.write_text("".join(line for line in lines if '"cell-complete"' not in line
                               or '"native-cell"' not in line), encoding="utf-8")
        with self.assertRaises(SystemExit):
            tables.campaign(self.directory, root=self.scratch)

    def test_matched_and_fastest_compatible_cells_never_share_a_part(self):
        cells = tables.campaign(self.directory, root=self.scratch)["cells"]
        for kind in tables.PARTS:
            text = self.part(kind, [cell for cell in cells if cell["part"] == kind])
            for identifier, _, _, _ in self.CELLS:
                self.assertEqual(f"`{identifier}`" in text, self.PARTED[identifier] == kind,
                                 (kind, identifier))

    def test_a_selector_calibration_family_has_a_part_of_its_own(self):
        directory = constructed(self.scratch / "holdout", self.CELLS, self.ARMS,
                                tables.SELECTOR_CALIBRATION)
        self.assertEqual({cell["part"] for cell in tables.campaign(directory, root=self.scratch)["cells"]},
                         {SELECTOR})

    def test_peak_memory_appears_only_in_its_own_configuration(self):
        cells = tables.campaign(self.directory, root=self.scratch)["cells"]
        for kind in tables.PARTS:
            selected = [cell for cell in cells if cell["part"] == kind]
            if not selected:
                continue
            text = self.part(kind, selected)
            for row in self.rows(text, "Arm measurements"):
                self.assertFalse(any("9870" in field or "9871" in field or "9872" in field
                                     for field in row), row)
            arms = {side["arm"] for cell in selected for side in cell["sides"]}
            for row in self.rows(text, "Peak memory"):
                record = next(entry for entry in self.MEMORY["rows"]
                              if f"`{entry['arm']}`" == row[0])
                self.assertEqual(row[2:4], [str(record["batch"]), record["declared_by"]])
                self.assertIn(f"`{record['identity'][0]}`", row[7])
                self.assertIn(record["identity"][0], arms)

    def test_each_build_has_rows_of_its_own(self):
        cells = [cell for cell in tables.campaign(self.directory, root=self.scratch)["cells"]
                 if cell["part"] == BEFORE_AFTER]
        rows = self.rows(self.part(BEFORE_AFTER, cells), "Arm measurements")
        builds = {build for build, _ in self.ARMS.values()}
        seen = [{build for build in builds if f"`{build}`" in row} for row in rows]
        self.assertTrue(all(len(found) == 1 for found in seen))
        self.assertEqual(set().union(*seen), {"conservative-portable", "tuned-portable", "native"})

    def test_a_quantity_an_arm_does_not_report_is_stated_unmeasured(self):
        cells = [cell for cell in tables.campaign(self.directory, root=self.scratch)["cells"]
                 if cell["part"] == BEFORE_AFTER]
        text = self.part(BEFORE_AFTER, cells)
        for row in self.rows(text, "Arm measurements"):
            probed = row[4] == "`conservative-portable`"
            self.assertEqual(row[12][0].isdigit(), probed, row)
        self.assertIn("without a row of the record: `tuned`", text)




if __name__ == "__main__":
    unittest.main()
