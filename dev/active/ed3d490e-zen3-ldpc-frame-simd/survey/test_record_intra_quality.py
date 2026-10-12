#!/usr/bin/env python3
"""Check the INTRA quality recorder's comparison and its replay of the committed record.

The replay test needs the built `aff3ct-intra-quality` and is skipped without it.
"""

import importlib.util
import json
import subprocess
import sys
import unittest
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("intra_recorder", SURVEY / "record-intra-quality.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)

QUALITY = {"frame_bit_errors": [0, 2, 0], "frames": 3, "memory_bytes": 7, "latency_ns_p50": 9}


class DifferencesTest(unittest.TestCase):
    def test_timing_and_memory_diagnostics_never_differ(self):
        self.assertEqual(recorder.differences(QUALITY, {**QUALITY, "memory_bytes": 1,
                                                        "latency_ns_p50": 2}), [])

    def test_a_changed_frame_is_named(self):
        changed = {**QUALITY, "frame_bit_errors": [0, 3, 0]}
        self.assertEqual(recorder.differences(changed, QUALITY), ["frame_bit_errors"])

    def test_a_missing_field_differs(self):
        shorter = {name: value for name, value in QUALITY.items() if name != "frames"}
        self.assertEqual(recorder.differences(shorter, QUALITY), ["frames"])

    def test_the_record_carries_zero_diagnostics(self):
        self.assertEqual({name: recorder.untimed(QUALITY)[name] for name in recorder.DIAGNOSTICS},
                         dict.fromkeys(recorder.DIAGNOSTICS, 0))


class HostFactsTest(unittest.TestCase):
    def test_a_cpuinfo_block_binds_model_and_flags(self):
        text = ("processor\t: 0\nmodel name\t: Test CPU @ 1GHz\nflags\t\t: fpu avx2 fma sse\n\n"
                "processor\t: 1\nmodel name\t: Other\nflags\t\t: fpu\n")
        self.assertEqual(recorder.host_facts(text),
                         {"cpu_model": "Test CPU @ 1GHz", "avx2": True, "fma": True})

    def test_absent_flags_are_false(self):
        facts = recorder.host_facts("model name\t: X\nflags\t\t: fpu\n")
        self.assertEqual((facts["avx2"], facts["fma"]), (False, False))


class CommittedRecordTest(unittest.TestCase):
    def test_a_second_replay_reproduces_the_committed_record(self):
        executable = recorder.smoke.ROOT / recorder.smoke.release() / recorder.EXECUTABLE
        if not executable.exists():
            self.skipTest("the arms workspace is not built")
        records = [recorder.smoke.ROOT / path for path in recorder.smoke.listed(recorder.RECORD)
                   if Path(path).parent.name == "quality-intra"]
        records = [path.parent for path in records]
        self.assertEqual(len(records), 1)
        run = subprocess.run([sys.executable, "-B", str(SURVEY / "record-intra-quality.py"),
                              "check", str(records[0])], cwd=recorder.smoke.ROOT,
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stdout)


if __name__ == "__main__":
    unittest.main()
