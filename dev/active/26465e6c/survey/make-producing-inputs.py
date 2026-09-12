#!/usr/bin/env python3
"""Writes the issue-owned producing-input manifest (jit:26465e6c).

The manifest names every repository file whose bytes decide what an arm of
either family measures: the gf2 sources the arm binary links, the survey
harness with its vendored external kernels and build script, the plan
projection, the launcher, the lock wrapper and the protocol tooling. The runner
snapshots each build input into every receipt and pins the closure's digest,
so a receipt is invalidated by a change to one of these files and by nothing
else.

`lifecycle_sources` decide how a campaign is opened, checkpointed, resumed and
finalized; `behavior_sources` decide the measured behaviour; `build_inputs` is
the whole closure. The runner requires lifecycle within behaviour within build
inputs.

Usage: make-producing-inputs.py  (from the repository root)
"""

import json
import pathlib

SURVEY = pathlib.Path("dev/active/26465e6c/survey")
OUTPUT = SURVEY / "producing-inputs.json"
LAUNCHER = "dev/bench_results/26465e6c/run-campaign.sh"

RUST_SOURCES = [
    "crates/gf2-core/src",
    "crates/gf2-kernels-simd/src",
    "dev/tools/tuning-campaign-support/src",
    str(SURVEY / "gf2-side/src"),
]
SURVEY_BEHAVIOR = [
    str(SURVEY / "families.py"),
    str(SURVEY / "build-plan.py"),
    str(SURVEY / "gf2-side/build.rs"),
    str(SURVEY / "gf2-side/csrc/libpopcnt_kernels.c"),
    str(SURVEY / "gf2-side/csrc/mula_kernels.cpp"),
    str(SURVEY / "vendor/libpopcnt/libpopcnt.h"),
    str(SURVEY / "vendor/mula/config.h"),
    str(SURVEY / "vendor/mula/popcnt-avx2-harley-seal.cpp"),
    str(SURVEY / "vendor/mula/popcnt-lookup.cpp"),
    LAUNCHER,
    "dev/scripts/ccx1-bench-flock.sh",
]
LIFECYCLE = [
    LAUNCHER,
    "dev/scripts/ccx1-bench-flock.sh",
    "dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs",
    "dev/tools/tuning-campaign-support/src/journal.rs",
    "dev/tools/tuning-campaign-support/src/process.rs",
    "dev/tools/tuning-campaign-support/src/protocol.rs",
    "dev/tools/tuning-campaign-support/src/provenance.rs",
    "dev/tools/tuning-campaign-support/src/receipt.rs",
    "dev/tools/tuning-campaign-support/src/trial_ledger.rs",
]
BUILD_ONLY = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/gf2-core/Cargo.toml",
    "crates/gf2-kernels-simd/Cargo.toml",
    "dev/tools/tuning-campaign-support/Cargo.toml",
    str(SURVEY / "gf2-side/Cargo.lock"),
    str(SURVEY / "gf2-side/Cargo.toml"),
    str(SURVEY / "vendor/libpopcnt/LICENSE"),
    str(SURVEY / "vendor/mula/LICENSE"),
]


def rust_files(root):
    return [str(path) for path in pathlib.Path(root).rglob("*.rs") if path.is_file()]


def main():
    behavior = set(SURVEY_BEHAVIOR) | set(LIFECYCLE)
    for root in RUST_SOURCES:
        behavior.update(rust_files(root))
    build_inputs = behavior | set(BUILD_ONLY)
    for path in sorted(build_inputs):
        if not pathlib.Path(path).is_file():
            raise SystemExit(f"manifest names a missing regular file: {path}")
    manifest = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(LIFECYCLE),
        "build_inputs": sorted(build_inputs),
    }
    OUTPUT.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(
        f"{OUTPUT}: {len(manifest['build_inputs'])} build inputs, "
        f"{len(manifest['behavior_sources'])} behaviour sources, "
        f"{len(manifest['lifecycle_sources'])} lifecycle sources"
    )


if __name__ == "__main__":
    main()
