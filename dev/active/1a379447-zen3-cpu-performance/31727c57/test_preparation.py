#!/usr/bin/env python3
"""Check prepared families, arm identities and the window guard without timing.

Usage: python3 test_preparation.py
"""

import os
import subprocess
import sys
import unittest

sys.dont_write_bytecode = True
import prepare  # noqa: E402


class Preparation(unittest.TestCase):
    def test_family_contracts(self):
        prepare.self_test()

    def test_core_build_bytes(self):
        builder = prepare.load("31727c57/build-core.py", "core_builder")
        self.assertEqual(builder.document(False),
                         prepare.read(prepare.HERE / "core/arm-builds.json"))

    def test_launchers_require_window(self):
        environment = dict(os.environ)
        environment.pop("GF2_BENCH_WINDOW", None)
        for family in ("core", "decoder"):
            command = ["bash", str((prepare.HERE / family / "window.sh").relative_to(prepare.ROOT))]
            result = subprocess.run(command, env=environment, capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("timed runs require the overnight benchmark window", result.stderr)

    def test_batch_fill_is_reported_separately(self):
        diagnostics = prepare.read(prepare.HERE / "decoder/untimed-diagnostics.json")
        expected = sum(len(cell["arms"]) for phase in ("pilot", "confirmation")
                       for cell in prepare.read(prepare.HERE / "decoder" /
                                                f"{phase}-untimed-smoke.json")["cells"])
        self.assertEqual(len(diagnostics), expected)
        for row in diagnostics:
            self.assertIn("batch_fill_ns", row["batch-decoder-diagnostics"]["conversion"])


if __name__ == "__main__":
    unittest.main()
