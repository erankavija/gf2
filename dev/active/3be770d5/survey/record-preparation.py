#!/usr/bin/env python3
"""Record the build identity and producing-input closure of the survey (jit:3be770d5).

Run after the release build and before preparing any timed plan. Writes
`dev/bench_results/3be770d5/preparation/build-identity.json` with the observed
toolchain, AFF3CT source and static-library identity, shim flags, executable
digests and recorded-input identities, and `survey/producing-inputs.json`, the
manifest each runner plan selects. The runner snapshots and digests every file
the manifest names before its first measurement.

The producing closure follows Cargo's resolved local dependency graph of the
harness, plus the `c077a88b` shim the harness includes, the arm catalogue,
plan generator, campaign launcher and prepared quality. Generators that do
not run during measurement (addenda, summaries, structural counts, this
script) stay outside the behavior sources.

Usage (from the worktree root, under scripts/cargo-budget.sh):
  record-preparation.py --aff3ct DIR --bin-dir DIR --inputs DIR
"""

import argparse
import datetime
import hashlib
import json
import pathlib
import subprocess

ISSUE = "3be770d5"
SURVEY = pathlib.Path("dev/active") / ISSUE / "survey"
PREPARATION = pathlib.Path("dev/bench_results") / ISSUE / "preparation"
C077 = pathlib.Path("dev/bench_results/c077a88b/v3-preparation")
SHIM = pathlib.Path("dev/active/c077a88b/survey/harness/cpp/aff3ct_shim.cpp")
EXECUTABLES = ["gf2-throughput-arm", "aff3ct-throughput-arm", "ldpc-profile", "ldpc-plan-check",
               "ldpc-alloc-census", "ldpc-throughput-validate"]
NOT_BEHAVIOR = {"make-addenda.py", "record-preparation.py", "summarize-profile.py",
                "summarize.py", "edge-costs.py", "freeze-addendum.py", "validate.py",
                "validate-arms.py", "intervals.py",
                "run-profile.sh", "profile-session.sh", "profile-cases.tsv"}


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def output(command, **kwargs):
    return subprocess.run(command, check=True, capture_output=True, text=True, **kwargs).stdout.strip()


def build_identity(args):
    build_rs = (SURVEY / "harness/build.rs").read_text()
    bundles = {}
    for manifest in sorted(args.inputs.glob("*/manifest.json")):
        record = json.loads(manifest.read_text())
        bundles[manifest.parent.name] = {key: record[key] for key in (
            "code", "n", "k", "m", "nnz", "h_sha256", "codewords_sha256", "llrs_sha256",
            "frames", "seed", "esn0_db", "codeword_source", "punctured_prefix")}
    return {
        "schema": "ldpc-throughput-build-identity-v1",
        "observed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "rustc": output(["rustc", "+1.95", "--version", "--verbose"]),
        "cxx": output(["c++", "--version"]).splitlines()[0],
        "rustflags": "-C target-cpu=native",
        "cargo_profile": "release: opt-level 3, lto, codegen-units 1, debug line-tables-only",
        "build_command": (
            "CARGO_CI_NO_SCCACHE=1 GF2_AFF3CT_ROOT=<aff3ct root> "
            "CARGO_TARGET_DIR=\"$PWD/target/ldpc-throughput\" RUSTFLAGS='-C target-cpu=native' "
            "./scripts/cargo-budget.sh cargo +1.95 build --offline --release --features aff3ct "
            "--manifest-path dev/active/3be770d5/survey/harness/Cargo.toml"),
        "aff3ct": {
            "root": str(args.aff3ct),
            "commit": output(["git", "-C", str(args.aff3ct), "rev-parse", "HEAD"]),
            "static_library_sha256": sha(args.aff3ct / "build/lib/libaff3ct-4.7.0.a"),
            "c077a88b_build_identity": {"path": str(C077 / "build-identity.json"),
                                        "sha256": sha(C077 / "build-identity.json")},
            "source_archive": {"path": str(C077 / "source-inputs/aff3ct-source.tar.gz"),
                               "sha256": sha(C077 / "source-inputs/aff3ct-source.tar.gz")},
        },
        "shim": {
            "unit": str(SURVEY / "harness/cpp/throughput_shim.cpp"),
            "unit_sha256": sha(SURVEY / "harness/cpp/throughput_shim.cpp"),
            "included": str(SHIM),
            "included_sha256": sha(SHIM),
            "flags": ["-std=gnu++11", "-O3", "-march=native", "-g1", "-DNDEBUG"],
            "definitions_source": "dev/active/3be770d5/survey/harness/build.rs AFF3CT_DEFINITIONS",
            "build_rs_sha256": hashlib.sha256(build_rs.encode()).hexdigest(),
        },
        "executables": {name: sha(args.bin_dir / name) for name in EXECUTABLES},
        "inputs": {
            "archive": {"path": str(C077 / "source-inputs/recorded-inputs.tar.gz"),
                        "sha256": sha(C077 / "source-inputs/recorded-inputs.tar.gz")},
            "extracted_to": str(args.inputs),
            "bundles": bundles,
            "generator": "dev/active/c077a88b/survey/harness/src/bin/ldpc-make-inputs.rs",
            "generator_sha256": sha("dev/active/c077a88b/survey/harness/src/bin/ldpc-make-inputs.rs"),
            "rng": ("channel noise: gf2_sim::testutil::AwgnLlrSource, an in-repository SplitMix64 "
                    "stream (crates/gf2-sim/src/testutil.rs) seeded with the manifest seed; random "
                    "messages: the generator's own LCG seeded with seed ^ 0x6C64706D73677367. "
                    "Neither uses an external RNG crate."),
            "testutil_sha256": sha("crates/gf2-sim/src/testutil.rs"),
        },
        "prepared_quality": {str(path): sha(path) for path in sorted((C077 / "quality").glob("*.json"))},
    }


def producing_inputs():
    base = json.loads(pathlib.Path("dev/active/f547c394/producing-inputs.json").read_text())
    behavior = [str(path) for path in SURVEY.rglob("*")
                if path.is_file() and "target" not in path.parts
                and path.suffix in {".rs", ".cpp", ".py", ".sh", ".json"}
                and path.name not in NOT_BEHAVIOR and path.name != "producing-inputs.json"]
    metadata = json.loads(output([
        "./scripts/cargo-budget.sh", "cargo", "+1.95", "metadata", "--offline", "--locked",
        "--format-version", "1", "--manifest-path", str(SURVEY / "harness/Cargo.toml"),
        "--features", "aff3ct"]))
    repo = pathlib.Path.cwd().resolve()
    resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
    build_inputs = []
    for package in metadata["packages"]:
        manifest = pathlib.Path(package["manifest_path"])
        if package["id"] not in resolved or not manifest.is_relative_to(repo):
            continue
        directory = manifest.parent.relative_to(repo)
        behavior += [str(path) for path in (directory / "src").rglob("*") if path.is_file()]
        build_inputs.append(str(manifest.relative_to(repo)))
        if (directory / "build.rs").exists():
            behavior.append(str(directory / "build.rs"))
    behavior += [str(SHIM), "dev/bench_results/3be770d5/run-campaign.sh"]
    base["behavior_sources"] = sorted(set(base["behavior_sources"] + behavior))
    base["build_inputs"] = sorted(set(
        base["build_inputs"] + behavior + build_inputs
        + [str(SURVEY / "harness/Cargo.lock"), str(PREPARATION / "build-identity.json")]
        + [str(path) for path in sorted((C077 / "quality").glob("*.json"))]))
    return base


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--aff3ct", required=True, type=pathlib.Path)
    parser.add_argument("--bin-dir", required=True, type=pathlib.Path)
    parser.add_argument("--inputs", required=True, type=pathlib.Path)
    args = parser.parse_args()
    PREPARATION.mkdir(parents=True, exist_ok=True)
    identity = build_identity(args)
    (PREPARATION / "build-identity.json").write_text(json.dumps(identity, indent=2) + "\n")
    (SURVEY / "producing-inputs.json").write_text(json.dumps(producing_inputs(), indent=2) + "\n")
    print(PREPARATION / "build-identity.json", SURVEY / "producing-inputs.json")


if __name__ == "__main__":
    main()
