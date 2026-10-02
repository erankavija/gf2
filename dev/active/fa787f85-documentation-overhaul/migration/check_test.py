"""Self-tests for check.py over fixture trees; the command is in check.py's header."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("check.py")
CONFIG = '[documentation]\nmanaged_paths = ["dev/active", "dev/plans"]\n'


OWNED = {"evidence": "issue-id", "citation": "0123abcd", "owners": ["0123abcd"], "epic": "0123abcd"}
OWNERLESS = {"evidence": "none", "citation": "", "owners": [], "epic": ""}


def row(path, disposition, destination="", status="pending", **fields):
    values = {"path": path, **OWNED, "bundle": "", "disposition": disposition, "destination": destination}
    values |= {"digest_pinned": False, "consumers": [], "inbound": [], "status": status} | fields
    return "{ " + ", ".join(f"{k} = {json.dumps(v)}" for k, v in values.items()) + " },"


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
            row("dev/active/e/c.md", "retained-operational", status="complete", evidence="commit", citation="5b91b07",
                owners=["0123abcd", "456789ab"], digest_pinned=True, consumers=["src/a.rs:12"], inbound=["README.md:3"]),
            row("dev/active/e/d", "jit-container-archive", "dev/archive/e/d", "complete"),
            row("dev/active/e/d/x.md", "jit-container-archive", "dev/archive/e/d/x.md", "complete", bundle="dev/active/e/d"),
            row("dev/plans/b.md", "legacy-archive", "dev/archive/legacy/dev/plans/b.md", "complete", **OWNERLESS),
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

    def test_empty_manifest_fails_required_completeness(self):
        self.assertEqual(self.run_check([])[0], 0)
        self.assertEqual(self.run_check([], (), "--require-complete")[0], 1)

    def test_schema_violations_fail(self):
        cases = {
            "disposition must be one of": [row("dev/plans/a.md", "moved")],
            "legacy-archive destination must be": [row("dev/plans/a.md", "legacy-archive", "dev/archive/legacy/a.md")],
            "citation, owners and epic must be empty exactly when evidence is none": [
                row("dev/plans/a.md", "deletion", evidence="none"),
                row("dev/plans/a.md", "deletion", epic=""),
            ],
            "citation must be a commit sha or one of owners": [
                row("dev/plans/a.md", "deletion", evidence="commit", citation="xyz"),
                row("dev/plans/a.md", "deletion", evidence="doc-ref", citation="89abcdef"),
            ],
            "owners and epic must be 8-hex short ids": [
                row("dev/plans/a.md", "deletion", owners=["0123abcd", "0123"]),
                row("dev/plans/a.md", "deletion", epic="EPIC0123"),
            ],
            "consumers and inbound must be file:line references": [
                row("dev/plans/a.md", "deletion", consumers=["src/a.rs"]),
                row("dev/plans/a.md", "deletion", inbound=["README.md:x"]),
            ],
            "wrong type for digest_pinned": [row("dev/plans/a.md", "deletion", digest_pinned="yes")],
            "wrong type for owners": [row("dev/plans/a.md", "deletion", owners="0123abcd")],
            "bundle must name a head row": [row("dev/plans/a.md", "deletion", bundle="dev/plans")],
            "fields must be exactly": ['{ path = "dev/plans/a.md" },'],
            "destination must be repository-relative": [
                row("dev/active/e/c.md", "jit-container-archive", "dev/archive/../c.md"),
                row("dev/plans/../a.md", "legacy-archive", "dev/archive/legacy/dev/plans/../a.md"),
            ],
        }
        for message, rows in cases.items():
            for one in rows:
                with self.subTest(message, row=one):
                    code, out = self.run_check([one])
                    self.assertEqual(code, 1)
                    self.assertIn(message, out)
        code, out = self.run_check([row("dev/plans/a.md", "deletion"), row("dev/active/e/c.md", "deletion")])
        self.assertIn("rows must be unique and sorted", out)

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
