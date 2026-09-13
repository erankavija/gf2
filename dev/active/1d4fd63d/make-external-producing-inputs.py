#!/usr/bin/env python3
"""Write the producing-input manifest of the 1d4fd63d comparator family.

Usage: make-external-producing-inputs.py [output]
       (default external-producing-inputs.json)

The comparator family measures gf2 lanes against M4RI and Bitshuffle, so its
closure differs from the lane-selection family's: it carries the kernel crate
the gf2-side arm compiles rather than the whole production stack, and it adds
6fb89a3c's committed C arm sources, its build script and its build evidence,
because those bytes decide what the external executables are. The file lists
are derived from the tree, so an added source file enters the closure without
an edit here.
"""

import json
import os
import subprocess
import sys

ISSUE = "dev/active/1d4fd63d"
SURVEY = "dev/active/6fb89a3c/survey"
TOOL = "dev/tools/tuning-campaign-support"
LAUNCHER = "dev/bench_results/1d4fd63d/run-transpose-lane.sh"
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

# The external arms' sources and pins: their bytes decide what the executables
# are, and `check-external-arms.py` decides whether the executables present are
# the ones those bytes and 6fb89a3c's build evidence describe.
EXTERNAL = [
    f"{SURVEY}/build-evidence.json",
    f"{SURVEY}/bitshuffle_transpose_arm.c",
    f"{SURVEY}/fetch-build.sh",
    f"{SURVEY}/harness_common.h",
    f"{SURVEY}/json_min.c",
    f"{SURVEY}/json_min.h",
    f"{SURVEY}/m4ri_transpose_arm.c",
    f"{SURVEY}/Makefile",
]

EXTRA_BUILD = EXTERNAL + [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/gf2-kernels-simd/Cargo.toml",
    f"{ISSUE}/external-arms/Cargo.lock",
    f"{ISSUE}/external-arms/Cargo.toml",
    f"{ISSUE}/make-external-plan.py",
    f"{ISSUE}/make-external-producing-inputs.py",
    f"{ISSUE}/survey/bit-mapping-report.json",
    f"{ISSUE}/survey/check-external-arms.py",
    f"{ISSUE}/survey/external-arm-check.json",
    f"{ISSUE}/survey/verify-bit-mapping.py",
    f"{TOOL}/Cargo.toml",
    "scripts/cargo-budget.sh",
]

SOURCE_DIRECTORIES = (
    "crates/gf2-kernels-simd/src",
    f"{ISSUE}/external-arms/src",
    f"{TOOL}/src",
)


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
        else os.path.join(root, ISSUE, "external-producing-inputs.json")
    )
    behavior = set()
    for directory in SOURCE_DIRECTORIES:
        behavior.update(rust_sources(root, directory))
    behavior.update([LAUNCHER, LOCK_WRAPPER])
    behavior.update(EXTERNAL)
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
