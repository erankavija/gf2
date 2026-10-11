#!/usr/bin/env python3
"""Check the arms build identity recorder's refusals and its reproduction.

The reproduction test builds the arms workspace and needs GF2_AFF3CT_ROOT,
the staged AFF3CT tree; it is skipped without one.
"""

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
RECORDER = SURVEY / "record-gf2-arms-build.py"
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(SURVEY), "rev-parse", "--show-toplevel"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
)

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("arms_build_recorder", RECORDER)
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)

RELEASE = Path("arms/target/release")
BUILT = {"built-arm": "a" * 64}
PIN = {"record": "campaign/build-identity.json", "executable": "pinned-arm", "sha256": "b" * 64}
RECORDS = {PIN["record"]: {"sha256": "c" * 64, "pins": []}}


def workspace_arm(executable):
    return {"source": "workspace", "package": "arms", "executable": executable, "environment": {}}


def pinned_arm(digest):
    return {"source": "pinned", "executable": PIN["executable"], "executable_sha256": digest,
            "pinned_by": {"record": "build-identity.json", "sha256": "c" * 64},
            "environment": {}}


def arm_rows(named, found_at):
    return recorder.arm_rows(named, [{**PIN, "found_at": found_at}], RECORDS, BUILT, RELEASE)


class ArmRowsTest(unittest.TestCase):
    def test_every_catalogued_arm_gets_its_executable_digest(self):
        rows = arm_rows({"built": workspace_arm("built-arm"), "pinned": pinned_arm(PIN["sha256"])},
                        ["target/campaign/release/pinned-arm"])
        self.assertEqual(rows["built"]["executable_sha256"], BUILT["built-arm"])
        self.assertEqual(rows["built"]["path"], str(RELEASE / "built-arm"))
        self.assertEqual(rows["pinned"]["executable_sha256"], PIN["sha256"])
        self.assertEqual(rows["pinned"]["path"], "target/campaign/release/pinned-arm")
        self.assertEqual(rows["pinned"]["pinned_by"],
                         {"record": PIN["record"], "sha256": "c" * 64})

    def test_a_workspace_arm_the_build_does_not_produce_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "absent"):
            arm_rows({"absent": workspace_arm("absent-arm")}, [])

    def test_a_pinned_arm_no_target_file_carries_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "pinned"):
            arm_rows({"pinned": pinned_arm(PIN["sha256"])}, [])

    def test_a_pinned_arm_no_record_pins_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "pinned"):
            arm_rows({"pinned": pinned_arm("d" * 64)}, ["target/campaign/release/pinned-arm"])


@unittest.skipUnless(os.environ.get("GF2_AFF3CT_ROOT"), "needs the staged AFF3CT tree")
class ReproductionTest(unittest.TestCase):
    def test_a_second_run_over_an_unchanged_build_reproduces_the_record(self):
        records = []
        for _ in range(2):
            with tempfile.TemporaryDirectory() as out_dir:
                subprocess.run(
                    [sys.executable, "-B", str(RECORDER), out_dir, os.environ["GF2_AFF3CT_ROOT"]],
                    cwd=ROOT, check=True, capture_output=True)
                records.append((Path(out_dir) / "build-identity.json").read_bytes())
        self.assertEqual(records[0], records[1])


if __name__ == "__main__":
    unittest.main()
