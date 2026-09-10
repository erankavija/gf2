#!/usr/bin/env python3
"""Writes the issue-owned producing-input manifest (jit:04b85d10).

The manifest names every repository file whose bytes decide what the consumer
arm measures: the three production crates the harness links, the harness
itself, the plan derivation, the launcher, the lock wrapper and the protocol
tooling. The runner snapshots every build input into each receipt and pins
the closure's digest, so a receipt is invalidated by a change to one of these
files and by nothing else.

`lifecycle_sources` is the subset whose bytes decide how a campaign is opened,
checkpointed, resumed and finalized; `behavior_sources` is the subset that
decides the measured behaviour; `build_inputs` is the whole closure. The
manifest validator requires lifecycle within behaviour within build inputs.

Usage: make-producing-inputs.py  (from the repository root)
"""

import json
import os
import pathlib

OUTPUT = pathlib.Path("dev/active/04b85d10/survey/producing-inputs.json")

PRODUCTION_CRATES = ["gf2-core", "gf2-coding", "gf2-kernels-simd"]

SURVEY = pathlib.Path("dev/active/04b85d10/survey")
TOOLING = pathlib.Path("dev/tools/tuning-campaign-support")

LIFECYCLE = [
    "dev/bench_results/04b85d10/run-consumer-campaigns.sh",
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
    "dev/active/04b85d10/survey/gf2-side/Cargo.lock",
    "dev/active/04b85d10/survey/gf2-side/Cargo.toml",
    "dev/tools/tuning-campaign-support/Cargo.toml",
]


def files_under(root):
    return sorted(
        str(path)
        for path in pathlib.Path(root).rglob("*")
        if path.is_file()
    )


def main():
    behavior = set()
    for crate in PRODUCTION_CRATES:
        behavior.update(files_under(f"crates/{crate}/src"))
    behavior.update(files_under(SURVEY / "gf2-side" / "src"))
    behavior.add(str(SURVEY / "build-plan.py"))
    behavior.update(files_under(TOOLING / "src"))
    behavior.update(LIFECYCLE)

    build_inputs = set(behavior)
    build_inputs.update(BUILD_ONLY)
    build_inputs.update(f"crates/{crate}/Cargo.toml" for crate in PRODUCTION_CRATES)

    for path in sorted(build_inputs):
        if not os.path.isfile(path):
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
