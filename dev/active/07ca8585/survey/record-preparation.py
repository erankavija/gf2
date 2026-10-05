#!/usr/bin/env python3
"""Record the build identity and producing-input closure of this issue (jit:07ca8585).

Run after both release builds and before preparing any timed plan. Writes
`dev/bench_results/07ca8585/preparation/build-identity.json` with the observed
toolchain, the AFF3CT source and static-library identity, the shim flags, the
executable digests of both gf2 generations and the recorded-input identities,
and `survey/producing-inputs.json`, the manifest each runner plan selects. The
runner snapshots and digests every file the manifest names before its first
measurement.

The measured harness is `3be770d5`'s, unchanged, so this issue's producing
closure is that survey's closure plus this issue's arm catalogue, plan
generator and campaign launcher. The `before` arm is built from an export of a
pinned revision rather than from the working tree; it is identified by that
revision and by its executable's digest, which every campaign checks.

Usage (from the worktree root):
  record-preparation.py --aff3ct DIR --before-rev REV --before-dir DIR
      --after-dir DIR --inputs DIR
"""

import argparse
import datetime
import hashlib
import json
import pathlib
import subprocess

ISSUE = "07ca8585"
SURVEY = pathlib.Path("dev/active") / ISSUE / "survey"
PREPARATION = pathlib.Path("dev/bench_results") / ISSUE / "preparation"
HARNESS = pathlib.Path("dev/active/3be770d5/survey/harness")
SHIM = pathlib.Path("dev/active/c077a88b/survey/harness/cpp/aff3ct_shim.cpp")
C077 = pathlib.Path("dev/bench_results/c077a88b/v3-preparation")
LAUNCHER = pathlib.Path("dev/bench_results") / ISSUE / "run-campaign.sh"
GF2_EXECUTABLES = ["gf2-throughput-arm", "ldpc-alloc-census"]
AFF3CT_EXECUTABLES = ["aff3ct-throughput-arm", "ldpc-throughput-validate"]
BEHAVIOR_ADDITIONS = [SURVEY / "arms.json", SURVEY / "make-plan.py", LAUNCHER]


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def output(command, **kwargs):
    return subprocess.run(
        command, check=True, capture_output=True, text=True, **kwargs
    ).stdout.strip()


def build_identity(args):
    bundles = {}
    for manifest in sorted(args.inputs.glob("*/manifest.json")):
        record = json.loads(manifest.read_text())
        bundles[manifest.parent.name] = {
            key: record[key]
            for key in (
                "code", "n", "k", "m", "nnz", "h_sha256", "codewords_sha256",
                "llrs_sha256", "frames", "seed", "esn0_db", "codeword_source",
                "punctured_prefix",
            )
        }
    build_command = (
        "CARGO_CI_NO_SCCACHE=1 GF2_AFF3CT_ROOT=<aff3ct root> "
        'CARGO_TARGET_DIR="$PWD/<target dir>" RUSTFLAGS=\'-C target-cpu=native\' '
        "./scripts/cargo-budget.sh cargo +1.95 build --offline --release --features aff3ct "
        "--manifest-path <tree>/dev/active/3be770d5/survey/harness/Cargo.toml"
    )
    return {
        "schema": "ldpc-update-build-identity-v1",
        "observed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "rustc": output(["rustc", "+1.95", "--version", "--verbose"]),
        "cxx": output(["c++", "--version"]).splitlines()[0],
        "rustflags": "-C target-cpu=native",
        "cargo_profile": "release: opt-level 3, lto, codegen-units 1, debug line-tables-only",
        "build_command": build_command,
        "generations": {
            "before": {
                "revision": output(["git", "rev-parse", args.before_rev]),
                "tree": "an export of that revision under target/before-tree, built in place",
                "target_dir": str(args.before_dir),
                "executables": {name: sha(args.before_dir / name) for name in GF2_EXECUTABLES},
            },
            "after": {
                "revision": output(["git", "rev-parse", "HEAD"]),
                "tree": "the worktree",
                "target_dir": str(args.after_dir),
                "executables": {
                    name: sha(args.after_dir / name)
                    for name in GF2_EXECUTABLES + AFF3CT_EXECUTABLES
                },
            },
        },
        "harness": {
            "path": str(HARNESS),
            "cargo_lock_sha256": sha(HARNESS / "Cargo.lock"),
            "note": "the 3be770d5 steady-state harness, unchanged by this issue",
        },
        "aff3ct": {
            "root": str(args.aff3ct),
            "commit": output(["git", "-C", str(args.aff3ct), "rev-parse", "HEAD"]),
            "static_library_sha256": sha(args.aff3ct / "build/lib/libaff3ct-4.7.0.a"),
            "c077a88b_build_identity": {
                "path": str(C077 / "build-identity.json"),
                "sha256": sha(C077 / "build-identity.json"),
            },
            "source_archive": {
                "path": str(C077 / "source-inputs/aff3ct-source.tar.gz"),
                "sha256": sha(C077 / "source-inputs/aff3ct-source.tar.gz"),
            },
        },
        "shim": {
            "unit": str(HARNESS / "cpp/throughput_shim.cpp"),
            "unit_sha256": sha(HARNESS / "cpp/throughput_shim.cpp"),
            "included": str(SHIM),
            "included_sha256": sha(SHIM),
            "flags": ["-std=gnu++11", "-O3", "-march=native", "-g1", "-DNDEBUG"],
            "definitions_source": "dev/active/3be770d5/survey/harness/build.rs AFF3CT_DEFINITIONS",
            "build_rs_sha256": sha(HARNESS / "build.rs"),
        },
        "inputs": {
            "archive": {
                "path": str(C077 / "source-inputs/recorded-inputs.tar.gz"),
                "sha256": sha(C077 / "source-inputs/recorded-inputs.tar.gz"),
            },
            "extracted_to": str(args.inputs),
            "bundles": bundles,
        },
        "prepared_quality": {
            str(path): sha(path) for path in sorted((C077 / "quality").glob("*.json"))
        },
    }


def producing_inputs():
    base = json.loads((pathlib.Path("dev/active/3be770d5/survey") / "producing-inputs.json").read_text())
    additions = [str(path) for path in BEHAVIOR_ADDITIONS]
    base["behavior_sources"] = sorted(set(base["behavior_sources"] + additions))
    base["build_inputs"] = sorted(set(
        base["build_inputs"] + additions + [str(PREPARATION / "build-identity.json")]
    ))
    return base


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--aff3ct", required=True, type=pathlib.Path)
    parser.add_argument("--before-rev", required=True)
    parser.add_argument("--before-dir", required=True, type=pathlib.Path)
    parser.add_argument("--after-dir", required=True, type=pathlib.Path)
    parser.add_argument("--inputs", required=True, type=pathlib.Path)
    args = parser.parse_args()
    PREPARATION.mkdir(parents=True, exist_ok=True)
    identity = build_identity(args)
    (PREPARATION / "build-identity.json").write_text(json.dumps(identity, indent=2) + "\n")
    (SURVEY / "producing-inputs.json").write_text(json.dumps(producing_inputs(), indent=2) + "\n")
    print(PREPARATION / "build-identity.json", SURVEY / "producing-inputs.json")


if __name__ == "__main__":
    main()
