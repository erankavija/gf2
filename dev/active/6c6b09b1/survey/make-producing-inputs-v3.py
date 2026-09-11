#!/usr/bin/env python3
"""Write the producing-input manifest of the byte-field v3 campaigns (jit:6c6b09b1).

Usage: make-producing-inputs-v3.py [output]   (default survey/producing-inputs-v3.json)

The manifest selects every repository file whose bytes can change what the
survey's campaigns measure or how they run: the gf2 production crates the gf2
arm compiles, the survey's arm crates and C shim, the plan projection, the
launcher, the shared lock wrapper and the shared campaign tooling
(behavior); the subset that decides campaign lifecycle and resume
(lifecycle); and the behavior set plus manifests, lock files, build
configuration, the external-prefix digests, the conformance and provenance
tools, and the committed pre-timing evidence under conformance-v3/ (build
inputs). Source lists are derived from the tree, so an added source file
enters the closure without an edit here.
"""

import os
import subprocess
import sys
import json

ISSUE = "dev/active/6c6b09b1"
SURVEY = f"{ISSUE}/survey"
EVIDENCE = f"{ISSUE}/conformance-v3"
TOOL = "dev/tools/tuning-campaign-support"
LAUNCHER = "dev/bench_results/6c6b09b1/run-byte-field-v3.sh"
LOCK_WRAPPER = "dev/scripts/ccx1-bench-flock.sh"

LIFECYCLE = [
    LAUNCHER,
    LOCK_WRAPPER,
    f"{TOOL}/src/bin/benchmark-ab-runner.rs",
    f"{TOOL}/src/campaign.rs",
    f"{TOOL}/src/host.rs",
    f"{TOOL}/src/journal.rs",
    f"{TOOL}/src/process.rs",
    f"{TOOL}/src/protocol.rs",
    f"{TOOL}/src/provenance.rs",
    f"{TOOL}/src/receipt.rs",
    f"{TOOL}/src/transport.rs",
    f"{TOOL}/src/trial_ledger.rs",
]

BEHAVIOR_EXTRA = [
    LAUNCHER,
    LOCK_WRAPPER,
    f"{SURVEY}/byte_field_ext.c",
    f"{SURVEY}/byte_field_ext.h",
    f"{SURVEY}/ext-side/build.rs",
    f"{SURVEY}/make-plan-v3.py",
]

BUILD_EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/gf2-core/Cargo.toml",
    "crates/gf2-kernels-simd/Cargo.toml",
    "scripts/cargo-budget.sh",
    f"{TOOL}/Cargo.toml",
    f"{SURVEY}/Makefile",
    f"{SURVEY}/arm-common/Cargo.toml",
    f"{SURVEY}/arm-provenance.sh",
    f"{SURVEY}/backend_provenance.c",
    f"{SURVEY}/byte_field_conformance.c",
    f"{SURVEY}/ext-prefix.sha256",
    f"{SURVEY}/ext-side/Cargo.lock",
    f"{SURVEY}/ext-side/Cargo.toml",
    f"{SURVEY}/fetch-build.sh",
    f"{SURVEY}/field-laws/Cargo.lock",
    f"{SURVEY}/field-laws/Cargo.toml",
    f"{SURVEY}/field-laws/src/main.rs",
    f"{SURVEY}/gf2-side/Cargo.lock",
    f"{SURVEY}/gf2-side/Cargo.toml",
    f"{SURVEY}/make-producing-inputs-v3.py",
    f"{SURVEY}/stage-externals.sh",
    f"{EVIDENCE}/arm-provenance.txt",
    f"{EVIDENCE}/build-record.txt",
    f"{EVIDENCE}/ext-wrapper.txt",
    f"{EVIDENCE}/externals.txt",
    f"{EVIDENCE}/field-laws-element.txt",
    f"{EVIDENCE}/field-laws-wide.txt",
    f"{EVIDENCE}/gf2-side.txt",
    f"{EVIDENCE}/stage-externals.txt",
]


def rust_sources(root, directory):
    found = []
    for base, _, files in os.walk(os.path.join(root, directory)):
        for name in files:
            if name.endswith(".rs"):
                found.append(os.path.relpath(os.path.join(base, name), root))
    return found


def main():
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        root, SURVEY, "producing-inputs-v3.json")
    behavior = set(BEHAVIOR_EXTRA)
    for directory in ("crates/gf2-core/src", "crates/gf2-kernels-simd/src",
                      f"{SURVEY}/arm-common/src", f"{SURVEY}/gf2-side/src",
                      f"{SURVEY}/ext-side/src", f"{TOOL}/src"):
        behavior.update(rust_sources(root, directory))
    build = behavior | set(BUILD_EXTRA)
    manifest = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(LIFECYCLE),
        "build_inputs": sorted(build),
    }
    for path in manifest["build_inputs"]:
        if not os.path.isfile(os.path.join(root, path)):
            raise SystemExit(f"missing producing input {path}")
    with open(output, "w") as handle:
        json.dump(manifest, handle, indent=2)
        handle.write("\n")
    print(f"{output}: {len(manifest['behavior_sources'])} behavior, "
          f"{len(manifest['lifecycle_sources'])} lifecycle, "
          f"{len(manifest['build_inputs'])} build inputs")


if __name__ == "__main__":
    main()
