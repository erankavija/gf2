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
CAMPAIGNS = SURVEY.parent / "campaigns"
BASELINE_ADDENDA = {"isolated-addendum": "dense-isolated-fused-parity.json",
                    "allocated-addendum": "dense-allocated-matvec.json"}
FAMILIES = ("2037941f-dense-isolated-fused-parity", "2037941f-dense-allocated-matvec")
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

    def run_gate(self, accepted=True, arguments=None):
        result = subprocess.run(
            ["bash", str(self.launcher), "window",
             *(("--cases", str(self.cases)) if arguments is None else arguments)],
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

    def executable_fields(self):
        arm, scalar, tool = (digest(self.root / relative) for relative in EXECUTABLES)
        return f"arm={arm} scalar={scalar} tool={tool}"

    def commit(self):
        subprocess.run(["git", "-C", str(self.root), "add", "-A", "--", str(SURVEY)], check=True)
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=fixture",
                        "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
                        "commit", "-q", "-am", "fixture"], check=True)

    def write_ragged_record(self):
        """The first line of a committed `--cases` session's log."""
        subprocess.run(["git", "-C", str(self.root), "add", self.cases.name], check=True)
        older = (f"{self.executable_fields()} script={digest(self.launcher)} summary={UNRELATED} "
                 f"closure={UNRELATED} cases={digest(self.cases)} "
                 f"addendum={digest(self.addendum)} addendum={digest(self.second_addendum)}")
        self.log.parent.mkdir()
        self.log.write_text(f"identity {older}\nsession start 2026-10-08T23:07:09Z\n")
        return older

    def write_baseline_record(self):
        """The first line of a committed baseline session's log, with its accepted receipts."""
        fields = []
        for name, file_name in BASELINE_ADDENDA.items():
            path = self.root / CAMPAIGNS / file_name
            path.parent.mkdir(exist_ok=True, parents=True)
            path.write_text(json.dumps({"cells": [{"workload": {"seed": 1}}], "name": name}))
            fields.append(f"{name}={digest(path)}")
        for family in FAMILIES:
            receipt = self.root / "dev/bench_results/2037941f" / family / "v4-r1-pilot"
            receipt.mkdir(parents=True)
            (receipt / "receipt.json").write_text("{}")
            (receipt / "acceptance-summary.json").write_text(json.dumps(
                {"verdict": "accepted", "family": {"family_id": family}}))
        subprocess.run(["git", "-C", str(self.root), "add", "-A", "--", str(CAMPAIGNS)], check=True)
        older = (f"{self.executable_fields()} script={digest(self.launcher)} summary={UNRELATED} "
                 f"closure={UNRELATED} {' '.join(fields)}")
        self.log = self.root / "dev/bench_results/2037941f/dense-baseline-profile-r2/execution.log"
        self.log.parent.mkdir(parents=True)
        self.log.write_text(f"identity {older}\n")
        return older

    def test_older_case_session_resumes_with_identical_inputs(self):
        older = self.write_ragged_record()
        self.assertEqual(self.run_gate(), f"identity {older}")

    def test_older_baseline_session_resumes_with_identical_inputs(self):
        older = self.write_baseline_record()
        self.assertEqual(self.run_gate(arguments=()), f"identity {older}")

    def test_older_record_resumes_after_comment_and_formatting_edits(self):
        older = self.write_ragged_record()
        self.commit()
        self.cases.write_text("# Comment only\n" + self.cases.read_text().replace(
            "title=Fixture", "title=Description"))
        self.addendum.write_text(json.dumps(json.loads(self.addendum.read_text()), indent=4))
        self.launcher.write_text(self.launcher.read_text() + "# Documentation only\n")
        self.assertEqual(self.run_gate(), f"identity {older}")

    def test_older_record_resumes_when_addendum_paths_became_file_names(self):
        path_form = self.cases.read_text().replace("|fixture-addendum.json|",
                                                   "|campaigns/fixture-addendum.json|")
        file_name_form = self.cases.read_text()
        self.cases.write_text(path_form)
        older = self.write_ragged_record()
        self.commit()
        self.cases.write_text(file_name_form)
        self.assertEqual(self.run_gate(), f"identity {older}")

    def test_older_record_refuses_measurement_setting_change(self):
        self.write_ragged_record()
        self.commit()
        text = self.launcher.read_text()
        for old, new in (("REPETITIONS=9", "REPETITIONS=10"),
                         ("RECORD_EVENT=cycles:u", "RECORD_EVENT=instructions:u"),
                         ("SAMPLE_FREQUENCY=4999", "SAMPLE_FREQUENCY=5000"),
                         ("taskset -c", "taskset -a -c")):
            with self.subTest(change=new):
                self.launcher.write_text(text.replace(old, new))
                self.run_gate(accepted=False)
                self.launcher.write_text(text)

    def test_older_record_refuses_unrecoverable_script(self):
        older = self.write_ragged_record().replace(f"script={digest(self.launcher)}",
                                                   f"script={UNRELATED}")
        self.log.write_text(f"identity {older}\n")
        self.commit()
        self.run_gate(accepted=False)

    def test_older_record_binds_settings_after_first_resume(self):
        older = self.write_ragged_record()
        self.commit()
        self.assertEqual(self.run_gate(), f"identity {older}")
        self.assertIn("identity arm=", self.log.read_text().splitlines()[-1])
        self.assertIn("configuration=", self.log.read_text().splitlines()[-1])
        self.launcher.write_text(self.launcher.read_text().replace(
            "SAMPLE_FREQUENCY=4999", "SAMPLE_FREQUENCY=5000"))
        self.run_gate(accepted=False)

    def test_control_flow_change_refuses_resume(self):
        self.run_gate()
        self.launcher.write_text(self.launcher.read_text().replace(
            '$(seq 1 "${REPETITIONS}")', '$(seq 2 "${REPETITIONS}")'))
        self.run_gate(accepted=False)

    def test_older_record_refuses_changed_case(self):
        self.write_ragged_record()
        self.commit()
        self.cases.write_text(self.cases.read_text().replace("|route|", "|another-route|"))
        self.run_gate(accepted=False)

    def test_older_record_refuses_changed_addendum(self):
        self.write_ragged_record()
        self.commit()
        self.second_addendum.write_text(json.dumps({"cells": [{"workload": {"seed": 4}}]}))
        self.run_gate(accepted=False)

    def test_older_record_refuses_changed_baseline_addendum(self):
        self.write_baseline_record()
        self.commit()
        (self.root / CAMPAIGNS / BASELINE_ADDENDA["allocated-addendum"]).write_text("{}")
        self.run_gate(accepted=False, arguments=())

    def test_older_record_refuses_each_changed_executable(self):
        self.write_ragged_record()
        for relative in EXECUTABLES:
            with self.subTest(executable=relative):
                binary = self.root / relative
                original = binary.read_bytes()
                binary.write_bytes(original + b"# Different executable bytes\n")
                self.run_gate(accepted=False)
                binary.write_bytes(original)


if __name__ == "__main__":
    unittest.main()
