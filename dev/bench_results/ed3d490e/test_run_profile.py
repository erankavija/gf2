#!/usr/bin/env python3
"""The batch decoder profile launcher's refusals and resumption, timing nothing.

Each test runs a copy of the launcher beside a copy of its build record that
pins a fake driver, through the real acquisition wrapper on private lock files
and the real session script, with a `perf` that only runs the profiled command.
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
RECORD = HERE / "preparation/build-identity.json"
DRIVER = "ldpc-batch-profile"
CASES = ("stat\tsf-dvb-w1\tgf2-batch-single-frame\tdvb-t2-r12\tsingle-core\t8\t1\n"
         "record\tlane-nr-w1\tgf2-batch-lane-avx2\tnr-bg1-z384\tsingle-core\t8\t1\n")
# Driver runs per session: two stat counter groups and one record case.
RUNS_PER_SESSION = 3
REPETITIONS = 9
# Counts its runs; blocks at run BLOCK_AT, fails while FAIL is set, and edits
# its own bytes at run EDIT_AT.
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
        for name in ("FAIL", "BLOCK_AT", "EDIT_AT"):
            self.env.pop(name, None)
        self.pin()

    def pin(self, digest=None):
        """Writes the sandbox build record, pinning `digest` or the fake driver's."""
        record = json.loads(RECORD.read_text(encoding="utf-8"))
        record["executables"][DRIVER]["sha256"] = digest or sha256(self.driver)
        record["isolation"]["target_directory"] = str(self.driver.parent.parent.relative_to(ROOT))
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

    def test_refuses_to_resume_over_a_differing_executable(self):
        log = self.first_session_discarded()
        with self.sandbox.driver.open("a") as driver:
            driver.write("# rebuilt\n")
        self.sandbox.pin()
        result = self.sandbox.run()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertLogFirst(result)
        self.assertIn("changed since the first session", result.stderr)
        self.assertEqual(self.sandbox.log.read_text(), log)

    def test_refuses_to_resume_over_a_differing_case_file(self):
        log = self.first_session_discarded()
        with self.sandbox.cases.open("a") as cases:
            cases.write("# edited\n")
        result = self.sandbox.run()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("changed since the first session", result.stderr)
        self.assertEqual(self.sandbox.log.read_text(), log)

    def test_refuses_a_session_after_the_executable_changes(self):
        result = self.sandbox.run(EDIT_AT=str(RUNS_PER_SESSION))
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn(f"differs from {DRIVER}", result.stderr)
        self.assertEqual(self.sandbox.records(), [["rep-01", "start"], ["rep-01", "done"]])


if __name__ == "__main__":
    unittest.main()
