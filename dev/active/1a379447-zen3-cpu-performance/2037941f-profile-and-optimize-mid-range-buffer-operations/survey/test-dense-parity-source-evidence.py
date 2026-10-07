#!/usr/bin/env python3
"""The dense-parity source-evidence generator leaves its pinned ledger as committed."""

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

GENERATOR = Path(__file__).with_name("make-dense-parity-source-evidence.py")


def git(root, *arguments):
    return subprocess.run(
        ["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t@t", *arguments],
        capture_output=True, check=True, text=True,
    ).stdout.strip()


def load(root=None, claims=None, output=None):
    spec = importlib.util.spec_from_file_location("generator", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if root is not None:
        module.ROOT = root
        module.CLAIMS = claims
        module.OUTPUT = output
    return module


def run_generator(*arguments):
    return subprocess.run(
        ["python3", "-B", str(GENERATOR), *arguments], capture_output=True, text=True
    )


class Synthetic(unittest.TestCase):
    """A repository whose cited lines move after the ledger is committed."""

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        git(self.root, "init", "-q")
        source = "fn a() {}\nfn b() {}\nfn b() {}\n"
        (self.root / "s.rs").write_text(source)
        git(self.root, "add", "s.rs")
        git(self.root, "commit", "-qm", "source")
        commit = git(self.root, "rev-parse", "HEAD")
        self.claims = [("s.rs", "fn a()", 1, "first"), ("s.rs", "fn b()", 2, "second")]
        self.output = Path("ledger.json")
        rows = [
            {
                "project": "gf2", "commit": commit, "path": "s.rs", "line": line,
                "occurrences": occurrences, "verbatim": verbatim,
                "sha256": hashlib.sha256(source.encode()).hexdigest(), "why": why,
            }
            for (_, _, occurrences, why), line, verbatim in zip(
                self.claims, (1, 2), ("fn a() {}", "fn b() {}")
            )
        ]
        document = {
            "schema": "dense-parity-source-evidence-v1",
            "issue": "96c94b81",
            "addendum_identity": "2037941f-dense-parity-v2",
            "claims": rows,
        }
        self.committed = json.dumps(document, indent=2) + "\n"
        (self.root / self.output).write_text(self.committed)
        git(self.root, "add", str(self.output))
        git(self.root, "commit", "-qm", "ledger")
        (self.root / "s.rs").write_text("// moved\n// moved\n" + source)
        git(self.root, "commit", "-qam", "move the cited lines")
        self.module = load(self.root, self.claims, self.output)

    def ledger(self):
        return (self.root / self.output).read_text()

    def test_moved_lines_leave_the_ledger_byte_identical(self):
        self.module.main([])
        self.assertEqual(self.ledger(), self.committed)

    def test_reproduces_the_committed_bytes_from_the_recorded_commits(self):
        self.assertEqual(self.module.reproduce(), self.committed)

    def test_check_passes_on_the_committed_ledger(self):
        self.module.main(["--check"])

    def test_refuses_and_writes_nothing_when_a_claim_differs(self):
        self.module.CLAIMS = [(*self.claims[0][:3], "reworded"), self.claims[1]]
        for arguments in ([], ["--check"]):
            with self.assertRaises(SystemExit) as refused:
                self.module.main(arguments)
            self.assertNotEqual(refused.exception.code, 0)
            self.assertEqual(self.ledger(), self.committed)

    def test_unavailable_recorded_commit_is_an_error(self):
        document = json.loads(self.committed)
        document["claims"][0]["commit"] = "0" * 40
        (self.root / self.output).write_text(json.dumps(document, indent=2) + "\n")
        with self.assertRaises(SystemExit):
            self.module.reproduce()


class Committed(unittest.TestCase):
    """The ledger committed in this tree."""

    def test_generator_leaves_the_committed_ledger_unchanged(self):
        ledger = load().ROOT / load().OUTPUT
        before = ledger.read_bytes()
        self.addCleanup(ledger.write_bytes, before)
        run_generator()
        self.assertEqual(ledger.read_bytes(), before)

    def test_check_reports_no_mismatch_and_writes_nothing(self):
        ledger = load().ROOT / load().OUTPUT
        before = ledger.read_bytes()
        self.addCleanup(ledger.write_bytes, before)
        done = run_generator("--check")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(ledger.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
