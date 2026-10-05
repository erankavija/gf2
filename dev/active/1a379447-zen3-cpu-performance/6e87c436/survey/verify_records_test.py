"""Self-tests for the verdict's record check and the generators' refusal (jit:66698c3c).

Run from any directory:
    python3 verify_records_test.py
"""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

SURVEY = Path(__file__).resolve().parent
sys.path.insert(0, str(SURVEY))
from locate import ROOT, repository_files  # noqa: E402

SHARED = ("content_anchor", "repository_files", "rust_code_text", "asm_listing")
SCRIPTS = (
    "locate", "make-production-drift", "make-asm-comparison", "freeze-end-state", "verify-records"
)


def run(command, cwd):
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True)


class CommittedRecords(unittest.TestCase):
    def check(self, *arguments):
        return run([sys.executable, "-B", str(SURVEY / "verify-records.py"), *arguments], SURVEY)

    def test_the_committed_records_verify_and_nothing_is_written(self):
        def digest():
            return hashlib.sha256(
                subprocess.run(
                    ["git", "-C", str(ROOT), "status", "--porcelain=v1", "-z"], capture_output=True
                ).stdout
                + b"".join(p.read_bytes() for p in sorted(SURVEY.iterdir()) if p.is_file())
            ).hexdigest()

        before = digest()
        done = self.check()
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(digest(), before)

    def mutated(self, name, edit):
        with tempfile.TemporaryDirectory() as directory:
            record = json.loads((SURVEY / name).read_bytes())
            edit(record)
            copy = Path(directory) / name
            copy.write_text(json.dumps(record))
            option = "--drift" if name.startswith("production") else "--assembly"
            return self.check(option, str(copy))

    def test_a_flipped_file_class_fails_by_path(self):
        def edit(record):
            row = next(f for f in record["baselines"][0]["files"] if f["class"] == "code-differs")
            row["class"] = "comment-or-blank-only"
            self.path = row["path"]

        done = self.mutated("production-drift.json", edit)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn(self.path, done.stderr)

    def test_an_unpinned_current_digest_fails_by_path(self):
        def edit(record):
            row = record["baselines"][0]["files"][0]
            row["current_sha256"] = "0" * 64
            self.path = row["path"]

        done = self.mutated("production-drift.json", edit)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn(self.path, done.stderr)

    def test_a_changed_symbol_row_fails_by_artefact(self):
        def edit(record):
            record["modules"][0]["symbols"][0]["instruction_text"] = "differs"
            self.path = record["modules"][0]["artefact"]

        done = self.mutated("asm-comparison.json", edit)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn(self.path, done.stderr)


class RegenerationRefusal(unittest.TestCase):
    """The generators on a fixture tree whose baselined file changed after the freeze."""

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.survey = self.root / "survey"
        self.survey.mkdir()
        (self.root / "scripts").mkdir()
        for module in SHARED:
            shutil.copy(ROOT / repository_files.live_file(ROOT, f"{module}.py"), self.root / "scripts")
        shutil.copy(
            ROOT / repository_files.live_file(ROOT, "repo_artifacts.py"), self.survey
        )
        for script in SCRIPTS:
            shutil.copy(SURVEY / f"{script}.py", self.survey)
        for package in ("gf2-core", "gf2-kernels-simd"):
            manifest = self.root / "crates" / package / "Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(f'[package]\nname = "{package}"\n')
            (manifest.parent / "src").mkdir()
            (manifest.parent / "src" / "lib.rs").write_text("fn one() {}\n")
        self.sentinels = {
            "production-drift.json": '{"baselines": []}\n',
            "asm-comparison.json": '{"modules": []}\n',
        }
        for name, text in self.sentinels.items():
            (self.survey / name).write_text(text)
        run(["git", "init", "-q"], self.root)
        run(["git", "config", "user.email", "fixture@example.invalid"], self.root)
        run(["git", "config", "user.name", "Fixture"], self.root)
        self.commit("fixture")
        done = run([sys.executable, "-B", "survey/freeze-end-state.py", "HEAD"], self.root)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.commit("end state")

    def commit(self, message):
        run(["git", "add", "-A"], self.root)
        run(["git", "commit", "-q", "-m", message], self.root)

    def regenerate(self, generator):
        return run([sys.executable, "-B", f"survey/{generator}.py"], self.root)

    def test_a_changed_baselined_file_refuses_and_leaves_the_record_bytes(self):
        (self.root / "crates/gf2-core/src/lib.rs").write_text("fn two() {}\n")
        self.commit("later change")
        for generator, record in (
            ("make-production-drift", "production-drift.json"),
            ("make-asm-comparison", "asm-comparison.json"),
        ):
            done = self.regenerate(generator)
            self.assertNotEqual(done.returncode, 0)
            self.assertIn("crates/gf2-core/src/lib.rs", done.stderr)
            self.assertEqual((self.survey / record).read_text(), self.sentinels[record])

    def test_the_pinned_tree_does_not_refuse(self):
        for generator in ("make-production-drift", "make-asm-comparison"):
            self.assertNotIn("describes the tree pinned", self.regenerate(generator).stderr)


if __name__ == "__main__":
    unittest.main()
