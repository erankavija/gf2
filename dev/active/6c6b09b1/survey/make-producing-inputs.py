#!/usr/bin/env python3
"""Write the producing-input manifest of one byte-field campaign generation (jit:6c6b09b1).

Usage: make-producing-inputs.py PROTOCOL_VERSION [output]
       (default survey/producing-inputs-v<PROTOCOL_VERSION>.json)

The manifest selects every repository file whose bytes can change what the
survey's campaigns measure or how they run: the gf2 production crates the gf2
arm compiles, the survey's arm crates and C shim, the plan projection, the
launcher, the shared lock wrapper and the shared campaign tooling
(behavior); the subset that decides campaign lifecycle and resume
(lifecycle); and the behavior set plus manifests, lock files, build
configuration, the external-prefix digests, the conformance and provenance
tools, and the committed pre-timing evidence of this generation (build
inputs). Source lists are derived from the tree, so an added source file
enters the closure without an edit here.
"""

import os
import subprocess
import sys
import json

ISSUE = "dev/active/6c6b09b1"
SURVEY = f"{ISSUE}/survey"
TOOL = "dev/tools/tuning-campaign-support"
LOCK_WRAPPER = "dev/scripts/ccx1-bench-flock.sh"


def evidence_dir(version):
    """Pre-timing evidence directory of one campaign generation."""
    return f"{ISSUE}/conformance-v{version}"


def launcher(version):
    """Launcher of one campaign generation."""
    return f"dev/bench_results/6c6b09b1/run-byte-field-v{version}.sh"


LIFECYCLE = [
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
    LOCK_WRAPPER,
    f"{SURVEY}/byte_field_ext.c",
    f"{SURVEY}/byte_field_ext.h",
    f"{SURVEY}/ext-side/build.rs",
    f"{SURVEY}/make-plan-versioned.py",
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
    f"{SURVEY}/make-producing-inputs.py",
    f"{SURVEY}/stage-externals.sh",
]

EVIDENCE_FILES = [
    "arm-provenance.txt",
    "build-record.txt",
    "ext-wrapper.txt",
    "externals.txt",
    "field-laws-element.txt",
    "field-laws-wide.txt",
    "gf2-side.txt",
    "stage-externals.txt",
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
    if len(sys.argv) not in (2, 3):
        raise SystemExit(__doc__.strip())
    version = int(sys.argv[1])
    output = sys.argv[2] if len(sys.argv) > 2 else os.path.join(
        root, SURVEY, f"producing-inputs-v{version}.json")
    evidence = evidence_dir(version)
    behavior = set(BEHAVIOR_EXTRA) | {launcher(version)}
    for directory in ("crates/gf2-core/src", "crates/gf2-kernels-simd/src",
                      f"{SURVEY}/arm-common/src", f"{SURVEY}/gf2-side/src",
                      f"{SURVEY}/ext-side/src", f"{TOOL}/src"):
        behavior.update(rust_sources(root, directory))
    build = behavior | set(BUILD_EXTRA) | {
        f"{evidence}/{name}" for name in EVIDENCE_FILES}
    manifest = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(LIFECYCLE + [launcher(version)]),
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
