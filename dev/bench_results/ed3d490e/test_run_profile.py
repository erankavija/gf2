#!/usr/bin/env python3
"""The profile series launchers' refusals and resumption, timing nothing.

A launcher copy runs beside a build record copy that pins a fake driver and
sandbox quality files, through the real acquisition wrapper on private lock
files and the real session script, with a `perf` that only runs its command.
"""

import hashlib
import json
import os
import shutil
import signal
import subprocess
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(subprocess.run(["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
                           check=True, capture_output=True, text=True).stdout.strip())
LAUNCHER = HERE / "run-profile.sh"

def located(name):
    """Root-relative path `repository_files.py file` prints for `name`."""
    def run(*command):
        return subprocess.run(command, cwd=ROOT, check=True, capture_output=True,
                              text=True).stdout.strip()
    files = run("git", "ls-files", "--", ":(glob)**/repository_files.py",
                ":(exclude,glob)**/inputs/**")
    return Path(run("python3", "-B", files, "file", name))


RESAMPLE = located("run-profile-resample.sh")
RECORD = HERE / "preparation/build-identity.json"
LANE_ARM = "gf2-batch-lane-avx2"
DRIVER = "ldpc-batch-profile"
CASES = ("stat\tsf-dvb-w1\tgf2-batch-single-frame\tdvb-t2-r12\tsingle-core\t8\t1\n"
         "record\tlane-nr-w1\tgf2-batch-lane-avx2\tnr-bg1-z384\tsingle-core\t8\t1\n")
# Two stat counter groups and one record case.
RUNS_PER_SESSION = 3
REPETITIONS = 9
FAKE_DRIVER = """#!/bin/sh
n=$(( $(cat "$COUNTER" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$COUNTER"
if [ "$n" = "${EDIT_AT:-0}" ]; then echo "# edited" >> "$0"; fi
if [ "$n" = "${BLOCK_AT:-0}" ]; then : > "$REACHED"; sleep 600; fi
[ -z "${FAIL:-}" ] || exit 1
echo '{}'
"""
FAKE_PERF = """#!/bin/sh
case "$1" in
    stat|record) while [ "$1" != "--" ]; do shift; done; shift; exec "$@" ;;
esac
"""


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Sandbox:
    def __init__(self, directory):
        self.directory = directory
        self.bench = directory / "bench"
        (self.bench / "preparation").mkdir(parents=True)
        shutil.copy2(LAUNCHER, self.bench / LAUNCHER.name)
        self.driver = directory / "arms/release" / DRIVER
        self.driver.parent.mkdir(parents=True)
        self.driver.write_text(FAKE_DRIVER)
        self.driver.chmod(0o755)
        tools = directory / "path"
        tools.mkdir()
        (tools / "perf").write_text(FAKE_PERF)
        (tools / "perf").chmod(0o755)
        self.cases = directory / "cases.tsv"
        self.cases.write_text(CASES)
        self.out = directory / "out"
        self.log = self.out / "repetitions.log"
        self.reached = directory / "reached"
        self.env = {**os.environ, "PATH": f"{tools}:{os.environ['PATH']}",
                    "GF2_BENCH_WINDOW": "1",
                    "GF2_CCX1_LOCK": str(directory / "lock"),
                    "GF2_CCX1_TURNSTILE": str(directory / "turnstile"),
                    "COUNTER": str(directory / "runs"), "REACHED": str(self.reached)}
        for name in ("FAIL", "BLOCK_AT", "EDIT_AT", "GF2_LDPC_LANE_BACKEND"):
            self.env.pop(name, None)
        self.quality = self.prepared_quality()
        self.pin()

    def prepared_quality(self):
        """Copies the record of pinned quality files and the files it names into the sandbox."""
        build = json.loads(RECORD.read_text(encoding="utf-8"))
        source = ROOT / build["arms"]["aff3ct-flooding-nms-f32"]["pinned_by"]["record"]
        pinned = json.loads(source.read_text(encoding="utf-8"))
        directory = self.directory / "quality"
        directory.mkdir()
        files = {}
        for path, digest in pinned["prepared_quality"].items():
            shutil.copy2(ROOT / path, directory)
            files[str((directory / Path(path).name).relative_to(ROOT))] = digest
        pinned["prepared_quality"] = files
        self.pinned_quality = directory / source.name
        self.pinned_quality.write_text(json.dumps(pinned))
        return directory

    def pin(self, digest=None, edit=lambda record: None):
        """Writes the sandbox build record, pinning `digest` or the fake driver's."""
        record = json.loads(RECORD.read_text(encoding="utf-8"))
        record["executables"][DRIVER]["sha256"] = digest or sha256(self.driver)
        record["isolation"]["target_directory"] = str(self.driver.parent.parent.relative_to(ROOT))
        for row in record["arms"].values():
            if row.get("pinned_by"):
                row["pinned_by"]["record"] = str(self.pinned_quality.relative_to(ROOT))
        edit(record)
        (self.bench / "preparation" / RECORD.name).write_text(json.dumps(record))

    def command(self):
        return [str(self.bench / LAUNCHER.name), str(self.cases), str(self.out)]

    def run(self, **env):
        return subprocess.run(self.command(), cwd=ROOT, env={**self.env, **env},
                              capture_output=True, text=True, timeout=120)

    def records(self):
        return [line.split()[:2] for line in self.log.read_text().splitlines()]


class ProfileLauncherTest(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / "target/run-profile-test"
        scratch.mkdir(parents=True, exist_ok=True)
        self.sandbox = Sandbox(Path(tempfile.mkdtemp(dir=scratch)))
        self.addCleanup(shutil.rmtree, self.sandbox.directory)

    def assertLogFirst(self, result):
        self.assertEqual(result.stderr.splitlines()[0],
                         f"profile execution log: {self.sandbox.log}")

    def test_refuses_an_executable_differing_from_the_build_record(self):
        self.sandbox.pin("0" * 64)
        result = self.sandbox.run()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertLogFirst(result)
        self.assertIn(f"differs from {DRIVER}", result.stderr)
        self.assertEqual(self.sandbox.records(), [])

    def test_refuses_a_case_naming_no_batch_arm(self):
        self.sandbox.cases.write_text(
            "stat\taff3ct-dvb-w1\taff3ct-flooding-nms-f32\tdvb-t2-r12\tsingle-core\t8\t8\n")
        result = self.sandbox.run()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("is not a gf2 batch arm", result.stderr)
        self.assertEqual(self.sandbox.records(), [])

    def interrupt_at_session(self, session):
        """Kills the launcher's process group inside `session`."""
        launcher = subprocess.Popen(
            self.sandbox.command(), cwd=ROOT, start_new_session=True,
            env={**self.sandbox.env, "BLOCK_AT": str((session - 1) * RUNS_PER_SESSION + 1)},
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 60
        while not self.sandbox.reached.exists():
            self.assertIsNone(launcher.poll(), "the launcher exited before the blocking run")
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.1)
        os.killpg(launcher.pid, signal.SIGKILL)
        launcher.wait()

    def test_resumes_after_the_last_completed_session(self):
        self.interrupt_at_session(3)
        self.assertEqual(self.sandbox.records(), [
            ["rep-01", "start"], ["rep-01", "done"],
            ["rep-02", "start"], ["rep-02", "done"], ["rep-03", "start"]])

        failed = self.sandbox.run(FAIL="1")
        self.assertEqual(failed.returncode, 1, failed.stderr)
        self.assertLogFirst(failed)
        self.assertEqual(self.sandbox.records()[5:], [
            ["rep-03", "discarded"], ["rep-03", "start"], ["rep-03", "discarded"]])
        log = self.sandbox.log.read_text()
        self.assertIn("rep-03 discarded", log.splitlines()[5])
        self.assertIn("unfinished start(s)", log.splitlines()[5])
        self.assertIn("every profiled case failed", log.splitlines()[7])

        finished = self.sandbox.run()
        self.assertEqual(finished.returncode, 0, finished.stderr)
        self.assertLogFirst(finished)
        records = self.sandbox.records()
        starts = Counter(rep for rep, kind in records if kind == "start")
        done = [rep for rep, kind in records[:-1] if kind == "done"]
        self.assertEqual(done, [f"rep-{index:02d}" for index in range(1, REPETITIONS + 1)])
        self.assertEqual((starts["rep-01"], starts["rep-02"], starts["rep-03"]), (1, 1, 3))
        self.assertEqual(records[-1], ["series", "done"])

    def first_session_discarded(self):
        result = self.sandbox.run(FAIL="1")
        self.assertEqual(result.returncode, 1, result.stderr)
        return self.sandbox.log.read_text()

    def assertResumeRefused(self, change, **env):
        log = self.first_session_discarded()
        change()
        result = self.sandbox.run(**env)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertLogFirst(result)
        self.assertIn("changed since the first session", result.stderr)
        self.assertEqual(self.sandbox.log.read_text(), log)

    def test_refuses_to_resume_over_a_differing_executable(self):
        def rebuild():
            with self.sandbox.driver.open("a") as driver:
                driver.write("# rebuilt\n")
            self.sandbox.pin()
        self.assertResumeRefused(rebuild)

    def test_refuses_to_resume_over_a_differing_case_file(self):
        def edit():
            with self.sandbox.cases.open("a") as cases:
                cases.write("# edited\n")
        self.assertResumeRefused(edit)

    def test_refuses_to_resume_over_a_differing_route(self):
        def reroute(record):
            record["arms"][LANE_ARM]["executable"] = "gf2-batch-single-frame-arm"
        self.assertResumeRefused(lambda: self.sandbox.pin(edit=reroute))

    def test_refuses_to_resume_over_a_differing_backend_environment(self):
        def portable(record):
            record["arms"][LANE_ARM]["environment"] = {"GF2_LDPC_LANE_BACKEND": "portable"}
        self.assertResumeRefused(lambda: self.sandbox.pin(edit=portable))

    def test_refuses_to_resume_over_a_differing_effective_inherited_variable(self):
        self.assertResumeRefused(lambda: None, GF2_LDPC_LANE_BACKEND="portable")

    def assertResumes(self, result):
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(self.sandbox.records()[-2:], [["rep-01", "start"], ["rep-01", "discarded"]])

    def test_resumes_over_an_inherited_variable_the_session_overrides(self):
        self.first_session_discarded()
        self.assertResumes(self.sandbox.run(FAIL="1", RAYON_NUM_THREADS="7"))

    def test_resumes_over_an_execution_log_longer_than_a_pipe_buffer(self):
        log = self.first_session_discarded()
        with self.sandbox.log.open("a") as appended:
            appended.write(log * 2000)
        self.assertGreater(self.sandbox.log.stat().st_size, 1 << 20)
        self.assertResumes(self.sandbox.run(FAIL="1"))

    def test_refuses_to_resume_over_a_differing_quality_file(self):
        def edit():
            with (self.sandbox.quality / "gf2-nms-f32-nr-bg1-z384.json").open("a") as quality:
                quality.write("\n")
        self.assertResumeRefused(edit)

    def test_refuses_to_resume_over_a_differing_bundle(self):
        expanded = (self.sandbox.out / "cases.expanded.tsv")
        def edit():
            manifest = Path(expanded.read_text().splitlines()[0].split("\t")[3]) / "manifest.json"
            original = manifest.read_bytes()
            self.addCleanup(manifest.write_bytes, original)
            manifest.write_bytes(original + b"\n")
        self.assertResumeRefused(edit)

    def test_refuses_a_session_after_the_executable_changes(self):
        result = self.sandbox.run(EDIT_AT=str(RUNS_PER_SESSION))
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn(f"differs from {DRIVER}", result.stderr)
        self.assertEqual(self.sandbox.records(), [["rep-01", "start"], ["rep-01", "done"]])

    def test_resample_refuses_an_executable_differing_from_its_record(self):
        bench = self.sandbox.directory / "resample"
        (bench / "preparation").mkdir(parents=True)
        launcher = bench / RESAMPLE.name
        shutil.copy2(ROOT / RESAMPLE, launcher)
        shutil.copy2(ROOT / RESAMPLE.parent / "preparation/build-identity.json", bench / "preparation")
        source = ROOT / RESAMPLE.parent / "preparation/kernel-build-identity.json"
        record = json.loads(source.read_text(encoding="utf-8"))
        pinned = record["profile_resample"]["executables"]
        record["profile_resample"]["executables"] = {name: "0" * 64 for name in pinned}
        (bench / "preparation" / source.name).write_text(json.dumps(record))
        result = subprocess.run([str(launcher), str(self.sandbox.out)], cwd=ROOT,
                                env=self.sandbox.env, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertLogFirst(result)
        self.assertIn(f"differs from {bench / 'preparation' / source.name}", result.stderr)
        self.assertEqual(self.sandbox.records(), [])


if __name__ == "__main__":
    unittest.main()
