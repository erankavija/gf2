#!/usr/bin/env python3
"""Check the peak memory recorder's statistics, structure check and replay of the committed record.

The replay test needs the built arms workspace and the staged AFF3CT comparator
and is skipped without them.
"""

import copy
import importlib.util
import subprocess
import sys
import unittest
from pathlib import Path

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


BUILD = {"rustc": "rustc 1.95.0",
         "executables": {"arm-bin": {"compiler_options": ["-O"]}},
         "catalogue": {"path": "arms.json", "sha256": "1" * 64},
         "arms": {"arm": {"executable": "arm-bin", "executable_sha256": "a" * 64,
                          "environment": {"MODE": "x"}},
                  "other": {"executable": "other-bin", "executable_sha256": "c" * 64,
                            "environment": {}}}}


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

    def test_the_arms_executable_digest_is_checked(self):
        self.assertEqual(len(self.changed(
            lambda build: build["arms"]["arm"].update(executable_sha256="b" * 64))), 1)

    def test_the_toolchain_and_build_flags_are_checked(self):
        self.assertTrue(self.changed(lambda build: build.update(rustc="rustc 1.96.0")))
        self.assertTrue(self.changed(
            lambda build: build["executables"]["arm-bin"].update(compiler_options=["-O0"])))

    def test_another_arm_relinking_is_named_for_that_arm_only(self):
        failures = self.changed(
            lambda build: build["arms"]["other"].update(executable_sha256="d" * 64))
        self.assertEqual([failure.split(":")[0] for failure in failures], ["other"])

    def test_the_shared_record_digest_does_not_matter(self):
        self.assertEqual(self.changed(
            lambda build: build["catalogue"].update(sha256="2" * 64)), [])


class LaunchTest(unittest.TestCase):
    def test_a_launch_reports_a_peak_and_a_launcher_share(self):
        status, out, peak, launcher = recorder.launch(["/bin/true"], {}, b"")
        self.assertEqual((status, out), (0, ""))
        self.assertGreater(min(peak, launcher), 0)

    def test_a_failing_child_reports_its_status(self):
        self.assertNotEqual(recorder.launch(["/bin/false"], {}, b"")[0], 0)


class CommittedRecordTest(unittest.TestCase):
    def test_a_relaunch_stays_within_the_committed_envelope(self):
        root = recorder.smoke.ROOT
        records = [root / path for path in recorder.smoke.listed(recorder.RECORD)]
        self.assertEqual(len(records), 1)
        build = recorder.build_record()[1]
        missing = [row for row in build["arms"].values()
                   if not (root / (row["path"] if "path" in row else row["found_at"][0])).exists()]
        if missing or not (root / "target").is_dir():
            self.skipTest("the arms are not built")
        run = subprocess.run([sys.executable, "-B", str(SURVEY / "record-peak-memory.py"),
                              "check", str(records[0].parent)], cwd=root,
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stdout)


if __name__ == "__main__":
    unittest.main()
