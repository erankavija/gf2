#!/usr/bin/env python3
"""Checks the family generator on the sample declaration beside this test."""

import ast
import copy
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SURVEY = Path(__file__).resolve().parent
SAMPLE = SURVEY / "test_make_family_sample.json"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("make_family", SURVEY / "make-family.py")
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)
peak, smoke = generator.peak, generator.smoke
CONSERVATIVE, TUNED = "gf2-batch-conservative-portable", "gf2-batch-tuned-portable"
ENVELOPE = Path(smoke.output(["git", "-C", str(smoke.ROOT), "ls-files",
                              "crates/gf2-coding/data/tuning-profiles/*.json"]))


class GeneratorTest(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.directory)
        self.declaration = json.loads(SAMPLE.read_text(encoding="utf-8"))

    def generate(self, declaration=None, name="out"):
        path = self.directory / f"{name}.json"
        path.write_text(json.dumps(declaration or self.declaration), encoding="utf-8")
        out_dir = self.directory / name
        generator.generate(path, out_dir)
        return (json.loads((out_dir / generator.ADDENDUM).read_text(encoding="utf-8")),
                json.loads((out_dir / generator.PLAN).read_text(encoding="utf-8")))

    def test_the_runner_checks_the_sample_plan_and_its_addendum(self):
        self.generate()
        runner, environment = smoke.build_runner()
        checked = subprocess.run([runner, "check", str(self.directory / "out" / generator.PLAN)],
                                 env=environment, capture_output=True, text=True)
        self.assertEqual(checked.returncode, 0, checked.stderr)

    def test_a_plan_inside_the_repository_names_its_addendum_relative_to_it(self):
        out_dir = smoke.ROOT / "target" / "make-family-test"
        self.addCleanup(shutil.rmtree, out_dir, ignore_errors=True)
        generator.generate(SAMPLE, out_dir)
        plan = json.loads((out_dir / generator.PLAN).read_text(encoding="utf-8"))
        self.assertFalse(Path(plan["addendum"]).is_absolute())
        self.assertEqual(smoke.ROOT / plan["addendum"], out_dir / generator.ADDENDUM)
        runner, environment = smoke.build_runner()
        checked = subprocess.run([runner, "check", str(out_dir / generator.PLAN)],
                                 env=environment, capture_output=True, text=True)
        self.assertEqual(checked.returncode, 0, checked.stderr)

    def test_two_runs_write_identical_bytes(self):
        self.generate()
        first = {name: (self.directory / "out" / name).read_bytes()
                 for name in (generator.ADDENDUM, generator.PLAN)}
        self.generate()
        self.assertEqual(first, {name: (self.directory / "out" / name).read_bytes()
                                 for name in first})

    def test_declared_values_reach_the_output(self):
        changed = copy.deepcopy(self.declaration)
        changed["campaign"]["seed"] += 1
        changed["campaign"]["pilot_pairs"] += 1
        changed["max_pilot_trials_per_cell"] += 1
        changed["cell"]["timed_batch"] += 1
        changed["cell"]["decoder"]["iteration_cap"] += 1
        changed["cell"]["decoder"]["quality_tolerance"]["fer_ratio_max"] += 1
        before, before_plan = self.generate(name="before")
        after, after_plan = self.generate(changed, name="after")
        self.assertEqual(after_plan["campaign_seed"], before_plan["campaign_seed"] + 1)
        self.assertEqual(after["search_budget"]["max_pilot_trials_per_cell"],
                         before["search_budget"]["max_pilot_trials_per_cell"] + 1)
        first, first_after = before["cells"][0], after["cells"][0]
        self.assertEqual(first_after["workload"]["size"]["timed_batch"],
                         first["workload"]["size"]["timed_batch"] + 1)
        self.assertEqual(first_after["decoder"]["iteration_cap"],
                         first["decoder"]["iteration_cap"] + 1)
        self.assertEqual(first_after["decoder"]["quality_tolerance"]["fer_ratio_max"],
                         first["decoder"]["quality_tolerance"]["fer_ratio_max"] + 1)
        cell, cell_after = before_plan["cells"][0], after_plan["cells"][0]
        self.assertEqual(cell_after["pilot_pairs"], cell["pilot_pairs"] + 1)
        self.assertEqual(cell_after["case"]["batch_size"], cell["case"]["batch_size"] + 1)
        self.assertEqual(cell_after["case"]["iteration_cap"], cell["case"]["iteration_cap"] + 1)

    def test_a_cell_entry_overrides_the_shared_cell(self):
        addendum, plan = self.generate()
        for declared, cell, case in zip(self.declaration["cells"], addendum["cells"],
                                        (cell["case"] for cell in plan["cells"])):
            merged = generator.merged(self.declaration["cell"], declared)
            self.assertEqual((cell["objective"], cell["scaling"]),
                             (merged["objective"], merged["scaling"]))
            self.assertEqual(case["batch_size"], merged["timed_batch"])
            self.assertEqual(cell["decoder"]["batching"], merged["decoder"]["batching"])

    def test_a_declared_case_overrides_the_derived_case_only(self):
        changed = copy.deepcopy(self.declaration)
        derived = self.declaration["cell"]["decoder"]["normalization"]["factor"]
        changed["cells"][0]["case"] = {"normalization_factor": derived + 1}
        before, before_plan = self.generate(name="before")
        after, after_plan = self.generate(changed, name="after")
        self.assertEqual(after_plan["cells"][0]["case"]["normalization_factor"], derived + 1)
        unchanged = [{key: value for key, value in cell["case"].items() if key != "bundle"}
                     for cell in (before_plan["cells"][1], after_plan["cells"][1])]
        self.assertEqual(*unchanged)
        self.assertEqual(after["cells"], before["cells"])

    def test_shared_settings_come_from_the_protocol(self):
        settings = generator.shared_settings(generator.schema())
        addendum, _ = self.generate()
        self.assertEqual(addendum["family_wise"]["alpha"], settings["family_alpha"])
        self.assertEqual(addendum["search_budget"]["max_confirmatory_attempts_per_candidate"],
                         settings["max_confirmatory_attempts_per_candidate"])
        moved = {name: value * 2 for name, value in settings.items()}
        with mock.patch.object(generator, "shared_settings", return_value=moved):
            addendum, _ = self.generate(name="moved")
        self.assertEqual(addendum["family_wise"]["alpha"], moved["family_alpha"])
        for cell in addendum["cells"]:
            self.assertEqual(cell["decoder"]["quality_tolerance"]["confidence"],
                             moved["quality_confidence"])

    def test_arms_follow_the_build_identity_record(self):
        build_path, build = next((path, build) for path, build in peak.build_records()
                                 if self.declaration["cell"]["candidate"] in build["arms"])
        _, plan = self.generate()
        for cell in plan["cells"]:
            for role in ("baseline", "candidate"):
                name = self.declaration["cell"][role]
                arm = plan["arms"][cell[f"{role}_arm"]]
                self.assertEqual(arm["executable"],
                                 str(peak.executable_path(
                                     build["arms"][name], Path(build["build_root"]))))
                self.assertLessEqual(build["arms"][name]["environment"].items(),
                                     arm["environment"].items())
        moved = copy.deepcopy(build)
        name = self.declaration["cell"]["candidate"]
        moved["arms"][name]["path"] += "-moved"
        moved["arms"][name]["environment"]["GF2_GENERATOR_PROBE"] = "moved"
        records = [(path, moved if path == build_path else record)
                   for path, record in peak.build_records()]
        with mock.patch.object(peak, "build_records", return_value=records):
            _, plan = self.generate(name="moved")
        arm = plan["arms"][plan["cells"][0]["candidate_arm"]]
        self.assertTrue(arm["executable"].endswith("-moved"))
        self.assertEqual(arm["environment"]["GF2_GENERATOR_PROBE"], "moved")

    def portable(self, profiles):
        declaration = copy.deepcopy(self.declaration)
        declaration["cell"].update(baseline=CONSERVATIVE, candidate=TUNED)
        declaration["tuning_profiles"] = profiles
        return declaration

    def test_a_named_tuning_profile_reaches_the_plan_and_the_addendum_build(self):
        addendum, plan = self.generate(self.portable({TUNED: str(ENVELOPE)}))
        envelope = (smoke.ROOT / ENVELOPE).read_bytes()
        named = {"id": json.loads(envelope)["profile_id"],
                 "sha256": hashlib.sha256(envelope).hexdigest()}
        for cell, planned in zip(addendum["cells"], plan["cells"]):
            self.assertEqual(cell["builds"], {"baseline": "conservative-portable",
                                              "candidate": "tuned-portable"})
            self.assertEqual(plan["arms"][planned["candidate_arm"]]["tuning_profile"], named)
            baseline = plan["arms"][planned["baseline_arm"]]
            self.assertEqual((baseline["build"], baseline["tuning_profile"]),
                             ("conservative-portable", None))
            self.assertIsNone(baseline["rustflags"])
        self.assert_checked(plan)

    def test_a_portable_arm_without_a_named_profile_is_conservative(self):
        addendum, plan = self.generate(self.portable({}))
        for cell in addendum["cells"]:
            self.assertEqual(set(cell["builds"].values()), {"conservative-portable"})
        self.assertTrue(all(arm["tuning_profile"] is None for arm in plan["arms"].values()))

    def test_a_profile_of_an_uncatalogued_arm_is_refused(self):
        with self.assertRaises(SystemExit):
            self.generate(self.portable({"uncatalogued": str(ENVELOPE)}))

    def assert_checked(self, plan):
        path = self.directory / "checked.plan.json"
        path.write_text(json.dumps(plan), encoding="utf-8")
        runner, environment = smoke.build_runner()
        checked = subprocess.run([runner, "check", str(path)], env=environment,
                                 capture_output=True, text=True)
        self.assertEqual(checked.returncode, 0, checked.stderr)

    def test_workload_sizes_come_from_the_recorded_bundles(self):
        addendum, plan = self.generate()
        for cell, case in zip(addendum["cells"], (cell["case"] for cell in plan["cells"])):
            manifest = json.loads((Path(case["bundle"]) / "manifest.json").read_text())
            size = cell["workload"]["size"]
            self.assertEqual((size["n"], size["k"], size["frames"]),
                             (manifest["n"], manifest["k"], manifest["frames"]))
            self.assertEqual(size["timed_frames"], size["timed_batch"] * size["workers"])
            self.assertEqual(cell["decoder"]["input"]["llr_sha256"], manifest["llrs_sha256"])

    def test_a_confirmation_plans_its_frozen_cells_at_the_confirmatory_count(self):
        pilot, pilot_plan = self.generate()
        declaration = copy.deepcopy(self.declaration)
        declaration["confirmation"] = {"id": "generator-sample-confirmation",
                                       "seed": declaration["campaign"]["seed"] + 1,
                                       "max_cells_per_session":
                                           declaration["campaign"]["max_cells_per_session"]}
        frozen = copy.deepcopy(pilot)
        frozen["cells"] = [{**cell, "role": "confirmatory"} for cell in pilot["cells"][1:]]
        path = self.directory / "confirmation.json"
        path.write_text(json.dumps(declaration), encoding="utf-8")
        out_dir = self.directory / "confirmation"
        out_dir.mkdir()
        frozen_bytes = json.dumps(frozen).encode()
        (out_dir / generator.ADDENDUM).write_bytes(frozen_bytes)
        generator.generate(path, out_dir, confirmation=True)
        plan = json.loads((out_dir / generator.PLAN).read_text(encoding="utf-8"))
        self.assertEqual((out_dir / generator.ADDENDUM).read_bytes(), frozen_bytes)
        self.assertEqual((plan["label"], plan["campaign_id"], plan["campaign_seed"]),
                         ("confirmation", declaration["confirmation"]["id"],
                          declaration["confirmation"]["seed"]))
        self.assertEqual(plan["addendum"], str(out_dir / generator.ADDENDUM))
        self.assertEqual([cell["cell_id"] for cell in plan["cells"]],
                         [cell["cell_id"] for cell in frozen["cells"]])
        self.assertTrue(all(cell["pilot_pairs"] is None for cell in plan["cells"]))

        def measured(cell):
            return {**{key: value for key, value in cell.items() if key != "pilot_pairs"},
                    "case": {key: value for key, value in cell["case"].items()
                             if key != "bundle"}}
        self.assertEqual(list(map(measured, plan["cells"])),
                         list(map(measured, pilot_plan["cells"][1:])))

    def inventory(self, frame_len=0, edges=0, seed=0, batch=0):
        """A calibration list-grid record of one stratum, in the producer's form."""
        entry = {"field": "decode.lane_min_frames", "code": "inventory-code",
                 "frame_len": frame_len, "edges": edges, "batch_frames": [batch],
                 "seeds": {"root": seed, "streams": [{"bank": 0, "role": 0, "seed": seed}]}}
        path = self.directory / f"inventory-{frame_len}-{edges}-{seed}-{batch}.json"
        path.write_text(json.dumps({"operation": "list-grid",
                                    "evidence": json.dumps({"inventory": [entry]})}),
                        encoding="utf-8")
        return path

    def with_holdout(self, inventory):
        declaration = copy.deepcopy(self.declaration)
        holdout = dict(declaration["cells"][1], cell_id="nr-bg1-z384-lane-b5-w1", timed_batch=5)
        declaration["holdout_declaration"] = {"inventory": str(inventory),
                                              "rationale": "fresh inputs", "cells": [holdout]}
        return declaration

    def holdout_record(self, declaration, name="out"):
        addendum, plan = self.generate(declaration, name=name)
        record = self.directory / name / generator.HOLDOUT
        return addendum, plan, json.loads(record.read_text(encoding="utf-8"))

    def test_the_holdout_record_is_the_freezers_declaration_of_cells_no_pilot_measures(self):
        addendum, plan, record = self.holdout_record(self.with_holdout(self.inventory()))
        self.assertEqual(record["family"], addendum["family"]["id"])
        self.assertEqual(record["rationale"], "fresh inputs")
        (cell,) = record["addendum_cells"]
        self.assertEqual((cell["cell_id"], cell["role"]), ("nr-bg1-z384-lane-b5-w1", "holdout"))
        self.assertEqual(cell["workload"]["size"]["timed_batch"], 5)
        pilot = {entry["cell_id"] for entry in addendum["cells"]}
        self.assertNotIn(cell["cell_id"], pilot)
        self.assertEqual({entry["cell_id"] for entry in plan["cells"]}, pilot)
        self.assertEqual({key: value for key, value in cell.items() if key != "role"},
                         {key: value for key, value in addendum["cells"][1].items()
                          if key != "role"} | {"cell_id": cell["cell_id"],
                                               "workload": cell["workload"]})

    def test_the_holdout_record_shows_each_cell_absent_from_the_inventory(self):
        inventory = self.inventory()
        _, _, record = self.holdout_record(self.with_holdout(inventory))
        self.assertEqual(record["inventory"]["sha256"],
                         hashlib.sha256(inventory.read_bytes()).hexdigest())
        (absent,) = record["absence"]
        (cell,) = record["addendum_cells"]
        size = cell["workload"]["size"]
        self.assertEqual(absent, {"cell_id": cell["cell_id"],
                                  "code": {"frame_len": size["n"],
                                           "edges": absent["code"]["edges"]},
                                  "seed": cell["workload"]["seed"],
                                  "batch_frames": size["timed_batch"]})
        self.assertNotIn(absent["code"], record["inventory"]["codes"])
        self.assertNotIn(absent["seed"], record["inventory"]["seeds"])
        self.assertNotIn(absent["batch_frames"], record["inventory"]["batch_frames"])

    def test_a_holdout_cell_the_inventory_holds_is_refused(self):
        _, _, record = self.holdout_record(self.with_holdout(self.inventory()))
        (absent,) = record["absence"]
        for present in ({"frame_len": absent["code"]["frame_len"],
                         "edges": absent["code"]["edges"]},
                        {"seed": absent["seed"]}, {"batch": absent["batch_frames"]}):
            with self.subTest(present=present), self.assertRaises(SystemExit):
                self.generate(self.with_holdout(self.inventory(**present)), name="present")

    def test_a_holdout_cell_that_is_also_a_pilot_cell_is_refused(self):
        declaration = self.with_holdout(self.inventory())
        declaration["holdout_declaration"]["cells"][0]["cell_id"] = \
            declaration["cells"][0]["cell_id"]
        with self.assertRaises(SystemExit):
            self.generate(declaration)

    def test_a_confirmation_plans_its_frozen_holdout_cells(self):
        declaration = self.with_holdout(self.inventory())
        pilot, _, record = self.holdout_record(declaration)
        declaration["confirmation"] = {"id": "generator-sample-confirmation",
                                       "seed": declaration["campaign"]["seed"] + 1,
                                       "max_cells_per_session":
                                           declaration["campaign"]["max_cells_per_session"]}
        frozen = copy.deepcopy(pilot)
        frozen["cells"] = ([{**cell, "role": "confirmatory"} for cell in pilot["cells"]]
                           + record["addendum_cells"])
        path = self.directory / "confirmation.json"
        path.write_text(json.dumps(declaration), encoding="utf-8")
        out_dir = self.directory / "confirmation"
        out_dir.mkdir()
        (out_dir / generator.ADDENDUM).write_text(json.dumps(frozen), encoding="utf-8")
        generator.generate(path, out_dir, confirmation=True)
        plan = json.loads((out_dir / generator.PLAN).read_text(encoding="utf-8"))
        self.assertEqual([cell["cell_id"] for cell in plan["cells"]],
                         [cell["cell_id"] for cell in frozen["cells"]])
        self.assertEqual(plan["cells"][-1]["case"]["batch_size"], 5)
        self.assertFalse((out_dir / generator.HOLDOUT).exists())

    def test_a_declaration_without_holdout_cells_writes_no_holdout_record(self):
        self.generate()
        self.assertFalse((self.directory / "out" / generator.HOLDOUT).exists())

    def test_the_generator_holds_no_numeric_literal(self):
        tree = ast.parse((SURVEY / "make-family.py").read_text(encoding="utf-8"))
        numbers = [node.value for node in ast.walk(tree) if isinstance(node, ast.Constant)
                   and isinstance(node.value, (int, float)) and not isinstance(node.value, bool)]
        self.assertEqual(numbers, [])


if __name__ == "__main__":
    unittest.main()
