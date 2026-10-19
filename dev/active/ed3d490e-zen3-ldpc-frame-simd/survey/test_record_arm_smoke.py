#!/usr/bin/env python3
"""Checks the committed runner smoke record against the build record."""

import importlib.util
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("arm_smoke", SURVEY / "record-arm-smoke.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)
peak, smoke, validation = recorder.peak, recorder.smoke, recorder.validation


class StagesTest(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.directory)

    def test_the_stages_have_their_own_scratch_apart_from_the_validation_stages(self):
        self.assertNotEqual(recorder.SCRATCH, validation.SCRATCH)

    def test_an_interrupted_removal_leaves_no_stage_to_resume(self):
        scratch = self.directory / "scratch"
        (scratch / "arm.stage").mkdir(parents=True)
        with mock.patch.object(validation.shutil, "rmtree", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                validation.remove_stages(scratch)
        self.assertEqual((scratch.exists(), scratch.with_name("scratch.removed").exists()),
                         (False, True))

    def test_an_interrupted_record_write_keeps_the_previous_record(self):
        validation.write_record(self.directory, [{"line": 1}], recorder.RECORD)
        written = (self.directory / recorder.RECORD).read_bytes()
        with mock.patch.object(Path, "replace", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                validation.write_record(self.directory, [{"line": 2}], recorder.RECORD)
        self.assertEqual((self.directory / recorder.RECORD).read_bytes(), written)


class CommittedRecordTest(unittest.TestCase):
    """Each build record's smoke record beside it."""

    def records(self):
        for build_path, build in peak.build_records():
            text = (build_path.parent / recorder.RECORD).read_text(encoding="utf-8")
            with self.subTest(build=str(build_path)):
                yield build_path, build, json.loads(text)

    def test_every_arm_has_one_untimed_dispatch_on_each_code(self):
        for _, build, record in self.records():
            found = sorted((entry["arm"], entry["code"]) for entry in record["arms"])
            self.assertEqual(found, sorted((name, key) for name in build["arms"]
                                           for key in smoke.CODES))
            for entry in record["arms"]:
                self.assertEqual((entry["smoke"]["role"], entry["smoke"]["windows"]),
                                 ("validation", 0))

    def test_each_entry_names_its_arm_by_the_build_record(self):
        for build_path, build, record in self.records():
            cited = peak.cited(build_path)
            for entry in record["arms"]:
                self.assertEqual(entry["build_identity"], cited)
                self.assertEqual(entry["smoke"]["executable_sha256"],
                                 build["arms"][entry["arm"]]["executable_sha256"])

if __name__ == "__main__":
    unittest.main()
