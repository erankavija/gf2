"""Self-tests for check.py over fixture trees; the command is in check.py's header."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("check.py")
CONFIG = '[documentation]\nmanaged_paths = ["dev/active", "dev/plans"]\n'


def row(path, disposition, destination="", status="pending", evidence="issue-id", owner="0123abcd", bundle=""):
    return (
        f'{{ path = "{path}", evidence = "{evidence}", owner = "{owner}", bundle = "{bundle}", '
        f'disposition = "{disposition}", destination = "{destination}", status = "{status}" }},'
    )


def policy(value, status):
    return f'{{ key = "documentation.managed_paths", value = "{value}", removal = "89abcdef", status = "{status}" }},'


class CheckTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        for path in ("dev/plans/a.md", "dev/archive/legacy/dev/plans/b.md", "dev/active/e/c.md", "dev/archive/e/d/x.md"):
            (self.root / path).parent.mkdir(parents=True, exist_ok=True)
            (self.root / path).write_text("x\n")
        (self.root / ".jit").mkdir()
        (self.root / ".jit/config.toml").write_text(CONFIG)

    def run_check(self, artifacts, policies=(), *flags):
        manifest = self.root / "manifest.toml"
        body = "\n".join(["artifacts = [", *artifacts, "]", "policy = [", *policies, "]", ""])
        manifest.write_text(body)
        argv = [sys.executable, SCRIPT, "--manifest", manifest, "--root", self.root, *flags]
        result = subprocess.run(argv, capture_output=True, text=True)
        return result.returncode, result.stdout

    def complete_rows(self):
        return [
            row("dev/active/e/c.md", "retained-operational", status="complete"),
            row("dev/active/e/d", "jit-container-archive", "dev/archive/e/d", "complete"),
            row("dev/active/e/d/x.md", "jit-container-archive", "dev/archive/e/d/x.md", "complete", bundle="dev/active/e/d"),
            row("dev/plans/b.md", "legacy-archive", "dev/archive/legacy/dev/plans/b.md", "complete", "none", ""),
            row("dev/sessions/gone.md", "deletion", status="complete"),
        ]

    def test_complete_manifest_passes_required_completeness(self):
        code, out = self.run_check(self.complete_rows(), [policy("dev/sessions", "complete")], "--require-complete")
        self.assertEqual(code, 0, out)
        self.assertIn("jit-container-archive: 2/2 complete", out)
        self.assertIn("policy: 1/1 complete", out)

    def test_incomplete_manifest_reports_progress(self):
        rows = sorted([*self.complete_rows(), row("dev/plans/a.md", "legacy-archive", "dev/archive/legacy/dev/plans/a.md")])
        policies = [policy("dev/plans", "pending")]
        code, out = self.run_check(rows, policies)
        self.assertEqual(code, 0, out)
        self.assertIn("legacy-archive: 1/2 complete", out)
        self.assertIn("policy: 0/1 complete", out)
        code, _ = self.run_check(rows, policies, "--require-complete")
        self.assertEqual(code, 1)

    def test_schema_violations_fail(self):
        cases = {
            "disposition must be one of": [row("dev/plans/a.md", "moved")],
            "legacy-archive destination must be": [row("dev/plans/a.md", "legacy-archive", "dev/archive/legacy/a.md")],
            "owner must be empty exactly when evidence is none": [row("dev/plans/a.md", "deletion", evidence="none")],
            "bundle must name a head row": [row("dev/plans/a.md", "deletion", bundle="dev/plans")],
            "rows must be unique and sorted": [row("dev/plans/a.md", "deletion"), row("dev/active/e/c.md", "deletion")],
            "fields must be exactly": ['{ path = "dev/plans/a.md" },'],
        }
        for message, rows in cases.items():
            with self.subTest(message):
                code, out = self.run_check(rows)
                self.assertEqual(code, 1)
                self.assertIn(message, out)

    def test_failed_assertions_fail(self):
        cases = {
            "pending source is missing": [row("dev/plans/missing.md", "deletion")],
            "complete location dev/archive/legacy/dev/plans/c.md is missing": [
                row("dev/plans/c.md", "legacy-archive", "dev/archive/legacy/dev/plans/c.md", "complete")
            ],
            "complete location dev/plans/c.md is missing": [
                row("dev/plans/c.md", "retained-operational", status="complete"),
                row("dev/plans/c.md", "rewritten-topic", "dev/plans/c.md", "complete"),
            ],
            "complete source still exists": [
                row("dev/plans/a.md", "deletion", status="complete"),
                row("dev/plans/a.md", "retained-operational", "dev/active/e/c.md", "complete"),
            ],
        }
        for message, rows in cases.items():
            for one in rows:
                with self.subTest(message, row=one):
                    code, out = self.run_check([one])
                    self.assertEqual(code, 1)
                    self.assertIn(message, out)
        code, out = self.run_check([], [policy("dev/plans", "complete")])
        self.assertEqual(code, 1)
        self.assertIn("complete entry is present", out)


if __name__ == "__main__":
    unittest.main()
