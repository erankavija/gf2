#!/usr/bin/env python3
"""Write the producing-input manifest of this family's campaigns (jit:ad2a6a58).

Usage: make-producing-inputs.py [output]
       (default producing-inputs.json beside this file)

The manifest is written by the shared `campaign_inputs.py`; this file declares
which files are this family's closure: the gf2 production crates the arm
compiles, the harness crates (this family's arm and the two byte-field survey
crates it reuses), the shared campaign generators, the launcher, the shared lock
wrapper and the shared campaign tooling.

Every location is resolved under the repository root git reports: this family's
own files from this file's directory, shared scripts and the launcher by file
name, harness and tool crates by package name.
"""

import os
import subprocess
import sys
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = subprocess.run(
    ["git", "-C", HERE, "rev-parse", "--show-toplevel"], check=True, capture_output=True,
    text=True,
).stdout.strip()


def shared_scripts():
    """Root-relative directory of the shared campaign scripts.

    Receipt input snapshots hold byte copies of it under an `inputs` directory;
    the live one is the path outside them.
    """
    listing = subprocess.run(
        ["git", "-C", ROOT, "ls-files", "--", ":(glob)**/campaign_inputs.py"], check=True,
        capture_output=True, text=True,
    ).stdout.split()
    live = [path for path in listing if "inputs" not in path.split("/")[:-1]]
    if len(live) != 1:
        raise SystemExit(f"{len(live)} live campaign_inputs.py files; exactly one must exist")
    return os.path.dirname(live[0])


SHARED = shared_scripts()
sys.path.insert(0, os.path.join(ROOT, SHARED))
import campaign_inputs  # noqa: E402
import repository_files  # noqa: E402


def package(name):
    return repository_files.package_directory(Path(ROOT), name)


def live_file(name):
    """Root-relative path of the one live file called `name`."""
    found = repository_files.tracked_files(Path(ROOT), name)
    if len(found) != 1:
        raise SystemExit(f"{len(found)} live files are called {name}; exactly one must be")
    return found[0]


SURVEY = os.path.relpath(HERE, ROOT)
ISSUE = os.path.dirname(SURVEY)
ARM = package("gf256-axpy-arm")
ARM_COMMON = package("byte-field-arm-common")
GF2_SIDE = package("byte-field-gf2-side")
TOOL = package("tuning-campaign-support")
LOCK_WRAPPER = f"{SHARED}/ccx1-bench-flock.sh"
LAUNCHER = live_file("run-axpy-confirmation.sh")
EVIDENCE = f"{ISSUE}/conformance"

LIFECYCLE = [
    LOCK_WRAPPER,
    LAUNCHER,
    f"{SHARED}/verify-campaign-log.py",
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
    f"{SHARED}/campaign_plan.py",
    f"{SHARED}/verify-campaign-log.py",
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
    f"{SHARED}/campaign_inputs.py",
    f"{SHARED}/campaign_tables.py",
    f"{SHARED}/pin-prior-receipt.py",
    f"{SHARED}/smoke-campaign-arms.sh",
    f"{SURVEY}/make-addendum.py",
    f"{SURVEY}/make-producing-inputs.py",
    f"{SURVEY}/make-tables.py",
    f"{SURVEY}/pilot-smoke.json",
    f"{SURVEY}/confirmation-smoke.json",
    f"{SURVEY}/smoke-arms.sh",
    f"{ARM_COMMON}/Cargo.toml",
    f"{GF2_SIDE}/Cargo.lock",
    f"{GF2_SIDE}/Cargo.toml",
    f"{EVIDENCE}/lane-equivalence.txt",
    f"{EVIDENCE}/shipped-conformance.txt",
]

SOURCE_DIRS = [
    "crates/gf2-core/src",
    "crates/gf2-kernels-simd/src",
    f"{ARM}/src",
    f"{ARM_COMMON}/src",
    f"{GF2_SIDE}/src",
    f"{TOOL}/src",
]


def main():
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "producing-inputs.json")
    campaign_inputs.write_manifest(ROOT, output, SOURCE_DIRS, BEHAVIOR_EXTRA, LIFECYCLE,
                                   BUILD_EXTRA)


if __name__ == "__main__":
    main()
