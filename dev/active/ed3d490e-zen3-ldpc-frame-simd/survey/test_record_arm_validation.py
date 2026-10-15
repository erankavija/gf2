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

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("arm_validation", SURVEY / "record-arm-validation.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)
peak, smoke = recorder.peak, recorder.smoke


# A staged smoke that pauses (exit 3) on its first `PAUSES` calls, counted in `COUNT`, then completes.
RUNNER = """#!/bin/sh
echo >> "$COUNT"
stage=$4
mkdir -p "$stage"
if [ "$(wc -l < "$COUNT")" -le "$PAUSES" ]; then
    echo '{"event": "paused"}' >> "$stage/execution.log"
    exit 3
fi
echo '{"event": "complete"}' >> "$stage/execution.log"
cp "$2" "$stage/plan.json"
printf '{"cells": []}' > "$6"
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

    def smoke(self, pauses):
        environment = {**os.environ, "COUNT": str(self.count), "PAUSES": str(pauses)}
        recorder.staged_smoke(str(self.runner), environment, self.plan, self.stage, self.record)
        return len(self.count.read_text(encoding="utf-8").splitlines()) if self.count.exists() else 0

    def test_a_paused_stage_is_resumed_until_it_completes(self):
        self.assertEqual(self.smoke(pauses=2), 3)
        self.assertTrue(recorder.complete(self.stage))

    def test_a_complete_stage_is_reused_without_a_dispatch(self):
        self.smoke(pauses=0)
        self.count.unlink()
        self.assertEqual(self.smoke(pauses=0), 0)

    def test_a_complete_stage_of_another_plan_is_refused(self):
        self.smoke(pauses=0)
        self.plan.write_text('{"changed": true}', encoding="utf-8")
        with self.assertRaises(SystemExit):
            self.smoke(pauses=0)

    def test_a_complete_stage_of_another_executable_is_refused(self):
        executable = self.directory / "arm"
        executable.write_bytes(b"built")
        self.smoke(pauses=0)
        self.record.write_text(json.dumps({"cells": [{"arms": [{
            "executable": str(executable), "executable_sha256": "0" * 64}]}]}), encoding="utf-8")
        with self.assertRaises(SystemExit):
            self.smoke(pauses=0)


class CommittedRecordTest(unittest.TestCase):
    def setUp(self):
        self.build_path, self.build = peak.build_record()
        text = (self.build_path.parent / recorder.RECORD).read_text(encoding="utf-8")
        self.lines = [json.loads(line) for line in text.splitlines()]

    def test_every_arm_runs_every_family_cell_of_each_code(self):
        declared = Counter()
        for family in recorder.FAMILIES:
            addendum = json.loads((smoke.ROOT / smoke.located("file", smoke.FAMILIES[family][0]))
                                  .read_text(encoding="utf-8"))
            for cell in addendum["cells"]:
                key = next(key for key in smoke.CODES if cell["cell_id"].startswith(key + "-"))
                declared[family, key, cell["workload"]["size"]["workers"]] += 1
        expected = Counter({(name, *cell): count for name in self.build["arms"]
                            for cell, count in declared.items()})
        found = Counter((line["arm"], line["family"], line["code"], line["workers"])
                        for line in self.lines)
        self.assertEqual(found, expected)

    def test_each_line_names_its_arm_by_the_build_record(self):
        cited = peak.cited(self.build_path)
        for line in self.lines:
            self.assertEqual(line["build_identity"], cited)
            self.assertEqual(line["validation"]["executable_sha256"],
                             self.build["arms"][line["arm"]]["executable_sha256"])

    def test_each_worker_is_pinned_and_decodes_every_recorded_frame_untimed(self):
        files = peak.quality_files(self.build)
        for line in self.lines:
            self.assertEqual((line["validation"]["role"], line["validation"]["windows"]),
                             ("validation", 0))
            path, digest = files[line["arm"], line["code"]]
            self.assertEqual((line["quality"]["path"], line["quality"]["sha256"]),
                             (str(path), digest))
            quality = json.loads((smoke.ROOT / path).read_text(encoding="utf-8"))
            self.assertEqual(line["batch_frames"], len(quality["frame_bit_errors"]))
            workers = line[smoke.PLACEMENT]["workers"]
            self.assertEqual(len(workers), line["workers"])
            for worker in workers:
                self.assertEqual((worker["affinity"], worker["cpus_seen"], worker["calls"]),
                                 ([worker["cpu"]], [worker["cpu"]], 1))


if __name__ == "__main__":
    unittest.main()
