#!/usr/bin/env python3
"""Check the staged smoke's resumption and the validation record beside the build record.

The committed-record tests read the committed record, the build record and the smoke families.
"""

import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest import mock

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("arm_validation", SURVEY / "record-arm-validation.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)
peak, smoke = recorder.peak, recorder.smoke


# A runner that pauses (exit 3) on its first `PAUSES` calls, counted in `COUNT`, then exits
# `STATUS` and, on success, writes the record.
RUNNER = """#!/bin/sh
echo >> "$COUNT"
if [ "$(wc -l < "$COUNT")" -le "$PAUSES" ]; then
    exit 3
fi
[ "$STATUS" = 0 ] && printf '{"cells": []}' > "$6"
exit "$STATUS"
"""


class StagedSmokeTest(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.directory)
        self.runner = self.directory / "runner"
        self.runner.write_text(RUNNER, encoding="utf-8")
        self.runner.chmod(0o755)
        self.plan = self.directory / "arm.plan.json"
        self.plan.write_text("{}", encoding="utf-8")
        self.stage, self.record = self.directory / "arm.stage", self.directory / "arm.smoke.json"
        self.count = self.directory / "count"

    def smoke(self, pauses, status=0):
        environment = {**os.environ, "COUNT": str(self.count), "PAUSES": str(pauses),
                       "STATUS": str(status)}
        recorder.staged_smoke(str(self.runner), environment, self.plan, self.stage, self.record)
        return len(self.count.read_text(encoding="utf-8").splitlines())

    def test_a_paused_stage_is_resumed_until_it_completes(self):
        self.assertEqual(self.smoke(pauses=2), 3)
        self.assertTrue(self.record.exists())

    def test_every_call_goes_through_the_runner_which_reexports_a_complete_stage(self):
        self.smoke(pauses=0)
        self.record.unlink()
        self.assertEqual(self.smoke(pauses=0), 2)
        self.assertTrue(self.record.exists())

    def test_a_failed_smoke_stops_the_recorder(self):
        with self.assertRaises(SystemExit):
            self.smoke(pauses=0, status=1)


class InterruptionTest(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.directory)

    def test_a_torn_trailing_journal_line_is_left_to_the_runner(self):
        stage = self.directory / "stage"
        stage.mkdir()
        event = {"event": "child-diagnostic", "case": {"cell_id": "c", "arm": "a"},
                 "details": {"stderr": ""}}
        (stage / "execution.log").write_text(
            json.dumps(event) + "\n" + json.dumps(event)[:20], encoding="utf-8")
        self.assertEqual(smoke.child_diagnostics(stage), [event])

    def test_an_interrupted_record_write_keeps_the_previous_record(self):
        recorder.write_record(self.directory, [{"line": 1}])
        written = (self.directory / recorder.RECORD).read_bytes()
        with mock.patch.object(Path, "replace", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                recorder.write_record(self.directory, [{"line": 2}])
        self.assertEqual((self.directory / recorder.RECORD).read_bytes(), written)

    def test_an_interrupted_removal_leaves_no_stage_to_resume(self):
        scratch = self.directory / "scratch"
        (scratch / "family" / "arm.stage").mkdir(parents=True)
        removed = scratch.with_name("scratch.removed")
        with mock.patch.multiple(recorder, SCRATCH=scratch, REMOVED=removed), \
                mock.patch.object(recorder.shutil, "rmtree", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                recorder.remove_stages()
        self.assertEqual((scratch.exists(), removed.exists()), (False, True))



if __name__ == "__main__":
    unittest.main()
