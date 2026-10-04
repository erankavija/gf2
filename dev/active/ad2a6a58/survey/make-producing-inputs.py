#!/usr/bin/env python3
"""Write the producing-input manifest of this family's campaigns (jit:ad2a6a58).

Usage: make-producing-inputs.py [output]
       (default dev/active/ad2a6a58/survey/producing-inputs.json)

The manifest is written by `dev/scripts/campaign_inputs.py`; this file declares
which files are this family's closure: the gf2 production crates the arm
compiles, the harness crates (this family's arm and the two byte-field survey
crates it reuses), the shared campaign generators, the launcher, the shared lock
wrapper and the shared campaign tooling.
"""

import os
import subprocess
import sys

sys.path.insert(0, os.path.join(subprocess.run(
    ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
).stdout.strip(), "dev/scripts"))
import campaign_inputs  # noqa: E402

ISSUE = "dev/active/ad2a6a58"
SURVEY = f"{ISSUE}/survey"
ARM = f"{SURVEY}/axpy-arm"
REUSED = "dev/active/6c6b09b1/survey"
TOOL = "dev/tools/tuning-campaign-support"
SHARED = "dev/scripts"
LOCK_WRAPPER = f"{SHARED}/ccx1-bench-flock.sh"
LAUNCHER = "dev/bench_results/ad2a6a58/run-axpy-confirmation.sh"
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
    f"{REUSED}/arm-common/Cargo.toml",
    f"{REUSED}/gf2-side/Cargo.lock",
    f"{REUSED}/gf2-side/Cargo.toml",
    f"{EVIDENCE}/lane-equivalence.txt",
    f"{EVIDENCE}/shipped-conformance.txt",
]

SOURCE_DIRS = [
    "crates/gf2-core/src",
    "crates/gf2-kernels-simd/src",
    f"{ARM}/src",
    f"{REUSED}/arm-common/src",
    f"{REUSED}/gf2-side/src",
    f"{TOOL}/src",
]


def main():
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        root, SURVEY, "producing-inputs.json")
    campaign_inputs.write_manifest(root, output, SOURCE_DIRS, BEHAVIOR_EXTRA, LIFECYCLE,
                                   BUILD_EXTRA)


if __name__ == "__main__":
    main()
