#!/usr/bin/env python3
"""Tests of `record-intra-quality.py`; the replay test is skipped without the built arm."""

import copy
import importlib.util
import json
import subprocess
import sys
import unittest
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("intra_recorder", SURVEY / "record-intra-quality.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)

QUALITY = {"frame_bit_errors": [0, 2, 0], "frames": 3, "memory_bytes": 7, "latency_ns_p50": 9}


class DifferencesTest(unittest.TestCase):
    def test_timing_and_memory_diagnostics_never_differ(self):
        self.assertEqual(recorder.differences("quality", QUALITY, {
            **QUALITY, "memory_bytes": 1, "latency_ns_p50": 2}), [])

    def test_a_changed_frame_is_named(self):
        changed = {**QUALITY, "frame_bit_errors": [0, 3, 0]}
        self.assertEqual(recorder.differences("quality", changed, QUALITY), ["frame_bit_errors"])

    def test_a_missing_field_differs(self):
        shorter = {name: value for name, value in QUALITY.items() if name != "frames"}
        self.assertEqual(recorder.differences("quality", shorter, QUALITY), ["frames"])

    def test_the_record_carries_zero_diagnostics(self):
        self.assertEqual({name: recorder.untimed(QUALITY)[name] for name in recorder.DIAGNOSTICS},
                         dict.fromkeys(recorder.DIAGNOSTICS, 0))


BUILD = {
    "rustc": "rustc 1.95.0", "cargo": "cargo 1.95.0", "release_profile": {"lto": True},
    "build_commands": [
        {"argv": ["cargo", "build", "--release"], "environment": {"RUSTFLAGS": "-C a"}},
        {"argv": ["cargo", "build", "--release", "--package", "ldpc-aff3ct-intra-arm"],
         "environment": {"RUSTFLAGS": "-C target-cpu=native"}}],
    "executables": {
        "aff3ct-intra-quality": {
            "sha256": "a" * 64, "compiler_options": ["-O3"],
            "inputs": {"repository_files_sha256": "1" * 64, "other_files_sha256": "2" * 64,
                       "linked": {"/lib/x.a": "3" * 64}}},
        "aff3ct-intra-arm": {"sha256": "b" * 64, "compiler_options": ["-O3"]}},
    "arms": {"aff3ct-spa-intra": {
        "source": "workspace", "executable": "aff3ct-intra-arm", "executable_sha256": "b" * 64,
        "package": "ldpc-aff3ct-intra-arm", "environment": {"GF2_AFF3CT_LLR_FACTOR": "2"}}},
}
HOST = {"cpu_model": "Test CPU", "avx2": True, "fma": True}
CODE = {"bundle_digests": {"llrs_sha256": "4" * 64}, "bundle_seed": 1,
        "request_declaration": {"iteration_cap": 50},
        "effective": {"decoder": {"class": "SPA"}, "llr_factor": 2.0, "settings": {}},
        "linked_aff3ct": {"revision": "r"}, "replayed_frames": 3,
        "quality": {"file": "quality.json", "sha256": "5" * 64}}


def replayed(build=BUILD, code=CODE, quality=QUALITY, host=HOST, identity=None):
    """The check's verdict on a second replay against the record of `BUILD`, `CODE`, `QUALITY`."""
    committed = {"build": recorder.identity(BUILD), "host": HOST, "codes": {"code": CODE}}
    record = {"build": identity or recorder.identity(build), "host": host,
              "codes": {"code": code}}
    return recorder.check_failures(committed, {"quality.json": QUALITY}, record,
                                   {"quality.json": quality})


def edited(edit, value=BUILD):
    value = copy.deepcopy(value)
    edit(value)
    return value


class CheckTest(unittest.TestCase):
    def test_an_identical_replay_has_no_failure(self):
        self.assertEqual(replayed(), {"code": [], "": []})

    def test_source_and_link_input_hashes_do_not_decide(self):
        inputs = ("executables", "aff3ct-intra-quality", "inputs")
        for name in ("repository_files_sha256", "other_files_sha256"):
            build = edited(lambda b: b[inputs[0]][inputs[1]][inputs[2]].update({name: "9" * 64}))
            self.assertEqual(replayed(build=build), {"code": [], "": []})
        build = edited(lambda b: b[inputs[0]][inputs[1]][inputs[2]].update(
            linked={"/elsewhere/y.a": "8" * 64}))
        self.assertEqual(replayed(build=build), {"code": [], "": []})

    def test_another_executable_and_another_packages_command_do_not_decide(self):
        def edit(build):
            build["executables"]["aff3ct-intra-arm"]["sha256"] = "c" * 64
            build["arms"]["aff3ct-spa-intra"]["executable_sha256"] = "c" * 64
            build["build_commands"][0]["environment"]["RUSTFLAGS"] = "-C b"
        self.assertEqual(replayed(build=edited(edit)), {"code": [], "": []})

    def test_the_linked_aff3ct_revision_and_library_do_not_decide(self):
        code = edited(lambda c: c.update(linked_aff3ct={"revision": "s"}), CODE)
        self.assertEqual(replayed(code=code), {"code": [], "": []})

    def test_a_field_outside_the_compared_ones_does_not_decide(self):
        self.assertEqual(
            replayed(identity={**recorder.identity(BUILD), "added": 1}, host={**HOST, "added": 1},
                     code={**CODE, "added": 1}, quality={**QUALITY, "added": 1}),
            {"code": [], "": []})

    def test_the_replays_build_identity_decides(self):
        for edit in (
            lambda b: b["executables"]["aff3ct-intra-quality"].update(sha256="e" * 64),
            lambda b: b["executables"]["aff3ct-intra-quality"].update(compiler_options=["-O0"]),
            lambda b: b["build_commands"][1]["environment"].update(RUSTFLAGS="-O0"),
            lambda b: b["build_commands"][1]["argv"].remove("--release"),
            lambda b: b.update(rustc="rustc 1.96.0"),
            lambda b: b.update(cargo="cargo 1.96.0"),
            lambda b: b["release_profile"].update(lto=False),
            lambda b: b["arms"]["aff3ct-spa-intra"]["environment"].update(
                GF2_AFF3CT_LLR_FACTOR="1"),
        ):
            verdict = replayed(build=edited(edit))
            self.assertEqual((verdict["code"], len(verdict[""])), ([], 1))

    def test_a_host_fact_decides(self):
        for fact in ({"cpu_model": "Other CPU"}, {"avx2": False}, {"fma": False}):
            verdict = replayed(host={**HOST, **fact})
            self.assertEqual((verdict["code"], len(verdict[""])), ([], 1))

    def test_the_replays_configuration_and_seed_decide(self):
        for edit in (
            lambda c: c["bundle_digests"].update(llrs_sha256="6" * 64),
            lambda c: c.update(bundle_seed=2),
            lambda c: c["request_declaration"].update(iteration_cap=20),
            lambda c: c["effective"]["decoder"].update({"class": "NMS"}),
            lambda c: c.update(replayed_frames=4),
        ):
            verdict = replayed(code=edited(edit, CODE))
            self.assertEqual((len(verdict["code"]), verdict[""]), (1, []))

    def test_a_differing_frame_and_a_differing_factor_are_named_for_their_code(self):
        quality = {**QUALITY, "frame_bit_errors": [0, 3, 0]}
        code = edited(lambda c: c["effective"].update(llr_factor=1.0), CODE)
        self.assertEqual(replayed(code=code, quality=quality)["code"],
                         ["quality field frame_bit_errors differs", "effective differs"])


class HostFactsTest(unittest.TestCase):
    def test_a_cpuinfo_block_binds_model_and_flags(self):
        text = ("processor\t: 0\nmodel name\t: Test CPU @ 1GHz\nflags\t\t: fpu avx2 fma sse\n\n"
                "processor\t: 1\nmodel name\t: Other\nflags\t\t: fpu\n")
        self.assertEqual(recorder.host_facts(text),
                         {"cpu_model": "Test CPU @ 1GHz", "avx2": True, "fma": True})

    def test_absent_flags_are_false(self):
        facts = recorder.host_facts("model name\t: X\nflags\t\t: fpu\n")
        self.assertEqual((facts["avx2"], facts["fma"]), (False, False))


class CommittedRecordTest(unittest.TestCase):
    def test_a_second_replay_reproduces_the_committed_record(self):
        executable = recorder.smoke.ROOT / recorder.smoke.release() / recorder.EXECUTABLE
        if not executable.exists():
            self.skipTest("the arms workspace is not built")
        records = [recorder.smoke.ROOT / path for path in recorder.smoke.listed(recorder.RECORD)
                   if Path(path).parent.name == "quality-intra"]
        records = [path.parent for path in records]
        self.assertEqual(len(records), 1)
        pinned = json.loads((records[0] / recorder.RECORD).read_text(encoding="utf-8"))
        if recorder.sha256(executable.read_bytes()) != pinned["build"]["executable_sha256"]:
            self.skipTest("the built replay executable is not the one the record pins")
        run =subprocess.run([sys.executable, "-B", str(SURVEY / "record-intra-quality.py"),
                              "check", str(records[0])], cwd=recorder.smoke.ROOT,
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stdout)


if __name__ == "__main__":
    unittest.main()
