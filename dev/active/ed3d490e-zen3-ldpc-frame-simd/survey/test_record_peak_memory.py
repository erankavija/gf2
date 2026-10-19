#!/usr/bin/env python3
"""Check the peak memory recorder's statistics, structure check and replay of the committed record.

The committed-record tests read the committed record and build record.
"""

import copy
import importlib.util
import json
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
    def setUp(self):
        self.records = recorder.build_records()

    def test_each_build_record_covers_every_catalogue_arm_of_the_builds_it_records(self):
        covered = set()
        for path, build in self.records:
            with self.subTest(path=str(path)):
                catalogue = recorder.catalogued(build)
                self.assertTrue(covered.isdisjoint(build["arms"]))
                covered |= build["arms"].keys()
        self.assertEqual(covered, catalogue.keys())

    def test_a_build_record_missing_an_arm_of_its_builds_is_refused(self):
        for path, build in self.records:
            with self.subTest(path=str(path)):
                partial = copy.deepcopy(build)
                partial["arms"].popitem()
                with self.assertRaises(SystemExit):
                    recorder.catalogued(partial)

    def test_every_arm_of_every_build_record_has_a_quality_file_on_each_code(self):
        for path, build in self.records:
            files = recorder.quality_files(build)
            self.assertEqual(set(files), {(name, key) for name in build["arms"] for key in CODES})

    def test_a_build_record_is_found_by_its_directory(self):
        for path, build in self.records:
            self.assertEqual(recorder.build_record(path.parent), (path, build))


class CommittedRecordTest(unittest.TestCase):
    def setUp(self):
        paths = recorder.smoke.listed(recorder.RECORD)
        self.assertEqual(len(paths), 1)
        self.record = json.loads((recorder.smoke.ROOT / paths[0]).read_text(encoding="utf-8"))
        self.directory = (recorder.smoke.ROOT / paths[0]).parent
        catalogue = recorder.build_record(self.directory)[1]["catalogue"]["path"]
        self.catalogue = json.loads((recorder.smoke.ROOT / catalogue).read_text(encoding="utf-8"))

    def test_it_passes_its_structure_check(self):
        build = recorder.build_record(self.directory)[1]
        self.assertEqual(recorder.structure_failures(self.record, build), [])
        self.assertEqual(recorder.quality_failures(self.record), [])

    def test_every_cell_states_its_own_arms_configuration(self):
        for name, arm in self.record["arms"].items():
            entry = self.catalogue["arms"][name]
            for key, cell in arm["codes"].items():
                quality = json.loads((recorder.smoke.ROOT / cell["quality"]["path"])
                                     .read_text(encoding="utf-8"))
                self.assertEqual(cell["effective"]["settings"], quality["settings"], (name, key))
                self.assertEqual(cell["effective"]["catalogue"],
                                 {field: entry.get(field) for field in recorder.CATALOGUE_FIELDS},
                                 (name, key))


if __name__ == "__main__":
    unittest.main()
