#!/usr/bin/env python3
"""Write the 1c602857 producing-input manifest.

Usage: make-producing-inputs.py [output]   (default producing-inputs.json)

The manifest selects every repository file whose bytes can change what the
public wide carry-less product receipt measures or how it runs: the gf2
production crates the arm executable compiles, the arm crate itself, the
launcher, the shared lock wrapper and the shared campaign tooling (behavior);
the subset that decides campaign lifecycle and resume (lifecycle); and the
behavior set plus manifests, lock files, build configuration and the
correctness validation report (build inputs). The file lists are derived from
the tree, so an added source file enters the closure without an edit here.
"""

import json
import os
import subprocess
import sys

ISSUE = "dev/active/1c602857"
TOOL = "dev/tools/tuning-campaign-support"
LAUNCHER = f"{ISSUE}/run-public-clmul.sh"
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
    f"{ISSUE}/arms/Cargo.lock",
    f"{ISSUE}/arms/Cargo.toml",
    f"{ISSUE}/make-producing-inputs.py",
    f"{ISSUE}/run-validation.sh",
    f"{ISSUE}/validation.json",
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
        ["git", "rev-parse", "--show-toplevel"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    output = (
        sys.argv[1]
        if len(sys.argv) > 1
        else os.path.join(root, ISSUE, "producing-inputs.json")
    )
    behavior = set()
    for directory in (
        "crates/gf2-core/src",
        "crates/gf2-kernels-simd/src",
        f"{ISSUE}/arms/src",
        f"{TOOL}/src",
    ):
        behavior.update(rust_sources(root, directory))
    behavior.update([LAUNCHER, LOCK_WRAPPER])
    missing = [
        path
        for path in sorted(behavior | set(EXTRA_BUILD))
        if not os.path.isfile(os.path.join(root, path))
    ]
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
    print(
        f"{len(manifest['behavior_sources'])} behavior, "
        f"{len(manifest['lifecycle_sources'])} lifecycle, "
        f"{len(manifest['build_inputs'])} build inputs -> {output}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
