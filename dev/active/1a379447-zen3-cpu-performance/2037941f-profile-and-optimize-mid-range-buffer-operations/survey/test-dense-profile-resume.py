#!/usr/bin/env python3
"""Exercise the profiler's resume gate without running measurements."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SOURCE = Path(__file__).with_name("run-dense-profile.sh").resolve()
REPOSITORY = Path(subprocess.run(
    ["git", "-C", str(SOURCE.parent), "rev-parse", "--show-toplevel"],
    capture_output=True, check=True, text=True,
).stdout.strip())
SURVEY = SOURCE.parent.relative_to(REPOSITORY)
UNRELATED = "a" * 64


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
EXECUTABLES = (
    "target/e1f9a78f-arms/release/dense-arm",
    "target/e1f9a78f-scalar-arm/release/dense-arm",
    "target/e1f9a78f-arms/release/dense-campaign",
)


class ResumeTest(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        survey = self.root / SURVEY
        survey.mkdir(parents=True)
        self.launcher = survey / SOURCE.name
        stop = 'echo "profile execution log: ${OUT}/execution.log"'
        source = SOURCE.read_text()
        self.assertEqual(source.count(stop), 1)
        self.launcher.write_text(source.replace(stop, "exit 0\n" + stop))
        shutil.copyfile(SOURCE.with_name("repo_artifacts.py"), survey / "repo_artifacts.py")
        (self.root / "dev/scripts").mkdir(parents=True)
        for name in ("resume_identity.py", "repository_files.py"):
            shutil.copyfile(REPOSITORY / "dev/scripts" / name, self.root / "dev/scripts" / name)
        subprocess.run(["git", "-C", str(self.root), "add", "dev/scripts"], check=True)
        (survey / "run-dense-harness.sh").write_text("#!/bin/sh\nexit 0\n")
        (survey / "run-dense-harness.sh").chmod(0o755)
        for name in ("summarize-dense-profile.py", "dense-producing-inputs.json"):
            (survey / name).write_text("fixture\n")
        for relative in EXECUTABLES:
            binary = self.root / relative
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_text("#!/bin/sh\nexit 0\n")
            binary.chmod(0o755)
        self.addendum = self.root / "fixture-addendum.json"
        self.addendum.write_text(json.dumps({"cells": [{"workload": {"seed": 1}}]}))
        self.second_addendum = self.root / "second-addendum.json"
        self.second_addendum.write_text(json.dumps({"cells": [{"workload": {"seed": 3}}]}))
        subprocess.run(["git", "-C", str(self.root), "add", self.addendum.name,
                        self.second_addendum.name], check=True)
        self.cases = self.root / "cases.txt"
        self.cases.write_text("out=profile\ntitle=Fixture\n"
                              "family|fixture-addendum.json|cell|route|simd\n"
                              "family|second-addendum.json|cell|route|simd\n")
        self.log = self.root / "profile/execution.log"
        self.environment = dict(os.environ, GF2_BENCH_WINDOW="1")
        self.environment["HOME"] = str(self.root / "home")

    def run_gate(self, accepted=True):
        result = subprocess.run(
            ["bash", str(self.launcher), "window", "--cases", str(self.cases)],
            cwd=self.root, env=self.environment, text=True, capture_output=True,
        )
        if accepted:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn("resume refused", result.stderr)
        return self.log.read_text().splitlines()[0]

    def test_case_comments_preserve_identity(self):
        identity = self.run_gate()
        self.cases.write_text("# Comment only\n" + self.cases.read_text() + "\n")
        self.assertEqual(self.run_gate(), identity)
        self.assertEqual(self.log.read_text().splitlines(), [identity])

    def test_addendum_formatting_preserves_identity(self):
        identity = self.run_gate()
        self.addendum.write_text(json.dumps(json.loads(self.addendum.read_text()), indent=4))
        self.assertEqual(self.run_gate(), identity)

    def test_title_does_not_decide_resume(self):
        identity = self.run_gate()
        self.cases.write_text(self.cases.read_text().replace("title=Fixture", "title=Description"))
        self.assertEqual(self.run_gate(), identity)

    def test_source_comments_summary_and_closure_do_not_decide_resume(self):
        identity = self.run_gate()
        self.launcher.write_text(self.launcher.read_text() + "# Documentation only\n")
        for name in ("summarize-dense-profile.py", "dense-producing-inputs.json"):
            self.launcher.with_name(name).write_text("changed documentation\n")
        self.assertEqual(self.run_gate(), identity)

    def test_changed_case_refuses_resume(self):
        self.run_gate()
        self.cases.write_text(self.cases.read_text().replace("|cell|", "|another-cell|"))
        self.run_gate(accepted=False)

    def test_changed_addendum_measurement_field_refuses_resume(self):
        self.run_gate()
        self.addendum.write_text(json.dumps({"cells": [{"workload": {"seed": 2}}]}))
        self.run_gate(accepted=False)

    def test_each_changed_executable_refuses_resume(self):
        self.run_gate()
        for relative in EXECUTABLES:
            with self.subTest(executable=relative):
                binary = self.root / relative
                original = binary.read_bytes()
                binary.write_bytes(original + b"# Different executable bytes\n")
                self.run_gate(accepted=False)
                binary.write_bytes(original)

    def test_changed_sampling_configuration_refuses_resume(self):
        self.run_gate()
        self.launcher.write_text(self.launcher.read_text().replace("REPETITIONS=9", "REPETITIONS=10"))
        self.run_gate(accepted=False)

    def write_older_record(self):
        """The first line of a committed `--cases` session's log."""
        arm, scalar, tool = (digest(self.root / relative) for relative in EXECUTABLES)
        self.log.parent.mkdir()
        self.log.write_text(
            f"identity arm={arm} scalar={scalar} tool={tool} script={UNRELATED} "
            f"summary={UNRELATED} closure={UNRELATED} cases={digest(self.cases)} "
            f"addendum={digest(self.addendum)} addendum={digest(self.second_addendum)}\n"
            "session start 2026-10-08T23:07:09Z\n")

    def test_older_identity_form_refuses_naming_a_new_log(self):
        self.write_older_record()
        result = subprocess.run(
            ["bash", str(self.launcher), "window", "--cases", str(self.cases)],
            cwd=self.root, env=self.environment, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("older identity form", result.stderr)
        self.assertIn("new log", result.stderr)

    def test_control_flow_change_refuses_resume(self):
        self.run_gate()
        self.launcher.write_text(self.launcher.read_text().replace(
            '$(seq 1 "${REPETITIONS}")', '$(seq 2 "${REPETITIONS}")'))
        self.run_gate(accepted=False)

    def test_emitted_prose_change_refuses_resume(self):
        self.run_gate()
        self.launcher.write_text(self.launcher.read_text().replace(
            "non-timed profile preparation complete", "preparation finished"))
        self.run_gate(accepted=False)


if __name__ == "__main__":
    unittest.main()
