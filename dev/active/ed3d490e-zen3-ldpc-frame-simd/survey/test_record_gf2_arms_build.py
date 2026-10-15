#!/usr/bin/env python3
"""Check the arms build identity recorder's refusals and its reproduction.

The reproduction tests build the arms workspace against GF2_AFF3CT_ROOT, the
staged AFF3CT tree, and are skipped without one.
"""

import importlib.util
import json
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

# The package whose build script compiles the AFF3CT translation unit.
HARNESS = "ldpc-throughput-harness"
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


def rule(target, *prerequisites):
    return f"{target}: " + " ".join(str(path).replace(" ", "\\ ") for path in prerequisites) + "\n"


class InputsTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        top = Path(temporary.name)
        self.root, self.target = top / "repository", top / "repository" / "target"
        self.source = self.root / "unit.cpp"
        self.header = top / "staged tree" / "decoder.hpp"
        self.archive = top / "staged tree" / "libtrace.a"
        self.product = self.target / "release" / "libshim.a"
        for path in (self.source, self.header, self.archive, self.product):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("first")
        self.executable = self.target / "release" / "arm"
        self.executable.write_text("linked")
        self.dep_info = top / "arm.d"
        self.dep_info.write_text(rule(self.executable, self.source, self.header))
        self.link_rules = top / "arm.link.d"
        self.link_rules.write_text(
            f"{self.executable}: \\\n {self.product} \\\n"
            f" {str(self.archive).replace(' ', chr(92) + ' ')}\n\n{self.product}:\n")

    def inputs(self):
        return recorder.inputs(self.executable, self.dep_info, self.link_rules,
                               self.root, self.target)

    def test_a_changed_file_on_either_side_of_the_root_changes_its_listing_digest(self):
        first = self.inputs()
        self.header.write_text("second")
        second = self.inputs()
        self.source.write_text("second")
        third = self.inputs()
        self.assertEqual(first["repository_files_sha256"], second["repository_files_sha256"])
        self.assertNotEqual(first["other_files_sha256"], second["other_files_sha256"])
        self.assertNotEqual(second["repository_files_sha256"], third["repository_files_sha256"])
        self.assertEqual(second["other_files_sha256"], third["other_files_sha256"])

    def test_a_changed_archive_the_link_read_changes_the_identity(self):
        first = self.inputs()
        self.assertEqual(list(first["linked"]), [str(self.archive)])
        self.archive.write_text("second")
        os.utime(self.archive, ns=(0, 0))
        self.assertNotEqual(first["linked"], self.inputs()["linked"])

    def test_an_executable_older_than_a_file_its_link_read_is_refused(self):
        newer = self.executable.stat().st_mtime_ns + 1
        os.utime(self.archive, ns=(newer, newer))
        with self.assertRaisesRegex(SystemExit, "libtrace.a"):
            self.inputs()

    def test_make_rules_are_read_as_make_reads_them(self):
        rules = ("out/unit.o: unit.cpp /opt/aff3ct\\ tree/decoder.hpp \\\n"
                 " /opt/cost$$/a\\#b.hpp back\\\\ slash.hpp\n\n"
                 "second\\ unit.o: C:/drive.hpp\nphony.hpp:\n")
        self.assertEqual(recorder.prerequisites(rules),
                         ["unit.cpp", "/opt/aff3ct tree/decoder.hpp", "/opt/cost$/a#b.hpp",
                          "back\\", "slash.hpp", "C:/drive.hpp"])

    def test_the_build_script_reads_make_rules_as_make_reads_them(self):
        unit = ROOT / recorder.located("package-directory", HARNESS) / "make_rules.rs"
        with tempfile.TemporaryDirectory() as temporary:
            tests = Path(temporary) / "make_rules"
            subprocess.run(["rustc", f"+{recorder.TOOLCHAIN}", "--edition", "2021", "--test",
                            str(unit), "-o", str(tests)], check=True, capture_output=True)
            subprocess.run([str(tests)], check=True, capture_output=True)


class LinkRulesTest(unittest.TestCase):
    def test_the_rules_are_those_written_beside_the_file_with_the_executable_content(self):
        with tempfile.TemporaryDirectory() as temporary:
            release = Path(temporary)
            (release / "deps").mkdir()
            for name, content in (("arm", "arm"), ("deps/arm-1", "stale"), ("deps/arm-2", "arm")):
                (release / name).write_text(content)
                (release / f"{name}.link.d").write_text("")
            self.assertEqual(recorder.link_rules(release / "arm"),
                             release / "deps/arm-2.link.d")
            (release / "deps/arm-2").write_text("relinked")
            with self.assertRaisesRegex(SystemExit, "arm"):
                recorder.link_rules(release / "arm")


class CargoConfigurationTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.top = Path(temporary.name)
        self.directory = self.top / "outer" / "checkout"
        self.home = self.top / "cargo home"

    def write(self, folder, name, text):
        (folder / ".cargo").mkdir(parents=True, exist_ok=True)
        (folder / ".cargo" / name).write_text(text)
        return folder / ".cargo" / name

    def test_every_file_cargo_reads_is_found(self):
        self.home.mkdir()
        (self.home / "config.toml").write_text("")
        expected = [self.write(self.directory, "config.toml", ""),
                    self.write(self.directory.parent, "config", ""),
                    self.write(self.top, "config.toml", "")]
        self.write(self.directory / "below", "config.toml", "")
        found = recorder.cargo_configuration(self.directory, self.home)
        self.assertEqual([path for path in found if self.top in path.parents],
                         expected + [self.home / "config.toml"])
        self.assertEqual(recorder.cargo_configuration(self.directory, self.top / ".cargo"),
                         found[:-1])

    def test_an_environment_table_is_recorded(self):
        path = self.write(self.directory, "config.toml", '[env]\nRAYON_NUM_THREADS = "4"\n')
        self.assertEqual(recorder.configuration_rows([path]),
                         [{"path": str(path), "sha256": recorder.digest(path),
                           "env": {"RAYON_NUM_THREADS": "4"}}])

    def test_a_setting_the_record_does_not_state_is_refused(self):
        for text in ("[profile.release]\nlto = false\n", "[build]\nrustflags = ['-O']\n",
                     "[target.x86_64-unknown-linux-gnu]\nlinker = 'ld'\n",
                     "[env]\nCXXFLAGS = '-O0'\n", "[env]\nPATH = '/opt/bin'\n",
                     "include = ['other.toml']\n"):
            path = self.write(self.directory, "config.toml", text)
            with self.assertRaisesRegex(SystemExit, "config.toml"):
                recorder.configuration_rows([path])


class BuildEnvironmentTest(unittest.TestCase):
    def test_inherited_compiler_settings_do_not_reach_the_build(self):
        steering = ["CXX", "CC", "CXXFLAGS", "CFLAGS", "HOST_CXX", "TARGET_CXXFLAGS",
                    "CXX_x86_64_unknown_linux_gnu", "CXXFLAGS_x86_64-unknown-linux-gnu",
                    "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS",
                    "RUSTC_WRAPPER", "CARGO_TARGET_DIR", "CARGO_PROFILE_RELEASE_LTO",
                    "CARGO_INCREMENTAL", "CARGO_HOST_LINKER", "CARGO_UNSTABLE_BUILD_STD",
                    "LD_RUN_PATH", "GCC_EXEC_PREFIX", "COMPILER_PATH", "GF2_AFF3CT_ROOT"]
        inherited = {**{name: "inherited" for name in steering}, "PATH": "/bin", "HOME": "/home"}
        environment = recorder.build_environment(inherited)
        self.assertEqual({name: value for name, value in environment.items()
                          if value == "inherited"}, {})
        self.assertEqual(environment, {"PATH": "/bin", "HOME": "/home", **recorder.ENVIRONMENT})
        self.assertEqual(environment["CXX"], recorder.CXX)


class SeparateMembersTest(unittest.TestCase):
    def test_each_member_outside_the_default_build_builds_alone(self):
        def package(name, features=()):
            return {"id": name, "name": name,
                    "dependencies": [{"name": HARNESS, "features": list(features)}]}

        metadata = {"packages": [package("ldpc-batch-arms"), package("ldpc-batch-profile"),
                                 package("ldpc-aff3ct-intra-arm", ["aff3ct"]),
                                 package("ldpc-batch-tuned-arm")],
                    "workspace_default_members": ["ldpc-batch-arms", "ldpc-batch-profile"]}
        self.assertEqual(recorder.separate_members(metadata),
                         [(["--package", "ldpc-aff3ct-intra-arm"], True),
                          (["--package", "ldpc-batch-tuned-arm"], False)])


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

    def test_the_intra_arm_inputs_name_its_headers_and_its_archives(self):
        with tempfile.TemporaryDirectory() as out_dir:
            subprocess.run(
                [sys.executable, "-B", str(RECORDER), out_dir, os.environ["GF2_AFF3CT_ROOT"]],
                cwd=ROOT, check=True, capture_output=True)
            record = json.loads((Path(out_dir) / "build-identity.json").read_text())
        dep_info = SURVEY / "arms/target/release/aff3ct-intra-arm.d"
        self.assertIn("Decoder_LDPC_BP_flooding_SPA.hpp", dep_info.read_text())
        intra = record["executables"]["aff3ct-intra-arm"]
        self.assertIn("libcpptrace.a", {Path(path).name for path in intra["inputs"]["linked"]})
        self.assertTrue(intra["compiler_options"])


if __name__ == "__main__":
    unittest.main()
