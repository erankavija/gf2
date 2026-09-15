#!/usr/bin/env python3
"""Generate the content closure for the residual-shift campaign."""

import json
import os
import subprocess
import sys

ACTIVE = "dev/active/c04dd4ac-zen3-shifts-and-permutations"
SURVEY = f"{ACTIVE}/survey"
TOOL = "dev/tools/tuning-campaign-support"
LAUNCHER = f"{SURVEY}/run-shift-profile.sh"
LOCK = "dev/scripts/ccx1-bench-flock.sh"
LIFECYCLE = [
    LAUNCHER,
    LOCK,
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
EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/gf2-core/Cargo.toml",
    "crates/gf2-kernels-simd/Cargo.toml",
    "crates/gf2-core/benches/shifts.rs",
    f"{ACTIVE}/shift-profile-addendum.json",
    f"{ACTIVE}/shift-profile-consumer-audit.json",
    f"{ACTIVE}/shift-profile-validation.json",
    f"{SURVEY}/find-shift-executable.py",
    f"{SURVEY}/freeze-shift-addendum.py",
    f"{SURVEY}/inspect-shift-consumers.py",
    f"{SURVEY}/make-shift-plan.py",
    f"{SURVEY}/make-shift-producing-inputs.py",
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
    output = sys.argv[1] if len(sys.argv) > 1 else f"{SURVEY}/shift-profile-producing-inputs.json"
    behavior = set()
    for directory in (
        "crates/gf2-core/src",
        "crates/gf2-kernels-simd/src",
        f"{TOOL}/src",
    ):
        behavior.update(rust_sources(root, directory))
    behavior.update(["crates/gf2-core/benches/shifts.rs", LAUNCHER, LOCK])
    all_inputs = behavior | set(EXTRA)
    missing = [path for path in sorted(all_inputs) if not os.path.isfile(os.path.join(root, path))]
    if missing:
        raise SystemExit(f"producing inputs missing from the tree: {missing}")
    manifest = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(LIFECYCLE),
        "build_inputs": sorted(all_inputs),
    }
    with open(output, "w") as destination:
        json.dump(manifest, destination, indent=2)
        destination.write("\n")
    print(
        f"{len(manifest['behavior_sources'])} behavior, "
        f"{len(manifest['lifecycle_sources'])} lifecycle, "
        f"{len(manifest['build_inputs'])} build inputs -> {output}"
    )


if __name__ == "__main__":
    main()
