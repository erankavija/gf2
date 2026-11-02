#!/usr/bin/env python3
"""Check the peak memory recorder's statistics, structure check and replay of the committed record.

The committed-record tests read the committed record and build record.
"""

import copy
import hashlib
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
spec = importlib.util.spec_from_file_location("peak_memory_recorder", SURVEY / "record-peak-memory.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)

LAUNCHES = 4
CODES = tuple(recorder.smoke.CODES)


def group(samples):
    return recorder.summary(samples)


def cell():
    floor = group([10, 11, 10, 12])
    floor["launcher"] = group([9, 9, 9, 9])
    peak = group([50, 52, 51, 51])
    return {**peak, "launcher": group([9, 9, 9, 9]), "floor": floor,
            "above_floor_median_kib": peak["median_kib"] - floor["median_kib"]}


BUILD = {"arms": {"arm": {"source": "pinned", "executable": "arm-bin",
                          "executable_sha256": "a" * 64, "environment": {"MODE": "x"}},
                  "other": {"source": "pinned", "executable": "other-bin",
                            "executable_sha256": "c" * 64, "environment": {}}}}


def record(build=BUILD):
    return {"cites": ["Cassagne2019"], "method": {"launches": LAUNCHES},
            "arms": {name: {"executable": row["executable"],
                            "identity": recorder.arm_identity(build, name),
                            "codes": {key: cell() for key in CODES}}
                     for name, row in build["arms"].items()}}


class SummaryTest(unittest.TestCase):
    def test_statistics_describe_the_samples(self):
        found = recorder.summary([4, 8, 6])
        self.assertEqual((found["count"], found["min_kib"], found["median_kib"], found["max_kib"]),
                         (3, 4, 6, 8))
        self.assertAlmostEqual(found["stdev_kib"], (8 / 3) ** 0.5)

    def test_a_constant_sample_has_no_spread(self):
        self.assertEqual(recorder.summary([7, 7])["stdev_kib"], 0)


class StructureTest(unittest.TestCase):
    def test_a_consistent_record_passes(self):
        self.assertEqual(recorder.structure_failures(record(), BUILD), [])

    def test_a_changed_sample_breaks_the_statistics(self):
        broken = record()
        broken["arms"]["arm"]["codes"][CODES[0]]["samples_kib"][0] += 1
        self.assertEqual(len(recorder.structure_failures(broken, BUILD)), 1)

    def test_a_missing_sample_breaks_the_count(self):
        broken = record()
        broken["arms"]["arm"]["codes"][CODES[0]]["floor"]["samples_kib"].pop()
        self.assertTrue(recorder.structure_failures(broken, BUILD))

    def test_an_unrecorded_code_is_named(self):
        broken = record()
        del broken["arms"]["arm"]["codes"][CODES[0]]
        self.assertEqual(len(recorder.structure_failures(broken, BUILD)), 1)

    def test_an_uncatalogued_arm_is_named(self):
        broken = record()
        broken["arms"]["extra"] = broken["arms"]["arm"]
        self.assertTrue(recorder.structure_failures(broken, BUILD))


class IdentityTest(unittest.TestCase):
    def changed(self, edit):
        build = copy.deepcopy(BUILD)
        edit(build)
        return recorder.structure_failures(record(), build)

    def test_the_arms_executable_digest_and_environment_are_checked(self):
        self.assertEqual(len(self.changed(
            lambda build: build["arms"]["arm"].update(executable_sha256="b" * 64))), 1)
        self.assertEqual(len(self.changed(
            lambda build: build["arms"]["arm"]["environment"].update(MODE="y"))), 1)

    def test_another_arm_relinking_is_named_for_that_arm_only(self):
        failures = self.changed(
            lambda build: build["arms"]["other"].update(executable_sha256="d" * 64))
        self.assertEqual([failure.split(":")[0] for failure in failures], ["other"])

    def test_a_shared_record_digest_does_not_matter(self):
        self.assertEqual(self.changed(
            lambda build: build.update(catalogue={"path": "arms.json", "sha256": "2" * 64})), [])


class LaunchTest(unittest.TestCase):
    def test_a_launch_reports_a_peak_and_a_launcher_share(self):
        status, out, peak, launcher = recorder.launch(["/bin/true"], {}, b"")
        self.assertEqual((status, out), (0, ""))
        self.assertGreater(min(peak, launcher), 0)

    def test_a_failing_child_reports_its_status(self):
        self.assertNotEqual(recorder.launch(["/bin/false"], {}, b"")[0], 0)


class FloorTest(unittest.TestCase):
    def shared(self, executable, script):
        return {"executable": executable, "arguments": ["-c", script], "request": "{}",
                "environment": {"GUARD": "1"}, "guard_variable": "GUARD"}

    def test_the_floor_launch_drops_the_sentinel(self):
        shared = self.shared("/bin/sh", 'test -n "$GUARD" && echo result')
        recorder.arm_launch("arm", shared, False)
        recorder.arm_launch("arm", shared, True)

    def test_a_floor_launch_that_decodes_is_refused(self):
        with self.assertRaises(SystemExit):
            recorder.arm_launch("arm", self.shared("/bin/sh", "echo result"), True)

    def test_a_validation_launch_without_one_result_line_is_refused(self):
        with self.assertRaises(SystemExit):
            recorder.arm_launch("arm", self.shared("/bin/sh", "true"), False)


class BuildRecordsTest(unittest.TestCase):
    def test_a_superseded_build_record_is_not_live(self):
        directory = Path(tempfile.mkdtemp(dir=recorder.smoke.ROOT / "target"))
        self.addCleanup(shutil.rmtree, directory)
        old, new = directory / "old.json", directory / "new.json"
        old.write_text(json.dumps({"catalogue": {}, "arms": {}}), encoding="utf-8")
        supersedes = {"path": str(old.relative_to(recorder.smoke.ROOT)),
                      "sha256": hashlib.sha256(old.read_bytes()).hexdigest()}
        new.write_text(json.dumps({"catalogue": {}, "arms": {}, "supersedes": supersedes}),
                       encoding="utf-8")
        listed = [str(path.relative_to(recorder.smoke.ROOT)) for path in (new, old)]
        with mock.patch.object(recorder.smoke, "listed", return_value=listed):
            self.assertEqual([path for path, _ in recorder.build_records()], [new])
            old.write_text(json.dumps({"catalogue": {}, "arms": {}, "changed": True}),
                           encoding="utf-8")
            self.assertEqual([path for path, _ in recorder.build_records()], [new, old])





if __name__ == "__main__":
    unittest.main()
