#!/usr/bin/env python3
"""Check the validation record beside the build record against it and the smoke families."""

import importlib.util
import json
import sys
import unittest
from collections import Counter
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("arm_validation", SURVEY / "record-arm-validation.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)
peak, smoke = recorder.peak, recorder.smoke


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
