#!/usr/bin/env python3
"""Record the preparation identity of this issue's REQ-10 granularity arms (jit:07ca8585).

The whole-decoding granularity is measured by the `3be770d5` harness, whose
build identity `record-preparation.py` records and whose executables the frozen
confirmations are pinned to. This script records the second identity, of the
work those confirmations do not cover: the isolated check-node arms of
`survey/arms`, their matched-ness receipt, and the fixed-stopping prepared
quality corpus the full-iteration cells decode against.

It writes two files and rewrites neither of the first identity's:

  `dev/bench_results/07ca8585/preparation/kernel-build-identity.json`
      toolchain, AFF3CT identity, shim flags, the digests of the four arms-
      workspace executables, the digests of the matched-ness receipt and of
      every fixed-stopping quality file, and the digests of the `after`
      throughput executables the profile re-sampling series measures.
  `dev/active/07ca8585/survey/producing-inputs-kernel.json`
      the producing closure the new families' plans select: the closure of the
      whole-decoding families plus this issue's arms workspace, its generators
      and the committed preparation these cells read.

Usage (from the worktree root):
  record-kernel-preparation.py --aff3ct DIR --kernel-dir DIR --after-dir DIR
      --inputs DIR
"""

import argparse
import datetime
import hashlib
import json
import pathlib
import subprocess

ISSUE = "07ca8585"
SURVEY = pathlib.Path("dev/active") / ISSUE / "survey"
ARMS = SURVEY / "arms"
PREPARATION = pathlib.Path("dev/bench_results") / ISSUE / "preparation"
QUALITY_FIXED = PREPARATION / "quality-fixed"
PARITY = PREPARATION / "checknode-parity.jsonl"
HARNESS = pathlib.Path("dev/active/3be770d5/survey/harness")
C077 = pathlib.Path("dev/bench_results/c077a88b/v3-preparation")
RESULTS = pathlib.Path("dev/bench_results") / ISSUE
KERNEL_EXECUTABLES = [
    "gf2-checknode-arm",
    "aff3ct-checknode-arm",
    "ldpc-checknode-verify",
    "ldpc-fixed-quality",
]
# The `after` throughput executables the profile re-sampling series measures.
PROFILE_EXECUTABLES = ["ldpc-profile", "ldpc-alloc-census"]
# Sources that decide what the new arms do, added to the whole-decoding closure.
ADDITIONS = [
    ARMS / "Cargo.toml",
    ARMS / "build.rs",
    ARMS / "cpp/update_rule_shim.cpp",
    ARMS / "src/lib.rs",
    ARMS / "src/cell.rs",
    ARMS / "src/prepare.rs",
    ARMS / "src/aff3ct_update.rs",
    ARMS / "src/bin/gf2-checknode-arm.rs",
    ARMS / "src/bin/aff3ct-checknode-arm.rs",
    ARMS / "src/bin/ldpc-checknode-verify.rs",
    ARMS / "src/bin/ldpc-fixed-quality.rs",
    SURVEY / "arms.json",
    SURVEY / "make-plan.py",
    SURVEY / "make-addenda.py",
    RESULTS / "run-campaign.sh",
]


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def output(command, **kwargs):
    return subprocess.run(
        command, check=True, capture_output=True, text=True, **kwargs
    ).stdout.strip()


def build_identity(args):
    build_command = (
        "CARGO_CI_NO_SCCACHE=1 GF2_AFF3CT_ROOT=<aff3ct root> "
        'CARGO_TARGET_DIR="$PWD/target/ldpc-update-arms" RUSTFLAGS=\'-C target-cpu=native\' '
        "./scripts/cargo-budget.sh cargo +1.95 build --offline --release --features aff3ct "
        "--manifest-path dev/active/07ca8585/survey/arms/Cargo.toml"
    )
    return {
        "schema": "ldpc-update-kernel-build-identity-v1",
        "observed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "rustc": output(["rustc", "+1.95", "--version", "--verbose"]),
        "cxx": output(["c++", "--version"]).splitlines()[0],
        "rustflags": "-C target-cpu=native",
        "cargo_profile": "release: opt-level 3, lto, codegen-units 1, debug line-tables-only",
        "build_command": build_command,
        "revision": output(["git", "rev-parse", "HEAD"]),
        "kernel": {
            "workspace": str(ARMS),
            "target_dir": str(args.kernel_dir),
            "note": (
                "its own workspace and its own target directory, so every executable the frozen "
                "whole-decoding confirmations were built from stays byte-identical"
            ),
            "executables": {name: sha(args.kernel_dir / name) for name in KERNEL_EXECUTABLES},
        },
        "profile_resample": {
            "target_dir": str(args.after_dir),
            "note": (
                "the `after` generation of the 3be770d5 harness, which the profile re-sampling "
                "series measures under the predecessor's own case set"
            ),
            "executables": {name: sha(args.after_dir / name) for name in PROFILE_EXECUTABLES},
        },
        "whole_decoding_identity": {
            "path": str(PREPARATION / "build-identity.json"),
            "sha256": sha(PREPARATION / "build-identity.json"),
        },
        "aff3ct": {
            "root": str(args.aff3ct),
            "commit": output(["git", "-C", str(args.aff3ct), "rev-parse", "HEAD"]),
            "static_library_sha256": sha(args.aff3ct / "build/lib/libaff3ct-4.7.0.a"),
            "source_archive": {
                "path": str(C077 / "source-inputs/aff3ct-source.tar.gz"),
                "sha256": sha(C077 / "source-inputs/aff3ct-source.tar.gz"),
            },
        },
        "shim": {
            "unit": str(ARMS / "cpp/update_rule_shim.cpp"),
            "unit_sha256": sha(ARMS / "cpp/update_rule_shim.cpp"),
            "includes_a_pinned_shim": False,
            "flags": ["-std=gnu++11", "-O3", "-march=native", "-g1", "-DNDEBUG"],
            "definitions_source": str(ARMS / "build.rs") + " AFF3CT_DEFINITIONS",
            "build_rs_sha256": sha(ARMS / "build.rs"),
        },
        "checknode_parity": {
            "path": str(PARITY),
            "sha256": sha(PARITY),
            "rows": [
                {
                    "code": row["code"],
                    "edges": row["edges"],
                    "checks": row["checks"],
                    "input_checksum": row["input_checksum"],
                    "output_checksum": row["gf2_output_checksum"],
                    "outputs_bit_identical": row["outputs_bit_identical"],
                }
                for row in (
                    json.loads(line) for line in PARITY.read_text().splitlines() if line.strip()
                )
            ],
        },
        "prepared_quality_fixed": {
            "note": (
                "produced by ldpc-fixed-quality from the frozen recorded frames with syndrome "
                "stopping off; every field but the observed peak RSS and the median per-frame "
                "latency is a deterministic function of the decoder and the recorded input"
            ),
            "files": {str(path): sha(path) for path in sorted(QUALITY_FIXED.glob("*.json"))},
        },
        "inputs": {
            "archive": {
                "path": str(C077 / "source-inputs/recorded-inputs.tar.gz"),
                "sha256": sha(C077 / "source-inputs/recorded-inputs.tar.gz"),
            },
            "extracted_to": str(args.inputs),
        },
        "harness_reused": {
            "path": str(HARNESS),
            "cargo_lock_sha256": sha(HARNESS / "Cargo.lock"),
            "note": "the 3be770d5 harness supplies the worker pool and the bundle loader",
        },
    }


def producing_inputs():
    base = json.loads((SURVEY / "producing-inputs.json").read_text())
    additions = [str(path) for path in ADDITIONS]
    base["behavior_sources"] = sorted(set(base["behavior_sources"] + additions))
    base["build_inputs"] = sorted(
        set(
            base["build_inputs"]
            + additions
            + [str(PREPARATION / "kernel-build-identity.json"), str(PARITY)]
            + [str(path) for path in sorted(QUALITY_FIXED.glob("*.json"))]
        )
    )
    return base


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--aff3ct", required=True, type=pathlib.Path)
    parser.add_argument("--kernel-dir", required=True, type=pathlib.Path)
    parser.add_argument("--after-dir", required=True, type=pathlib.Path)
    parser.add_argument("--inputs", required=True, type=pathlib.Path)
    args = parser.parse_args()
    identity = PREPARATION / "kernel-build-identity.json"
    identity.write_text(json.dumps(build_identity(args), indent=2) + "\n")
    manifest = SURVEY / "producing-inputs-kernel.json"
    manifest.write_text(json.dumps(producing_inputs(), indent=2) + "\n")
    print(identity, manifest)


if __name__ == "__main__":
    main()
