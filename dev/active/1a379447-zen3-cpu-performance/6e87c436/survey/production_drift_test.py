"""Self-tests for `make-production-drift.py` (jit:ecad1724).

Run from any directory:
    python3 production_drift_test.py
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
from locate import END_STATE_FILE, ROOT, repository_files  # noqa: E402

SHARED = ("content_anchor", "repository_files", "rust_code_text", "asm_listing")
SCRIPTS = ("locate", "make-production-drift")
BENCHMARK_SCHEMA = "zen3-benchmark-receipt-v1"
MANIFEST = b'[package]\nname = "gf2-core"\n'
LIB = b"fn one() {}\n"
# The live arm build records' reader and the files it loads beside itself.
ARM_FILES = (
    "record-peak-memory.py", "record-intra-quality.py", "smoke-gf2-arms.py", "arm_build.py",
    "arms.json",
)


def run(command, cwd):
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


BUILT, REBUILT = sha256(b"arm build"), sha256(b"another arm build")


class AllReceipts(unittest.TestCase):
    """The all-receipt mode on a fixture tree with one receipt per class."""

    # Receipt name: (declared schema, pinned path to measured bytes, bytes its snapshot holds,
    # arm to pinned executable digest).
    RECEIPTS = {
        "identical": (BENCHMARK_SCHEMA, {"Cargo.toml": MANIFEST, "src/lib.rs": LIB}, {}, {}),
        "comment-only": (
            BENCHMARK_SCHEMA,
            {"Cargo.toml": MANIFEST, "src/lib.rs": b"// note\nfn one() {}\n"},
            {"src/lib.rs": b"// note\nfn one() {}\n"},
            {},
        ),
        "code": (
            BENCHMARK_SCHEMA,
            {"Cargo.toml": MANIFEST, "src/lib.rs": b"fn two() {}\n"},
            {"src/lib.rs": b"fn two() {}\n"},
            {},
        ),
        "removed": (
            BENCHMARK_SCHEMA,
            {"Cargo.toml": MANIFEST, "src/lib.rs": LIB, "src/gone.rs": b"fn gone() {}\n"},
            {"src/gone.rs": b"fn gone() {}\n"},
            {},
        ),
        "added": (BENCHMARK_SCHEMA, {"Cargo.toml": MANIFEST}, {}, {}),
        "unavailable": (
            BENCHMARK_SCHEMA,
            {"Cargo.toml": MANIFEST, "src/lib.rs": b"fn three() {}\n"},
            {},
            {},
        ),
        "executable-identical": (BENCHMARK_SCHEMA, {}, {}, {"built-dvb-t2-r12": BUILT}),
        "executable-differs": (BENCHMARK_SCHEMA, {}, {}, {"built-nr-bg1-z384": REBUILT}),
        "executable-unrecorded": (BENCHMARK_SCHEMA, {}, {}, {"other-dvb-t2-r12": BUILT}),
        "unpinned": ("other-receipt-v1", {}, {}, {"built-dvb-t2-r12": None}),
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
        arms = cls.root / "arms"
        arms.mkdir()
        reader = (ROOT / repository_files.live_file(ROOT, ARM_FILES[0])).parent
        for name in ARM_FILES:
            shutil.copy(reader / name, arms)
        (arms / "build-identity.json").write_text(
            json.dumps({"catalogue": {}, "arms": {"built": {"executable_sha256": BUILT}}})
        )
        package = cls.root / "crates" / "gf2-core"
        (package / "src").mkdir(parents=True)
        (package / "Cargo.toml").write_bytes(MANIFEST)
        (package / "src" / "lib.rs").write_bytes(LIB)
        for name, (schema, pinned, held, executables) in cls.RECEIPTS.items():
            receipt = cls.root / "dev" / "bench_results" / "fixture" / name
            producing = receipt / "inputs" / "producing"
            producing.mkdir(parents=True)
            digests = {f"crates/gf2-core/{path}": sha256(data) for path, data in pinned.items()}
            if not pinned:
                digests = {"Cargo.lock": sha256(b"version = 4\n")}
            (producing / "producing-snapshot.json").write_text(
                json.dumps({"behavior_sha256": digests, "build_inputs_sha256": {}})
            )
            for path, data in held.items():
                (producing / "crates" / "gf2-core" / path).parent.mkdir(parents=True, exist_ok=True)
                (producing / "crates" / "gf2-core" / path).write_bytes(data)
            arms_pinned = {
                arm: {"executable_path": f"/built/{arm}"}
                | ({} if digest is None else {"executable_sha256": digest})
                for arm, digest in executables.items()
            }
            (receipt / "receipt.json").write_text(json.dumps({"schema": schema, "arms": arms_pinned}))
        run(["git", "init", "-q"], cls.root)
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

    def differing(self, name):
        return [
            (entry.get("arm", entry.get("path")), entry["class"])
            for entry in self.rows[name]["differing_files"]
        ]

    def assert_row(self, name, verdict, basis, differing):
        self.assertEqual(self.rows[name]["class"], verdict, name)
        self.assertEqual(self.rows[name]["basis"], basis, name)
        self.assertEqual(self.differing(name), differing, name)

    def test_every_enumerated_receipt_has_one_row_of_two_classes(self):
        self.assertEqual(set(self.rows), set(self.RECEIPTS))
        self.assertEqual(
            {row["class"] for row in self.rows.values()}, {"code-identical", "code-differs"}
        )

    def test_two_runs_write_identical_bytes(self):
        self.assertEqual(self.outputs[0], self.outputs[1])

    def test_identical_and_comment_only_files_are_code_identical(self):
        for name in ("identical", "comment-only"):
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

    def test_missing_measured_bytes_make_code_differs_by_path(self):
        self.assert_row(
            "unavailable",
            "code-differs",
            "measured bytes unavailable",
            [("crates/gf2-core/src/lib.rs", "unavailable")],
        )

    def test_executables_compare_with_the_live_build_record_by_arm(self):
        self.assert_row("executable-identical", "code-identical", "executable identity", [])
        self.assert_row(
            "executable-differs",
            "code-differs",
            "executable identity",
            [("built-nr-bg1-z384", "executable-differs")],
        )
        self.assert_row(
            "executable-unrecorded",
            "code-differs",
            "executable identity",
            [("other-dvb-t2-r12", "arm-unrecorded")],
        )

    def test_unpinned_measured_content_names_the_unresolved_inputs(self):
        self.assert_row(
            "unpinned", "code-differs", "measured content not pinned", [("Cargo.lock", "unresolved")]
        )


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
