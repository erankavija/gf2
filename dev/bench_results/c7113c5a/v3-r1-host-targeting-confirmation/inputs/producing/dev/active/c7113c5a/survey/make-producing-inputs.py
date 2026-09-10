#!/usr/bin/env python3
"""Write the c7113c5a producing-input manifest.

Usage: make-producing-inputs.py [output]   (default survey/producing-inputs.json)

The manifest selects every repository file whose bytes can change what the
survey's campaigns measure or how they run: the gf2 production crates the arm
executables compile, the survey arm crate, its build and plan scripts, the
launcher, the shared lock wrapper and the shared campaign tooling (behavior);
the subset that decides campaign lifecycle and resume (lifecycle); and the
behavior set plus manifests, lock files, build configuration, the validation
report and the gf2x build evidence (build inputs). The file lists are derived
from the tree, so an added source file enters the closure without an edit here.
"""

import json
import os
import subprocess
import sys

SURVEY = "dev/active/c7113c5a/survey"
TOOL = "dev/tools/tuning-campaign-support"
LAUNCHER = "dev/bench_results/c7113c5a/run-polynomial-v3.sh"
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

EXTRA_BUILD = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/gf2-core/Cargo.toml",
    "crates/gf2-kernels-simd/Cargo.toml",
    f"{SURVEY}/gf2-side/Cargo.lock",
    f"{SURVEY}/gf2-side/Cargo.toml",
    f"{SURVEY}/gf2x-build-evidence-v3.txt",
    f"{SURVEY}/make-producing-inputs.py",
    f"{SURVEY}/record-build-evidence.sh",
    f"{SURVEY}/run-validation.sh",
    f"{SURVEY}/validation-v3.json",
    f"{TOOL}/Cargo.toml",
    "scripts/cargo-budget.sh",
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
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, SURVEY, "producing-inputs.json")
    behavior = set()
    for directory in ("crates/gf2-core/src", "crates/gf2-kernels-simd/src",
                      f"{SURVEY}/gf2-side/src", f"{TOOL}/src"):
        behavior.update(rust_sources(root, directory))
    behavior.update([
        f"{SURVEY}/fetch-build.sh",
        f"{SURVEY}/gf2-side/build.rs",
        f"{SURVEY}/make-plan.py",
        LAUNCHER,
        LOCK_WRAPPER,
    ])
    missing = [path for path in sorted(behavior | set(EXTRA_BUILD))
               if not os.path.isfile(os.path.join(root, path))]
    if missing:
        raise SystemExit(f"producing inputs missing from the tree: {missing}")
    manifest = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(LIFECYCLE),
        "build_inputs": sorted(behavior | set(EXTRA_BUILD)),
    }
    with open(output, "w") as handle:
        json.dump(manifest, handle, indent=2)
        handle.write("\n")
    print(f"{len(manifest['behavior_sources'])} behavior, {len(manifest['lifecycle_sources'])} lifecycle, "
          f"{len(manifest['build_inputs'])} build inputs -> {output}", file=sys.stderr)


if __name__ == "__main__":
    main()
