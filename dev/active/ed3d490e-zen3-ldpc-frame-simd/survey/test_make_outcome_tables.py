#!/usr/bin/env python3
"""Checks the outcome-table generator on constructed campaigns and the committed tables."""

import importlib.util
import json
import sys
import tempfile
import unittest
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
MATCHED, FASTEST = tables.PARTS


def write(path, document):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document), encoding="utf-8")


def constructed(directory, cells, arms):
    """A campaign directory whose cells are (cell_id, arm_kind, baseline, candidate) and whose arms
    map a name to (build, reports stage probes)."""
    def execution(arm, index):
        return {"arm": arm, "ns_per_call": 1000.0 + index, "workers_observed": 1,
                "conversion": {"batch_fill_ns": 10 + index}}

    write(directory / "receipt.json", {
        "addendum": {"snapshot": "inputs/frozen.json"},
        "settings": {"pilot_min_pairs": PAIRS, "confirmatory_pairs": PAIRS},
        "arms": {name: {"build": build, "executable_sha256": name, "environment": {}}
                 for name, (build, _) in arms.items()},
        "cells": [{"cell_id": cell, "decoder_quality": {"baseline": QUALITY, "candidate": QUALITY},
                   "pairs": [{"baseline": execution(baseline, index),
                              "candidate": execution(candidate, index)} for index in range(PAIRS)]}
                  for cell, _, baseline, candidate in cells]})
    write(directory / "inputs" / "frozen.json", {"cells": [
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
             ("fastest-cell", FASTEST, "native", "external")]
    MEMORY = {"rows": {}, "method": {"quantity": "peak", "scope": "process"},
              "path": SURVEY / "peak-memory.json"}

    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.directory = constructed(Path(scratch.name) / "campaign", self.CELLS, self.ARMS)

    def part(self, kind, cells):
        return "\n".join(tables.part(self.directory / "tables.md", kind, cells, self.MEMORY))

    def measurement_rows(self, text):
        section = text.split("### Arm measurements\n", 1)[1].split("\n### ", 1)[0]
        return [[field.strip() for field in line.strip("|").split("|")]
                for line in section.splitlines() if line.startswith("| `")]

    def test_a_log_that_does_not_verify_stops_the_campaign(self):
        tables.campaign(self.directory)
        log = self.directory / "execution.log"
        lines = log.read_text(encoding="utf-8").splitlines(keepends=True)
        log.write_text("".join(line for line in lines if '"cell-complete"' not in line
                               or '"native-cell"' not in line), encoding="utf-8")
        with self.assertRaises(SystemExit):
            tables.campaign(self.directory)

    def test_matched_and_fastest_compatible_cells_never_share_a_part(self):
        cells = tables.campaign(self.directory)["cells"]
        for kind in tables.PARTS:
            text = self.part(kind, [cell for cell in cells if cell["part"] == kind])
            for identifier, cell_kind, _, _ in self.CELLS:
                self.assertEqual(f"`{identifier}`" in text, cell_kind == kind, (kind, identifier))

    def test_each_build_has_rows_of_its_own(self):
        cells = [cell for cell in tables.campaign(self.directory)["cells"]
                 if cell["part"] == MATCHED]
        rows = self.measurement_rows(self.part(MATCHED, cells))
        builds = {build for build, _ in self.ARMS.values()}
        seen = [{build for build in builds if f"`{build}`" in row} for row in rows]
        self.assertTrue(all(len(found) == 1 for found in seen))
        self.assertEqual(set().union(*seen), {"conservative-portable", "tuned-portable", "native"})

    def test_a_quantity_an_arm_does_not_report_is_stated_unmeasured(self):
        cells = [cell for cell in tables.campaign(self.directory)["cells"]
                 if cell["part"] == MATCHED]
        rows = self.measurement_rows(self.part(MATCHED, cells))
        for row in rows:
            fill, memory = row[12], row[14]
            probed = row[4] == "`conservative-portable`"
            self.assertEqual(fill[0].isdigit(), probed, row)
            self.assertFalse(memory[0].isdigit(), row)


class SeriesLogTest(unittest.TestCase):
    def test_a_series_without_its_terminal_record_stops(self):
        summarizer = tables.summarizer(tables.profile_summary())
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "rep-01").mkdir()
            sessions = "rep-01 start 0 identity load=[0]\nrep-01 done 0 load=[0]\n"
            for log, verifies in ((sessions + "series done 0 sessions=1\n", True),
                                  (sessions, False),
                                  (sessions + "series done 0 sessions=2\n", False)):
                (root / "repetitions.log").write_text(log, encoding="utf-8")
                if verifies:
                    self.assertEqual(len(summarizer.verified_sessions(root)), 1)
                else:
                    with self.assertRaises(SystemExit):
                        summarizer.verified_sessions(root)


class CommittedTablesTest(unittest.TestCase):
    def test_committed_tables_regenerate_byte_identically(self):
        for path in tables.smoke.listed("tables.md"):
            committed = (ROOT / path).read_text(encoding="utf-8")
            if committed.split("\n", 1)[0] != tables.TITLE:
                continue
            with self.subTest(path):
                place = ROOT / path
                campaigns = [tables.campaign(directory)
                             for directory in tables.outcome_directories(place.parent)]
                text = tables.render(place, campaigns, tables.peak_memory(place.parent),
                                     tables.calibration(), tables.profile(tables.profile_summary()))
                self.assertEqual(text, committed)


if __name__ == "__main__":
    unittest.main()
