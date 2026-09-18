#!/usr/bin/env python3
"""Write the producing-input manifest of this family's campaigns (jit:ad2a6a58).

Usage: make-producing-inputs.py [output]
       (default dev/active/ad2a6a58/survey/producing-inputs.json)

The manifest selects every repository file whose bytes can change what a
campaign measures or how it runs: the gf2 production crates the arm compiles,
the harness crates (this family's arm and the two byte-field survey crates it
reuses), the plan projection, the launcher, the shared lock wrapper and the
shared campaign tooling (behavior); the subset that decides campaign lifecycle
and resume (lifecycle); and the behavior set plus manifests, lock files, build
configuration and the committed pre-timing evidence, correctness and smoke
alike (build inputs). Source lists are derived from the tree, so an added
source file enters the closure without an edit here.
"""

import json
import os
import subprocess
import sys

ISSUE = "dev/active/ad2a6a58"
SURVEY = f"{ISSUE}/survey"
ARM = f"{SURVEY}/axpy-arm"
REUSED = "dev/active/6c6b09b1/survey"
TOOL = "dev/tools/tuning-campaign-support"
LOCK_WRAPPER = "dev/scripts/ccx1-bench-flock.sh"
LAUNCHER = "dev/bench_results/ad2a6a58/run-axpy-confirmation.sh"
EVIDENCE = f"{ISSUE}/conformance"

LIFECYCLE = [
    LOCK_WRAPPER,
    LAUNCHER,
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
    LAUNCHER,
    f"{SURVEY}/make-plan.py",
]

BUILD_EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/gf2-core/Cargo.toml",
    "crates/gf2-kernels-simd/Cargo.toml",
    "scripts/cargo-budget.sh",
    f"{TOOL}/Cargo.toml",
    f"{ARM}/Cargo.lock",
    f"{ARM}/Cargo.toml",
    f"{SURVEY}/make-addendum.py",
    f"{SURVEY}/make-producing-inputs.py",
    f"{SURVEY}/make-tables.py",
    f"{SURVEY}/runner-smoke.txt",
    f"{SURVEY}/smoke-arms.sh",
    f"{REUSED}/arm-common/Cargo.toml",
    f"{REUSED}/gf2-side/Cargo.lock",
    f"{REUSED}/gf2-side/Cargo.toml",
]

EVIDENCE_FILES = [
    "lane-equivalence.txt",
    "shipped-conformance.txt",
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
        root, SURVEY, "producing-inputs.json")
    behavior = set(BEHAVIOR_EXTRA)
    for directory in ("crates/gf2-core/src", "crates/gf2-kernels-simd/src", f"{ARM}/src",
                      f"{REUSED}/arm-common/src", f"{REUSED}/gf2-side/src", f"{TOOL}/src"):
        behavior.update(rust_sources(root, directory))
    build = behavior | set(BUILD_EXTRA) | {f"{EVIDENCE}/{name}" for name in EVIDENCE_FILES}
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
