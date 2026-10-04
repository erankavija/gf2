"""Self-tests for repository_files.py over fixture repositories.

Run from any directory:
    python3 repository_files_test.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

from repository_files import (  # noqa: E402
    PROTOCOL_OPENING,
    SHARED_PRODUCING_MANIFEST,
    SNAPSHOT_DIRECTORY,
    document,
    package_directory,
    shared_producing_manifest,
)

HELPER = Path(__file__).with_name("repository_files.py")


class Fixture(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        subprocess.run(["git", "-C", str(self.root), "init", "-q"], check=True)

    def write(self, path: str, content: bytes) -> None:
        (self.root / path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / path).write_bytes(content)

    def entry(self, directory: str, manifest: bytes | None) -> None:
        """A protocol document in `directory`, with `manifest` beside it."""
        self.write(f"{directory}/protocol.md", PROTOCOL_OPENING + b"1. Fixture.\n")
        if manifest is not None:
            self.write(f"{directory}/{SHARED_PRODUCING_MANIFEST}", manifest)


class SharedProducingManifest(Fixture):
    def test_one_manifest_is_located_beside_the_protocol_document(self):
        self.entry("b/entry", b"{}\n")
        self.write(f"a/family/{SHARED_PRODUCING_MANIFEST}", b"{}\n")
        self.write("a/family/protocol.md", b"# Another document\n")
        self.assertEqual(
            shared_producing_manifest(self.root), f"b/entry/{SHARED_PRODUCING_MANIFEST}"
        )

    def test_byte_identical_copies_are_one_named_by_the_first_path(self):
        self.entry("b/entry", b"{}\n")
        self.entry("a/copy", b"{}\n")
        self.assertEqual(
            shared_producing_manifest(self.root), f"a/copy/{SHARED_PRODUCING_MANIFEST}"
        )

    def test_snapshot_copy_of_the_protocol_document_is_not_live(self):
        self.entry("entry", b"{}\n")
        self.entry(f"receipt/{SNAPSHOT_DIRECTORY}", b'{"other": true}\n')
        self.assertEqual(
            shared_producing_manifest(self.root), f"entry/{SHARED_PRODUCING_MANIFEST}"
        )

    def test_two_different_manifests_are_rejected(self):
        self.entry("a", b"{}\n")
        self.entry("b", b'{"other": true}\n')
        with self.assertRaisesRegex(LookupError, "^2 distinct"):
            shared_producing_manifest(self.root)

    def test_absent_manifest_is_rejected(self):
        self.entry("a", None)
        with self.assertRaisesRegex(LookupError, "^0 distinct"):
            shared_producing_manifest(self.root)


class Document(Fixture):
    def test_document_is_located_by_name_and_opening(self):
        self.write("x/2026-01-01-record.md", b"# Wanted\n\nBody.\n")
        self.write("y/2026-01-02-record.md", b"# Other\n")
        self.write("z/notes.md", b"# Wanted\n")
        self.assertEqual(document(self.root, "*-record.md", b"# Wanted\n"), "x/2026-01-01-record.md")

    def test_byte_identical_copies_are_one_named_by_the_first_path(self):
        self.write("b/record.md", b"# Wanted\n")
        self.write("a/record.md", b"# Wanted\n")
        self.assertEqual(document(self.root, "record.md", b"# Wanted\n"), "a/record.md")

    def test_snapshot_copy_is_not_live(self):
        self.write("entry/record.md", b"# Wanted\n")
        self.write(f"receipt/{SNAPSHOT_DIRECTORY}/record.md", b"# Wanted\n\nSnapshot.\n")
        self.assertEqual(document(self.root, "record.md", b"# Wanted\n"), "entry/record.md")

    def test_absent_and_differing_documents_are_rejected(self):
        self.write("a/record.md", b"# Twice\n\nFirst.\n")
        self.write("b/record.md", b"# Twice\n\nSecond.\n")
        with self.assertRaisesRegex(LookupError, "^2 distinct"):
            document(self.root, "record.md", b"# Twice\n")
        with self.assertRaisesRegex(LookupError, "^0 distinct"):
            document(self.root, "record.md", b"# Absent\n")


class PackageDirectory(Fixture):
    def test_package_is_located_by_its_declared_name(self):
        self.write("x/crate/Cargo.toml", b'[package]\nname = "wanted"\n')
        self.write("y/crate/Cargo.toml", b'[package]\nname = "other"\n')
        self.assertEqual(package_directory(self.root, "wanted"), "x/crate")

    def test_absent_and_repeated_names_are_rejected(self):
        self.write("x/Cargo.toml", b'[package]\nname = "twice"\n')
        self.write("y/Cargo.toml", b'[package]\nname = "twice"\n')
        for name in ("twice", "absent"):
            with self.assertRaises(LookupError):
                package_directory(self.root, name)


class CommandLine(unittest.TestCase):
    def run_helper(self, *arguments: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, "-B", str(HELPER), *arguments], capture_output=True, text=True
        )

    def test_command_prints_the_path_the_function_returns(self):
        root = Path(
            subprocess.run(
                ["git", "-C", str(HELPER.parent), "rev-parse", "--show-toplevel"],
                capture_output=True,
                check=True,
                text=True,
            ).stdout.strip()
        )
        printed = self.run_helper("shared-producing-manifest")
        self.assertEqual(printed.returncode, 0)
        self.assertEqual(printed.stdout, shared_producing_manifest(root) + "\n")

    def test_failed_lookup_exits_nonzero_with_the_reason(self):
        failed = self.run_helper("package-directory", "no-such-package")
        self.assertEqual(failed.returncode, 1)
        self.assertEqual(failed.stdout, "")
        self.assertIn("no-such-package", failed.stderr)


if __name__ == "__main__":
    unittest.main()
