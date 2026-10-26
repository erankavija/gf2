"""Self-tests for `make-production-drift.py` (jit:ecad1724).

Run from any directory:
    python3 production_drift_test.py
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

SURVEY = Path(__file__).resolve().parent
sys.path.insert(0, str(SURVEY))
from locate import END_STATE_FILE, ROOT, repository_files  # noqa: E402

SHARED = ("content_anchor", "repository_files", "rust_code_text", "asm_listing")
SCRIPTS = ("locate", "make-production-drift")
BENCHMARK_SCHEMA = "zen3-benchmark-receipt-v1"
MANIFEST = b'[package]\nname = "gf2-core"\n'
LIB = b'mod inline;\nfn one() {}\n#[cfg(test)]\nmod unit;\n#[cfg(test)]\n#[path = "paired.rs"]\nmod paired_tests;\n'
EARLIER_LIB = b"fn zero() {}\n"
KERNEL = b"__global__ void one() {}\n"
EARLIER_KERNEL = b"__global__ void zero() {}\n"
EXTRA = b"__global__ void extra() {}\n"
TEST = b"#[test]\nfn one() {}\n"
EARLIER_TEST = b"#[test]\nfn zero() {}\n"
UNIT = b"#[test]\nfn unit() {}\n"
EARLIER_UNIT = b"#[test]\nfn earlier_unit() {}\n"
# The production files of the current fixture package.
BUILD = b'fn main() { cc::Build::new().file("hip/kernel.hip").file("hip/extra.hip"); }\n'
EARLIER_BUILD = b'fn main() { cc::Build::new().file("hip/kernel.hip").file("hip/old.hip"); }\n'
INLINE_TEST = b"fn two() {}\n#[cfg(test)]\nmod checks {\n    #[test]\n    fn two() {}\n}\n"
EARLIER_INLINE_TEST = b"fn two() {}\n#[cfg(test)]\nmod checks {\n    #[test]\n    fn zero() {}\n}\n"
TABLE_SOURCE = b'static TABLE: &[u8] = include_bytes!("../../../data/table.bin");\n'
TABLE = b"\x01"
EARLIER_TABLE = b"\x00"
# A package-relative path that leaves the package: an include target at the root.
OUTSIDE_TABLE = "../../data/table.bin"
PRODUCTION = {
    "Cargo.toml": MANIFEST,
    "build.rs": BUILD,
    "src/lib.rs": LIB,
    "src/inline.rs": INLINE_TEST,
    "src/unit.rs": UNIT,
    "src/paired.rs": UNIT,
    "hip/kernel.hip": KERNEL,
    "hip/extra.hip": EXTRA,
    "src/table.rs": TABLE_SOURCE,
    OUTSIDE_TABLE: TABLE,
}


def without(path):
    return {name: data for name, data in PRODUCTION.items() if name != path}


def run(command, cwd):
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class AllReceipts(unittest.TestCase):
    """The all-receipt mode on a fixture tree with one receipt per class and basis.

    The tree holds a documentation file the receipts do not pin, and its history
    an earlier commit whose `lib.rs` differs.
    """

    # Receipt name: (declared schema, pinned path to measured bytes, bytes its snapshot holds,
    # the `source_revision` its checkpoint manifest records: a commit label or a literal).
    RECEIPTS = {
        "identical": (BENCHMARK_SCHEMA, PRODUCTION, {}, None),
        "comment-only": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/lib.rs": b"// note\n" + LIB},
            {"src/lib.rs": b"// note\n" + LIB},
            None,
        ),
        "documentation-only": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/notes.md": b"# Earlier notes\n"},
            {"src/notes.md": b"# Earlier notes\n"},
            None,
        ),
        "code": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/lib.rs": b"fn two() {}\n"},
            {"src/lib.rs": b"fn two() {}\n"},
            None,
        ),
        "removed": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/gone.rs": b"fn gone() {}\n"},
            {"src/gone.rs": b"fn gone() {}\n"},
            None,
        ),
        "added": (BENCHMARK_SCHEMA, without("src/lib.rs"), {}, None),
        "added-kernel": (BENCHMARK_SCHEMA, without("hip/extra.hip"), {}, None),
        "kernel-source": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"hip/kernel.hip": EARLIER_KERNEL},
            {"hip/kernel.hip": EARLIER_KERNEL},
            None,
        ),
        "test-only": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"tests/check.rs": EARLIER_TEST},
            {"tests/check.rs": EARLIER_TEST},
            None,
        ),
        "unit-test-only": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/unit.rs": EARLIER_UNIT, "src/paired.rs": EARLIER_UNIT},
            {"src/unit.rs": EARLIER_UNIT, "src/paired.rs": EARLIER_UNIT},
            None,
        ),
        "inline-test-only": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/inline.rs": EARLIER_INLINE_TEST},
            {"src/inline.rs": EARLIER_INLINE_TEST},
            None,
        ),
        "removed-kernel": (
            BENCHMARK_SCHEMA,
            without("hip/extra.hip") | {"build.rs": EARLIER_BUILD, "hip/old.hip": EARLIER_KERNEL},
            {"build.rs": EARLIER_BUILD, "hip/old.hip": EARLIER_KERNEL},
            None,
        ),
        "unavailable": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/lib.rs": b"fn three() {}\n"},
            {},
            None,
        ),
        "unavailable-and-code": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"src/lib.rs": b"fn three() {}\n", "hip/kernel.hip": EARLIER_KERNEL},
            {"hip/kernel.hip": EARLIER_KERNEL},
            None,
        ),
        "deleted-test-target": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {"tests/gone.rs": TEST},
            {"tests/gone.rs": TEST},
            None,
        ),
        "include-target": (
            BENCHMARK_SCHEMA,
            PRODUCTION | {OUTSIDE_TABLE: EARLIER_TABLE},
            {OUTSIDE_TABLE: EARLIER_TABLE},
            None,
        ),
        "test-target-pin-only": (BENCHMARK_SCHEMA, {"tests/check.rs": TEST}, {}, "current"),
        "revision-identical": (BENCHMARK_SCHEMA, {}, {}, "current"),
        "revision-differs": (BENCHMARK_SCHEMA, {}, {}, "earlier"),
        "revision-kernel": (BENCHMARK_SCHEMA, {}, {}, "kernel-earlier"),
        "revision-test-only": (BENCHMARK_SCHEMA, {}, {}, "tests-earlier"),
        "unpinned": ("other-receipt-v1", {}, {}, ""),
    }

    @classmethod
    def setUpClass(cls):
        directory = tempfile.TemporaryDirectory()
        cls.addClassCleanup(directory.cleanup)
        cls.root = Path(directory.name)
        survey = cls.root / "survey"
        survey.mkdir()
        (cls.root / "scripts").mkdir()
        (cls.root / "selector").mkdir()
        for module in SHARED:
            shutil.copy(ROOT / repository_files.live_file(ROOT, f"{module}.py"), cls.root / "scripts")
        shutil.copy(ROOT / repository_files.live_file(ROOT, "repo_artifacts.py"), survey)
        shutil.copy(ROOT / repository_files.live_file(ROOT, "select-receipts.py"), cls.root / "selector")
        for script in SCRIPTS:
            shutil.copy(SURVEY / f"{script}.py", survey)
        package = cls.root / "crates" / "gf2-core"
        (package / "src").mkdir(parents=True)
        (package / "Cargo.toml").write_bytes(MANIFEST)
        (package / "src" / "lib.rs").write_bytes(EARLIER_LIB)
        run(["git", "init", "-q"], cls.root)
        run(["git", "config", "user.email", "fixture@example.invalid"], cls.root)
        run(["git", "config", "user.name", "Fixture"], cls.root)
        commits = {"earlier": cls.commit()}
        (package / "src" / "lib.rs").write_bytes(LIB)
        (package / "src" / "unit.rs").write_bytes(UNIT)
        (package / "src" / "paired.rs").write_bytes(UNIT)
        (package / "src" / "inline.rs").write_bytes(INLINE_TEST)
        (package / "build.rs").write_bytes(BUILD)
        (package / "benchmarks-cpp").mkdir()
        (package / "benchmarks-cpp" / "standalone.cpp").write_bytes(b"int main() {}\n")
        (package / "src" / "notes.md").write_bytes(b"# Notes\n")
        (package / "hip").mkdir()
        (package / "hip" / "kernel.hip").write_bytes(EARLIER_KERNEL)
        (package / "hip" / "extra.hip").write_bytes(EXTRA)
        (package / "tests").mkdir()
        (package / "tests" / "check.rs").write_bytes(TEST)
        (package / "src" / "table.rs").write_bytes(TABLE_SOURCE)
        (cls.root / "data").mkdir()
        (cls.root / "data" / "table.bin").write_bytes(TABLE)
        commits["kernel-earlier"] = cls.commit()
        (package / "hip" / "kernel.hip").write_bytes(KERNEL)
        (package / "tests" / "check.rs").write_bytes(EARLIER_TEST)
        commits["tests-earlier"] = cls.commit()
        (package / "tests" / "check.rs").write_bytes(TEST)
        commits["current"] = cls.commit()
        for name, (schema, pinned, held, revision) in cls.RECEIPTS.items():
            receipt = cls.root / "dev" / "bench_results" / "fixture" / name
            producing = receipt / "inputs" / "producing"
            producing.mkdir(parents=True)
            digests = {
                os.path.normpath(f"crates/gf2-core/{path}"): sha256(data)
                for path, data in pinned.items()
            }
            if not pinned:
                digests = {"Cargo.lock": sha256(b"version = 4\n")}
            (producing / "producing-snapshot.json").write_text(
                json.dumps({"behavior_sha256": digests, "build_inputs_sha256": {}})
            )
            for path, data in held.items():
                held_path = producing / os.path.normpath(f"crates/gf2-core/{path}")
                held_path.parent.mkdir(parents=True, exist_ok=True)
                held_path.write_bytes(data)
            (receipt / "receipt.json").write_text(json.dumps({"schema": schema}))
            if revision is not None:
                (receipt / "checkpoints").mkdir()
                (receipt / "checkpoints" / "manifest.json").write_text(
                    json.dumps({"source_revision": commits.get(revision, revision)})
                )
        run(["git", "add", "-A"], cls.root)
        cls.outputs = []
        for attempt in ("first.json", "second.json"):
            done = run(
                [sys.executable, "-B", "survey/make-production-drift.py", "--all-receipts", attempt],
                cls.root,
            )
            assert done.returncode == 0, done.stderr
            cls.outputs.append((cls.root / attempt).read_bytes())
        record = json.loads(cls.outputs[0])
        cls.rows = {
            Path(receipt["receipt"]).parent.name: row
            for row in record["rows"]
            for receipt in row["receipts"]
        }

    @classmethod
    def commit(cls):
        run(["git", "add", "-A"], cls.root)
        run(["git", "commit", "-q", "-m", "fixture"], cls.root)
        return run(["git", "rev-parse", "HEAD"], cls.root).stdout.strip()

    def differing(self, name):
        return [(entry["path"], entry["class"]) for entry in self.rows[name]["differing_files"]]

    def assert_row(self, name, verdict, basis, differing):
        self.assertEqual(self.rows[name]["class"], verdict, name)
        self.assertEqual(self.rows[name]["basis"], basis, name)
        self.assertEqual(self.differing(name), differing, name)

    def test_every_enumerated_receipt_has_one_row_of_three_classes(self):
        self.assertEqual(set(self.rows), set(self.RECEIPTS))
        self.assertEqual(
            {row["class"] for row in self.rows.values()},
            {"code-identical", "code-differs", "unresolvable"},
        )

    def test_two_runs_write_identical_bytes(self):
        self.assertEqual(self.outputs[0], self.outputs[1])

    def test_identical_comment_and_documentation_changes_are_code_identical(self):
        for name in ("identical", "comment-only", "documentation-only"):
            self.assert_row(name, "code-identical", "production files", [])
        self.assertIn("comment-or-blank-only", self.rows["comment-only"]["class_counts"])

    def test_code_removed_and_added_files_make_code_differs_by_path(self):
        expected = {
            "code": [("crates/gf2-core/src/lib.rs", "code-differs")],
            "removed": [("crates/gf2-core/src/gone.rs", "removed")],
            "added": [("crates/gf2-core/src/lib.rs", "added")],
        }
        for name, files in expected.items():
            self.assert_row(name, "code-differs", "production files", files)

    def test_a_changed_kernel_source_makes_code_differs(self):
        self.assert_row(
            "kernel-source",
            "code-differs",
            "production files",
            [("crates/gf2-core/hip/kernel.hip", "code-differs")],
        )

    def test_a_changed_test_target_is_not_production_drift(self):
        self.assert_row("test-only", "code-identical", "production files", [])

    def test_test_code_inside_the_library_is_measured_content(self):
        for name, path in (("unit-test-only", "src/paired.rs"), ("inline-test-only", "src/inline.rs")):
            self.assertIn(
                (f"crates/gf2-core/{path}", "code-differs"), self.differing(name), name
            )

    def test_a_deleted_pinned_kernel_is_removed(self):
        self.assert_row(
            "removed-kernel",
            "code-differs",
            "production files",
            [
                ("crates/gf2-core/build.rs", "code-differs"),
                ("crates/gf2-core/hip/old.hip", "removed"),
                ("crates/gf2-core/hip/extra.hip", "added"),
            ],
        )

    def test_an_added_kernel_source_makes_code_differs(self):
        self.assert_row(
            "added-kernel",
            "code-differs",
            "production files",
            [("crates/gf2-core/hip/extra.hip", "added")],
        )

    def test_missing_measured_bytes_make_code_differs_by_path(self):
        self.assert_row(
            "unavailable",
            "code-differs",
            "measured bytes unavailable",
            [("crates/gf2-core/src/lib.rs", "unavailable")],
        )

    def test_known_differences_stay_listed_beside_unavailable_bytes(self):
        self.assert_row(
            "unavailable-and-code",
            "code-differs",
            "measured bytes unavailable",
            [
                ("crates/gf2-core/hip/kernel.hip", "code-differs"),
                ("crates/gf2-core/src/lib.rs", "unavailable"),
            ],
        )

    def test_a_deleted_test_target_is_not_production_drift(self):
        self.assert_row("deleted-test-target", "code-identical", "production files", [])

    def test_a_changed_include_target_outside_the_package_makes_code_differs(self):
        self.assert_row(
            "include-target", "code-differs", "production files", [("data/table.bin", "code-differs")]
        )

    def test_a_pin_of_test_targets_only_falls_back_to_the_source_revision(self):
        self.assert_row(
            "test-target-pin-only", "code-identical", "production files at the source revision", []
        )

    def test_a_recorded_source_revision_compares_its_files_with_the_tree(self):
        basis = "production files at the source revision"
        self.assert_row("revision-identical", "code-identical", basis, [])
        self.assert_row(
            "revision-differs",
            "code-differs",
            basis,
            [
                ("crates/gf2-core/src/lib.rs", "code-differs"),
                ("crates/gf2-core/build.rs", "added"),
                ("crates/gf2-core/hip/extra.hip", "added"),
                ("crates/gf2-core/hip/kernel.hip", "added"),
                ("crates/gf2-core/src/inline.rs", "added"),
                ("crates/gf2-core/src/paired.rs", "added"),
                ("crates/gf2-core/src/table.rs", "added"),
                ("crates/gf2-core/src/unit.rs", "added"),
            ],
        )
        self.assert_row(
            "revision-kernel", "code-differs", basis, [("crates/gf2-core/hip/kernel.hip", "code-differs")]
        )
        self.assert_row("revision-test-only", "code-identical", basis, [])

    def test_unresolvable_measured_content_names_the_producing_inputs(self):
        row = self.rows["unpinned"]
        self.assertEqual((row["class"], row["basis"]), ("unresolvable", "measured content not pinned"))
        self.assertIn("Cargo.lock", row["unresolved_inputs"])


class DenseRecordReproduces(unittest.TestCase):
    """The dense mode, run on the tree its end state pins, writes the committed record."""

    def test_the_committed_dense_record_reproduces_byte_for_byte(self):
        end_state = ROOT / repository_files.live_file(ROOT, END_STATE_FILE)
        commit = json.loads(end_state.read_bytes())["commit_informational"]
        with tempfile.TemporaryDirectory() as directory:
            clone = Path(directory) / "clone"
            for command in (
                ["git", "clone", "-q", "--shared", "--no-checkout", str(ROOT), str(clone)],
                ["git", "-C", str(clone), "reset", "-q", "--hard", commit],
            ):
                subprocess.run(command, capture_output=True, check=True)
            survey = clone / SURVEY.relative_to(ROOT)
            shared = clone / Path(repository_files.live_file(ROOT, "repository_files.py")).parent
            shutil.rmtree(survey)
            shutil.copytree(SURVEY, survey, ignore=shutil.ignore_patterns("__pycache__"))
            for module in SHARED:
                shutil.copy(ROOT / repository_files.live_file(ROOT, f"{module}.py"), shared)
            done = run([sys.executable, "-B", "make-production-drift.py"], survey)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertEqual(
                (survey / "production-drift.json").read_bytes(),
                (SURVEY / "production-drift.json").read_bytes(),
            )


if __name__ == "__main__":
    unittest.main()
