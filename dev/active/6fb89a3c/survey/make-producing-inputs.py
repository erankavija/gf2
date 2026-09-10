#!/usr/bin/env python3
"""Write the issue-owned producing-input manifest for jit:6fb89a3c.

The manifest selects every repository file whose bytes can affect a receipt
of this survey: the shared runner and acceptance tool, the exact local crate
closure of the gf2-side arm crate (from Cargo's resolved metadata, so a new
transitive workspace dependency cannot fall out of the closure silently), the
C harness sources and their build scripts, the campaign launcher, the
committed generator-polynomial input the M4RI generator-matrix arm reads,
and the build evidence that pins the external library digests the harness
binaries were linked against. Evidence outputs, reports and this generator
are not producing inputs and are excluded.

Usage: dev/active/6fb89a3c/survey/make-producing-inputs.py  (from the repo root)
"""
from __future__ import annotations

import json
import pathlib
import subprocess

REPO = pathlib.Path.cwd().resolve()
ISSUE = pathlib.Path("dev/active/6fb89a3c")
SURVEY = ISSUE / "survey"
LAUNCHER = "dev/bench_results/6fb89a3c/run-campaign.sh"
WRAPPER = "dev/scripts/ccx1-bench-flock.sh"
GENERATORS = "dev/bench_results/4e732b56/generators.txt"
EXCLUDED_SURVEY = {"make-producing-inputs.py", "summarize.py"}
SOURCE_SUFFIXES = {".rs", ".c", ".h", ".py", ".sh"}
LIFECYCLE_BASENAMES = {
    "benchmark-ab-runner.rs", "journal.rs", "process.rs", "protocol.rs",
    "provenance.rs", "receipt.rs", "trial_ledger.rs",
}


def tracked(path: pathlib.Path) -> bool:
    return "target" not in path.parts and "__pycache__" not in path.parts


def main() -> None:
    behavior: set[str] = set()
    build: set[str] = set()
    for path in SURVEY.rglob("*"):
        if not path.is_file() or not tracked(path) or path.name in EXCLUDED_SURVEY:
            continue
        relative = str(path)
        if path.suffix in SOURCE_SUFFIXES or path.name == "Makefile":
            behavior.add(relative)
        elif path.name in {"Cargo.toml", "Cargo.lock", "build-evidence.json"}:
            build.add(relative)
    behavior.update({LAUNCHER, WRAPPER, GENERATORS})

    metadata = json.loads(subprocess.check_output([
        "./scripts/cargo-budget.sh", "cargo", "+1.95.0", "metadata", "--offline", "--locked",
        "--format-version", "1", "--manifest-path", str(SURVEY / "gf2-side/Cargo.toml"),
    ]))
    resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
    for package in metadata["packages"]:
        manifest = pathlib.Path(package["manifest_path"])
        if package["id"] not in resolved or not manifest.is_relative_to(REPO):
            continue
        directory = manifest.parent.relative_to(REPO)
        for source in (directory / "src").rglob("*"):
            if source.is_file() and tracked(source):
                behavior.add(str(source))
        build.add(str(manifest.relative_to(REPO)))
        build_script = directory / "build.rs"
        if build_script.exists():
            behavior.add(str(build_script))

    lifecycle = {
        path for path in behavior
        if pathlib.Path(path).name in LIFECYCLE_BASENAMES and "tuning-campaign-support" in path
    } | {LAUNCHER, WRAPPER}
    build |= behavior | {".cargo/config.toml", "Cargo.lock", "Cargo.toml"}
    manifest_document = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(lifecycle),
        "build_inputs": sorted(build),
    }
    for paths in manifest_document.values():
        if isinstance(paths, list):
            for path in paths:
                if not pathlib.Path(path).is_file():
                    raise SystemExit(f"producing input is not a regular file: {path}")
    (ISSUE / "producing-inputs.json").write_text(json.dumps(manifest_document, indent=2) + "\n")
    print(f"{len(behavior)} behavior, {len(lifecycle)} lifecycle, {len(build)} build inputs -> {ISSUE / 'producing-inputs.json'}")


if __name__ == "__main__":
    main()
