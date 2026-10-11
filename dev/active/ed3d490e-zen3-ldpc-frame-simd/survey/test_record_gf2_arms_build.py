#!/usr/bin/env python3
"""Check the arms build identity recorder's refusals and its reproduction.

The reproduction test builds the arms workspace and needs GF2_AFF3CT_ROOT,
the staged AFF3CT tree; it is skipped without one.
"""

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
RECORDER = SURVEY / "record-gf2-arms-build.py"
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(SURVEY), "rev-parse", "--show-toplevel"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
)

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("arms_build_recorder", RECORDER)
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)

RELEASE = Path("arms/target/release")
BUILT = {"built-arm": "a" * 64}
PIN = {"record": "campaign/build-identity.json", "executable": "pinned-arm", "sha256": "b" * 64}
RECORDS = {PIN["record"]: {"sha256": "c" * 64, "pins": []}}


def workspace_arm(executable):
    return {"source": "workspace", "package": "arms", "executable": executable, "environment": {}}


def pinned_arm(digest):
    return {"source": "pinned", "executable": PIN["executable"], "executable_sha256": digest,
            "pinned_by": {"record": "build-identity.json", "sha256": "c" * 64},
            "environment": {}}


def arm_rows(named, found_at):
    return recorder.arm_rows(named, [{**PIN, "found_at": found_at}], RECORDS, BUILT, RELEASE)


class ArmRowsTest(unittest.TestCase):
    def test_every_catalogued_arm_gets_its_executable_digest(self):
        rows = arm_rows({"built": workspace_arm("built-arm"), "pinned": pinned_arm(PIN["sha256"])},
                        ["target/campaign/release/pinned-arm"])
        self.assertEqual(rows["built"]["executable_sha256"], BUILT["built-arm"])
        self.assertEqual(rows["built"]["path"], str(RELEASE / "built-arm"))
        self.assertEqual(rows["pinned"]["executable_sha256"], PIN["sha256"])
        self.assertEqual(rows["pinned"]["found_at"], ["target/campaign/release/pinned-arm"])
        self.assertEqual(rows["pinned"]["pinned_by"],
                         {"record": PIN["record"], "sha256": "c" * 64})

    def test_a_workspace_arm_the_build_does_not_produce_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "absent"):
            arm_rows({"absent": workspace_arm("absent-arm")}, [])

    def test_a_pinned_arm_no_target_file_carries_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "pinned"):
            arm_rows({"pinned": pinned_arm(PIN["sha256"])}, [])

    def test_a_pinned_arm_no_record_pins_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "pinned"):
            arm_rows({"pinned": pinned_arm("d" * 64)}, ["target/campaign/release/pinned-arm"])


class InputsTest(unittest.TestCase):
    def test_a_changed_file_on_either_side_of_the_root_changes_its_listing_digest(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repository"
            source, header = root / "unit.cpp", Path(temporary) / "tree" / "decoder.hpp"
            for path in (source, header):
                path.parent.mkdir()
                path.write_text("first")
            dep_info = Path(temporary) / "arm.d"
            dep_info.write_text(f"{temporary}/arm: {source} {header}\n")
            first = recorder.inputs(dep_info, root)
            header.write_text("second")
            second = recorder.inputs(dep_info, root)
            source.write_text("second")
            third = recorder.inputs(dep_info, root)
        self.assertEqual(first["repository_files_sha256"], second["repository_files_sha256"])
        self.assertNotEqual(first["other_files_sha256"], second["other_files_sha256"])
        self.assertNotEqual(second["repository_files_sha256"], third["repository_files_sha256"])
        self.assertEqual(second["other_files_sha256"], third["other_files_sha256"])


class BuildEnvironmentTest(unittest.TestCase):
    def test_inherited_compiler_settings_do_not_reach_the_build(self):
        steering = ["CXX", "CC", "CXXFLAGS", "CFLAGS", "HOST_CXX", "TARGET_CXXFLAGS",
                    "CXX_x86_64_unknown_linux_gnu", "CXXFLAGS_x86_64-unknown-linux-gnu",
                    "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS",
                    "RUSTC_WRAPPER", "CARGO_TARGET_DIR", "CARGO_PROFILE_RELEASE_LTO"]
        inherited = {**{name: "inherited" for name in steering}, "PATH": "/bin", "HOME": "/home"}
        environment = recorder.build_environment(inherited)
        self.assertEqual({name: value for name, value in environment.items()
                          if value == "inherited"}, {})
        self.assertEqual(environment, {"PATH": "/bin", "HOME": "/home", **recorder.ENVIRONMENT})
        self.assertEqual(environment["CXX"], recorder.CXX)


@unittest.skipUnless(os.environ.get("GF2_AFF3CT_ROOT"), "needs the staged AFF3CT tree")
class ReproductionTest(unittest.TestCase):
    def test_a_second_run_over_an_unchanged_build_reproduces_the_record(self):
        records = []
        for _ in range(2):
            with tempfile.TemporaryDirectory() as out_dir:
                subprocess.run(
                    [sys.executable, "-B", str(RECORDER), out_dir, os.environ["GF2_AFF3CT_ROOT"]],
                    cwd=ROOT, check=True, capture_output=True)
                records.append((Path(out_dir) / "build-identity.json").read_bytes())
        self.assertEqual(records[0], records[1])

    def test_the_dep_info_of_the_intra_arm_names_the_headers_its_unit_compiles(self):
        with tempfile.TemporaryDirectory() as out_dir:
            subprocess.run(
                [sys.executable, "-B", str(RECORDER), out_dir, os.environ["GF2_AFF3CT_ROOT"]],
                cwd=ROOT, check=True, capture_output=True)
        dep_info = SURVEY / "arms/target/release/aff3ct-intra-arm.d"
        self.assertIn("Decoder_LDPC_BP_flooding_SPA.hpp", dep_info.read_text())


if __name__ == "__main__":
    unittest.main()
