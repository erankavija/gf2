#!/usr/bin/env python3
"""Record this issue's timed-arm preparation (jit:f63a2464).

Writes two files from what this run observes:

* the build identity of the three timed arms — the toolchain, the build
  commands, the AFF3CT root and static library, and the SHA-256 of every
  executable a campaign may launch;
* the producing-input manifest a plan selects, derived from the shared base
  manifest and from Cargo's resolved local dependency graph rather than a
  hand-kept crate list.

Neither file carries a typed figure: every value comes from a command this run
executes or from a file it reads.

Usage (from the worktree root):
  record-preparation.py OUT_DIR --aff3ct-root DIR
      --baseline-dir DIR --candidate-dir DIR
"""

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys
import time

TOOLCHAIN = "1.95"
RUSTFLAGS = "-C target-cpu=native"

BASELINE_ARMS = ["gf2-throughput-arm", "aff3ct-throughput-arm"]
CANDIDATE_ARMS = ["qc-decoder-arm"]


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def run(command, **kwargs):
    return subprocess.run(command, check=True, capture_output=True, text=True, **kwargs).stdout


def output(command):
    return run(command).strip()


def located(*query):
    """Root-relative path the repository-file helper prints for `query`."""
    helper = output(["git", "-C", ROOT, "ls-files", "--cached", "--others", "--exclude-standard",
                     "--", ":(glob)**/repository_files.py"])
    return pathlib.Path(output([sys.executable, "-B", str(pathlib.Path(ROOT, helper)), *query]))


ROOT = output(["git", "rev-parse", "--show-toplevel"])
SURVEY = pathlib.Path(__file__).resolve().parent.relative_to(ROOT)
ARMS = SURVEY / "arms/Cargo.toml"


def local_closure(manifest_path, features):
    """Repository-local packages Cargo resolves for `manifest_path`."""
    command = [
        "./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "metadata", "--offline",
        "--locked", "--format-version", "1", "--manifest-path", str(manifest_path),
    ]
    if features:
        command += ["--features", features]
    metadata = json.loads(run(command))
    repo = pathlib.Path.cwd().resolve()
    resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
    sources, manifests = [], []
    for package in metadata["packages"]:
        manifest = pathlib.Path(package["manifest_path"])
        if package["id"] not in resolved or not manifest.is_relative_to(repo):
            continue
        directory = manifest.parent.relative_to(repo)
        sources += [str(path) for path in (directory / "src").rglob("*") if path.is_file()]
        manifests.append(str(manifest.relative_to(repo)))
        build = directory / "build.rs"
        if build.exists():
            sources.append(str(build))
    return sources, manifests


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out_dir", type=pathlib.Path)
    parser.add_argument("--aff3ct-root", required=True)
    parser.add_argument("--baseline-dir", required=True)
    parser.add_argument("--candidate-dir", required=True)
    args = parser.parse_args()

    if pathlib.Path.cwd() != pathlib.Path(ROOT):
        sys.exit("invoke from the worktree root")

    baseline = pathlib.Path(args.baseline_dir)
    candidate = pathlib.Path(args.candidate_dir)
    executables = {}
    for name in BASELINE_ARMS:
        executables[name] = {"generation": "baseline", "sha256": digest(baseline / name)}
    for name in CANDIDATE_ARMS:
        executables[name] = {"generation": "candidate", "sha256": digest(candidate / name)}

    static_library = next(
        pathlib.Path(args.aff3ct_root).rglob("libaff3ct*.a"), None
    )
    baseline_harness = located("package-directory", "ldpc-throughput-harness")
    identity = {
        "schema": "ldpc-candidate-build-identity-v1",
        "observed_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "rustc": run(["rustc", f"+{TOOLCHAIN}", "--version", "--verbose"]).strip(),
        "cxx": run(["c++", "--version"]).splitlines()[0],
        "rustflags": RUSTFLAGS,
        "cargo_profile": "release: opt-level 3, lto, codegen-units 1, debug line-tables-only",
        "build_commands": {
            "baseline": (
                f"CARGO_CI_NO_SCCACHE=1 GF2_AFF3CT_ROOT=<aff3ct root> "
                f"CARGO_TARGET_DIR=\"$PWD/{baseline.parent.name}\" RUSTFLAGS='{RUSTFLAGS}' "
                f"./scripts/cargo-budget.sh cargo +{TOOLCHAIN} build --offline --release "
                f"--features aff3ct --manifest-path {baseline_harness / 'Cargo.toml'}"
            ),
            "candidate": (
                f"CARGO_TARGET_DIR=\"$PWD/{candidate.parent.name}\" RUSTFLAGS='{RUSTFLAGS}' "
                f"./scripts/cargo-budget.sh cargo +{TOOLCHAIN} build --offline --release "
                f"--manifest-path {ARMS}"
            ),
        },
        "directories": {"baseline": str(baseline), "candidate": str(candidate)},
        "executables": executables,
        "aff3ct": {
            "root": args.aff3ct_root,
            "static_library": None if static_library is None else str(static_library),
            "static_library_sha256": None if static_library is None else digest(static_library),
        },
        "inputs": {
            "archive": "dev/bench_results/c077a88b/v3-preparation/source-inputs/recorded-inputs.tar.gz",
            "archive_sha256": digest(
                "dev/bench_results/c077a88b/v3-preparation/source-inputs/recorded-inputs.tar.gz"
            ),
            "extracted_to": "target/ldpc-inputs",
        },
        "prepared_quality": {
            str(path): digest(path)
            for path in sorted(
                pathlib.Path("dev/bench_results/c077a88b/v3-preparation/quality").glob("*.json")
            )
        },
    }

    args.out_dir.mkdir(parents=True, exist_ok=True)
    (args.out_dir / "build-identity.json").write_text(
        json.dumps(identity, indent=2) + "\n", encoding="utf-8"
    )

    base = json.loads(located("shared-producing-manifest").read_text(encoding="utf-8"))
    survey_sources = [
        str(path)
        for path in SURVEY.rglob("*")
        if path.is_file() and path.suffix in {".rs", ".py", ".sh"} and "target" not in path.parts
    ]
    behavior, manifests = local_closure(ARMS, None)
    baseline_behavior, baseline_manifests = local_closure(
        baseline_harness / "Cargo.toml", "aff3ct"
    )
    behavior += baseline_behavior + survey_sources
    manifests += baseline_manifests
    base["behavior_sources"] = sorted(
        set(base["behavior_sources"])
        | {path for path in behavior if not path.endswith(("summarize-quality.py",))}
    )
    base["build_inputs"] = sorted(
        set(base["build_inputs"])
        | set(behavior)
        | set(manifests)
        | {
            str(baseline_harness / "Cargo.lock"),
            str(args.out_dir / "build-identity.json"),
            identity["inputs"]["archive"],
        }
        | set(identity["prepared_quality"])
    )
    (SURVEY / "producing-inputs.json").write_text(
        json.dumps(base, indent=2) + "\n", encoding="utf-8"
    )
    print((args.out_dir / "build-identity.json").as_posix())
    print((SURVEY / "producing-inputs.json").as_posix())


if __name__ == "__main__":
    main()
