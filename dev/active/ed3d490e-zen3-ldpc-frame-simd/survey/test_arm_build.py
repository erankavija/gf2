#!/usr/bin/env python3
"""Check that the arm build projection ignores what is outside the identity set and no more."""

import copy
import importlib.util
import sys
import unittest
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("arm_build", SURVEY / "arm_build.py")
projection = importlib.util.module_from_spec(spec)
spec.loader.exec_module(projection)

BUILD = {
    "rustc": "rustc 1.95.0", "cargo": "cargo 1.95.0", "release_profile": {"opt-level": 3},
    "build_commands": [
        {"environment": {"RUSTFLAGS": "-C target-cpu=native"},
         "argv": ["cargo", "build", "--release", "--manifest-path", "/a"]},
        {"environment": {"RUSTFLAGS": "-C target-cpu=native", "GF2_AFF3CT_ROOT": "/x"},
         "argv": ["cargo", "build", "--release", "--manifest-path", "/a", "--package", "member"]}],
    "executables": {"bin": {"sha256": "a" * 64, "compiler_options": ["-O"],
                            "inputs": {"repository_files_sha256": "1" * 64,
                                       "other_files_sha256": "2" * 64,
                                       "linked": {"/lib/x.a": "3" * 64}}}},
    "arms": {"workspace": {"source": "workspace", "executable": "bin", "package": "main",
                           "executable_sha256": "a" * 64},
             "member": {"source": "workspace", "executable": "bin", "package": "member",
                        "executable_sha256": "a" * 64},
             "pinned": {"source": "pinned", "executable": "other",
                        "executable_sha256": "b" * 64}},
}


def projected(edit, name="workspace"):
    build = copy.deepcopy(BUILD)
    edit(build)
    return projection.arm_build(build, name)


class ArmBuildTest(unittest.TestCase):
    def setUp(self):
        self.base = projection.arm_build(BUILD, "workspace")

    def test_source_and_link_inputs_do_not_matter(self):
        def edit(build):
            inputs = build["executables"]["bin"]["inputs"]
            inputs.update(repository_files_sha256="9" * 64, other_files_sha256="8" * 64,
                          linked={"/elsewhere/y.a": "7" * 64})
        self.assertEqual(projected(edit), self.base)

    def test_paths_and_other_arms_do_not_matter(self):
        def edit(build):
            build["build_commands"][0]["argv"][-1] = "/b"
            build["arms"]["pinned"]["executable_sha256"] = "c" * 64
        self.assertEqual(projected(edit), self.base)

    def test_the_executable_digest_matters(self):
        self.assertNotEqual(projected(
            lambda build: build["arms"]["workspace"].update(executable_sha256="d" * 64)), self.base)

    def test_each_code_generation_setting_matters(self):
        edits = [
            lambda build: build["executables"]["bin"].update(compiler_options=["-O0"]),
            lambda build: build.update(rustc="rustc 1.96.0"),
            lambda build: build.update(cargo="cargo 1.96.0"),
            lambda build: build["release_profile"].update({"opt-level": 2}),
            lambda build: build["build_commands"][0]["environment"].update(RUSTFLAGS=""),
            lambda build: build["build_commands"][0]["argv"].remove("--release"),
        ]
        for edit in edits:
            self.assertNotEqual(projected(edit), self.base)

    def test_a_command_of_another_package_does_not_matter(self):
        def edit(build):
            command = build["build_commands"][1]
            command["environment"]["RUSTFLAGS"] = "-C opt-level=1"
            command["argv"].remove("--release")
        self.assertEqual(projected(edit), self.base)

    def test_the_arms_own_command_matters(self):
        def edit(build):
            build["build_commands"][1]["environment"]["RUSTFLAGS"] = "-C opt-level=1"
        self.assertNotEqual(projected(edit, "member"), projection.arm_build(BUILD, "member"))

    def test_a_package_no_command_names_takes_the_commands_naming_none(self):
        self.assertEqual(len(self.base["builds"]), 1)

    def test_a_pinned_arm_is_identified_by_its_digest_alone(self):
        self.assertEqual(projection.arm_build(BUILD, "pinned"), {"executable_sha256": "b" * 64})


if __name__ == "__main__":
    unittest.main()
