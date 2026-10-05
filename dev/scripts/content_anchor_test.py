"""Self-tests for content_anchor.py over fixture repositories.

Run from any directory:
    python3 content_anchor_test.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

from content_anchor import Anchor, history_blob, sha256  # noqa: E402

PACKAGE = "crates/pkg"
SOURCE = f"{PACKAGE}/src/lib.rs"


class Fixture(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Fixture")
        self.commit(SOURCE, b"fn one() {}\n")
        self.commit(f"{PACKAGE}/src/other.rs", b"fn other() {}\n")

    def git(self, *arguments: str) -> None:
        subprocess.run(["git", "-C", str(self.root), *arguments], check=True, capture_output=True)

    def commit(self, path: str, content: bytes) -> None:
        (self.root / path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / path).write_bytes(content)
        self.git("add", path)
        self.git("commit", "-q", "-m", f"change {path}")

    def pinned(self) -> Anchor:
        anchor = Anchor(self.root, self.root / "end-state.json")
        anchor.freeze("HEAD", [PACKAGE], "fixture-v1", "fixture")
        return anchor


class HistoryBlob(Fixture):
    def test_an_overwritten_committed_content_is_found_by_digest(self):
        self.commit(SOURCE, b"fn two() {}\n")
        self.assertEqual(
            history_blob(self.root, SOURCE, sha256(b"fn one() {}\n")), b"fn one() {}\n"
        )

    def test_an_uncommitted_working_tree_content_is_not_found(self):
        (self.root / SOURCE).write_bytes(b"fn draft() {}\n")
        self.assertIsNone(history_blob(self.root, SOURCE, sha256(b"fn draft() {}\n")))

    def test_a_digest_the_path_never_held_is_not_found(self):
        self.assertIsNone(history_blob(self.root, SOURCE, sha256(b"fn other() {}\n")))


class AnchorBytes(Fixture):
    def test_a_path_without_a_snapshot_reads_the_committed_blob_after_it_changes(self):
        anchor = self.pinned()
        self.commit(SOURCE, b"fn two() {}\n")
        self.assertEqual(anchor.bytes(SOURCE), b"fn one() {}\n")

    def test_a_digest_no_blob_holds_exits(self):
        anchor = self.pinned()
        (self.root / "end-state.json").write_text(
            (self.root / "end-state.json").read_text().replace(sha256(b"fn one() {}\n"), "0" * 64)
        )
        with self.assertRaisesRegex(SystemExit, "no file holds the baseline digest"):
            anchor.bytes(SOURCE)


class RequireMatchingTree(Fixture):
    def test_the_recorded_tree_is_accepted(self):
        self.pinned().require_matching_tree("fixture record")

    def test_a_changed_file_is_refused_by_name(self):
        anchor = self.pinned()
        self.commit(SOURCE, b"fn two() {}\n")
        with self.assertRaisesRegex(SystemExit, SOURCE):
            anchor.require_matching_tree("fixture record")

    def test_an_added_file_is_refused_by_name(self):
        anchor = self.pinned()
        self.commit(f"{PACKAGE}/src/added.rs", b"fn added() {}\n")
        with self.assertRaisesRegex(SystemExit, "added.rs"):
            anchor.require_matching_tree("fixture record")


if __name__ == "__main__":
    unittest.main()
