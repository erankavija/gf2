#!/usr/bin/env python3
"""Check the family launcher's refusals untimed and its resumption on an untimed fake family.

The fake family's arms are the runner package's synthetic workload; one of them waits at a gate,
which holds a session open until the test interrupts the launcher.
"""

import importlib.util
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
LAUNCHER = SURVEY / "run-family-campaign.py"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("run_family_campaign", LAUNCHER)
launcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launcher)
smoke, peak = launcher.smoke, launcher.peak
ROOT = smoke.ROOT
SCRATCH = ROOT / "target/run-family-campaign-test"
CELLS = ("first", "second", "third")
GATED_CELL = "second"
# The arm the gated cell's candidate runs: it records its process ID once reached, then waits for
# the gate. The runner starts it in a process group of its own.
GATED_ARM = """#!/bin/sh
echo $$ > "$REACHED.partial" && mv "$REACHED.partial" "$REACHED"
until [ -e "$GATE" ]; do sleep 1; done
exec {workload}
"""


def addendum(ledger, role):
    """A version-4 kernel family over the synthetic workload, one cell per `CELLS`."""
    cell = {
        "objective": "improvement", "role": role,
        "workload": {"identity": "xor-fold", "size": {"words": 64}, "seed": 1},
        "metric_kind": "kernel-isolated", "scaling": "single-core-latency",
        "core_arm": "single-core", "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {"baseline": "conservative-portable", "candidate": "conservative-portable"},
        "conversion_costs_included": False,
    }
    return {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {"id": "launcher-fixture-family", "issue": "dbea4e10",
                   "purpose": "kernel-family", "description": "launcher fixture family"},
        "frozen": {"frozen_utc": "2026-10-01T00:00:00Z"},
        "effect": {"worthwhile_speedup": None, "rationale": "fixture",
                   "measurement_resolution": None, "resolution_evidence": None,
                   "equivalence_margin": None, "equivalence_rationale": "fixture",
                   "material_gap_threshold": None, "material_gap_rationale": "fixture"},
        "complexity_budget": {"max_new_unsafe_kernels": 0, "max_added_source_lines": None,
                              "maintenance_rationale": "fixture"},
        "family_wise": {"alpha": 0.05, "prior_confirmatory_trials": 0, "prior_trials": [],
                        "ledger_path": ledger},
        "search_budget": {"max_pilot_trials_per_cell": 2,
                          "max_confirmatory_attempts_per_candidate": 1},
        "holdout": {"required": False, "cells": []},
        "cells": [{"cell_id": cell_id, **cell} for cell_id in CELLS],
    }


def plan(campaign_id, addendum_path, lock, label, workload, gated, gate, reached):
    """Plan of `addendum`: one cell per session, the gated arm the candidate of `GATED_CELL`."""
    def arm(executable, environment):
        return {"build": "conservative-portable", "description": "synthetic workload",
                "executable": str(executable), "arguments": [], "environment": environment,
                "rustflags": None, "tuning_profile": None}
    return {
        "schema": "zen3-benchmark-plan-v1", "campaign_id": campaign_id, "issue": "dbea4e10",
        "label": label, "campaign_seed": 1, "addendum": addendum_path, "lock_path": str(lock),
        "wrapper": "unused", "timing_override": None,
        "arms": {"baseline": arm(workload, {"GF2_SMOKE_PASSES": "2"}),
                 "candidate": arm(workload, {}),
                 "gated": arm(gated, {"GATE": str(gate), "REACHED": str(reached)})},
        "cells": [{"cell_id": cell_id, "baseline_arm": "baseline",
                   "candidate_arm": "gated" if cell_id == GATED_CELL else "candidate",
                   "case": {"words": 64, "seed": index}, "pilot_pairs": None}
                  for index, cell_id in enumerate(CELLS)],
        "max_cells_per_session": 1,
    }


class Fixture:
    """A fake family under `SCRATCH`, or under `root` for a repository of its own."""

    def __init__(self, test, label="pilot", root=None):
        self.root = root or ROOT
        self.directory = Path(tempfile.mkdtemp(dir=SCRATCH if root is None else root))
        test.addCleanup(shutil.rmtree, self.directory, ignore_errors=True)
        relative = self.directory.relative_to(self.root)
        self.campaign_id = "launcher-fixture-" + self.directory.name.lower().replace("_", "-")
        self.campaign = ROOT / "target/campaigns" / self.campaign_id
        test.addCleanup(shutil.rmtree, self.campaign, ignore_errors=True)
        self.ledger = self.directory / "ledger.jsonl"
        self.gate, self.reached = self.directory / "gate", self.directory / "reached"
        self.gated = self.directory / "gated-arm"
        self.gated.write_text(GATED_ARM.format(workload=Workload.path), encoding="utf-8")
        self.gated.chmod(0o755)
        self.addendum = self.directory / "addendum.json"
        self.addendum.write_text(json.dumps(addendum(str(relative / "ledger.jsonl"), "exploratory"
                                                     if label == "pilot" else "confirmatory")),
                                 encoding="utf-8")
        self.plan = self.directory / "plan.json"
        self.plan.write_text(json.dumps(plan(
            self.campaign_id, str(relative / "addendum.json"), self.directory / "unused.lock",
            label, Workload.path, self.gated, self.gate, self.reached)), encoding="utf-8")
        self.identity = self.directory / "build-identity.json"
        self.identify(Workload.path, self.gated)

    def identify(self, *executables):
        """Writes a build identity record naming the digests of `executables`."""
        arms = {f"arm-{index}": {"executable_sha256": peak.sha256_file(path)}
                for index, path in enumerate(executables)}
        self.identity.write_text(json.dumps({"arms": arms}), encoding="utf-8")

    def command(self, *options):
        return [sys.executable, str(LAUNCHER), str(self.plan), str(self.identity), *options]

    def run(self, *options, window=None):
        environment = {key: value for key, value in os.environ.items()
                       if key != launcher.WINDOW}
        if window:
            environment[launcher.WINDOW] = window
        return subprocess.run(self.command(*options), cwd=self.root, env=environment,
                              capture_output=True, text=True)

    def journal(self):
        log = self.campaign / "untimed-stage/execution.log"
        return [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]


class Workload:
    """The synthetic workload arm, built once."""
    path = None

    @classmethod
    def build(cls):
        if cls.path is None:
            runner, _ = smoke.build_runner("ab-smoke-workload")
            cls.path = Path(runner).with_name("ab-smoke-workload")


def setUpModule():
    SCRATCH.mkdir(parents=True, exist_ok=True)
    Workload.build()


def cells(journal, event):
    return [record["case"]["cell_id"] for record in journal if record["event"] == event]


class ResumptionTest(unittest.TestCase):
    def test_an_interrupted_campaign_resumes_under_its_identity_without_repeating_a_cell(self):
        fixture = Fixture(self)
        interrupted = subprocess.Popen(fixture.command("--untimed"), cwd=ROOT,
                                       stdout=subprocess.PIPE, text=True,
                                       start_new_session=True)
        self.addCleanup(interrupted.wait)
        try:
            while not fixture.reached.exists():
                self.assertIsNone(interrupted.poll(), "the launcher ended before the gate")
                time.sleep(0.1)
            before = fixture.journal()
        finally:
            os.killpg(interrupted.pid, signal.SIGKILL)
            if fixture.reached.exists():
                os.kill(int(fixture.reached.read_text(encoding="utf-8")), signal.SIGKILL)
        self.assertEqual(cells(before, "cell-complete"), ["first"])
        self.assertEqual(interrupted.stdout.readline().strip(),
                         f"GF2_CAMPAIGN_EXECUTION_LOG="
                         f"{fixture.campaign / 'untimed-stage/execution.log'}")
        interrupted.stdout.close()

        fixture.gate.touch()
        resumed = fixture.run("--untimed")
        self.assertEqual(resumed.returncode, 0, resumed.stderr)
        self.assertTrue(resumed.stdout.startswith("GF2_CAMPAIGN_EXECUTION_LOG="))
        journal = fixture.journal()
        self.assertEqual(journal[:len(before)], before)
        after = journal[len(before):]
        self.assertEqual(Counter(cells(journal, "cell-complete")), Counter(CELLS))
        self.assertNotIn("first", cells(after, "cell-start"))
        self.assertEqual(cells(after, "cell-abandoned"), [GATED_CELL])
        self.assertEqual([record["event"] for record in journal
                          if record["event"] in ("paused", "complete")][-1], "complete")
        self.assertTrue((fixture.campaign / "untimed-smoke.json").exists())
        self.assertFalse(fixture.ledger.exists(), "an untimed run created the family ledger")


class RefusalTest(unittest.TestCase):
    def assert_refused(self, result, fixture, reason):
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(reason, result.stderr)
        self.assertNotIn("GF2_CAMPAIGN_EXECUTION_LOG", result.stdout)
        self.assertFalse(fixture.campaign.exists())
        self.assertFalse(fixture.ledger.exists())

    def test_a_campaign_outside_the_benchmark_window_is_refused(self):
        fixture = Fixture(self)
        self.assert_refused(fixture.run(), fixture, "benchmark window")
        self.assert_refused(fixture.run(window="0"), fixture, "benchmark window")

    def test_a_confirmation_whose_addendum_is_uncommitted_is_refused(self):
        fixture = Fixture(self, label="confirmation")
        self.assert_refused(fixture.run(window="1"), fixture, "uncommitted")

    def test_a_confirmation_whose_addendum_differs_from_its_commit_is_refused(self):
        repository = Path(tempfile.mkdtemp(dir=SCRATCH))
        self.addCleanup(shutil.rmtree, repository)
        git = ["git", "-C", str(repository), "-c", "user.name=fixture",
               "-c", "user.email=fixture@invalid"]
        subprocess.run([*git, "init", "-q"], check=True)
        fixture = Fixture(self, label="confirmation", root=repository)
        fixture.identify(Workload.path)
        subprocess.run([*git, "add", "-A"], check=True)
        subprocess.run([*git, "commit", "-q", "-m", "fixture"], check=True)
        # The committed addendum passes on to the next check, which refuses the gated arm.
        self.assert_refused(fixture.run(window="1"), fixture, "gated")
        with fixture.addendum.open("a", encoding="utf-8") as stream:
            stream.write("\n")
        self.assert_refused(fixture.run(window="1"), fixture, "uncommitted")

    def test_an_arm_the_build_identity_does_not_name_is_refused(self):
        fixture = Fixture(self)
        fixture.identify(Workload.path)
        self.assert_refused(fixture.run("--untimed"), fixture, "gated")


class GenesisTest(unittest.TestCase):
    def test_the_ledger_genesis_is_created_once(self):
        directory = Path(tempfile.mkdtemp(dir=SCRATCH))
        self.addCleanup(shutil.rmtree, directory)
        ledger = directory / "family" / "ledger.jsonl"
        launcher.genesis(ledger)
        self.assertEqual(ledger.read_bytes(), b"")
        ledger.write_bytes(b"{}\n")
        launcher.genesis(ledger)
        self.assertEqual(ledger.read_bytes(), b"{}\n")


if __name__ == "__main__":
    unittest.main()
