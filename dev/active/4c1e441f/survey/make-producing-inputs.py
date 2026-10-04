#!/usr/bin/env python3
"""Write the producing-input manifest of this family's campaigns (jit:4c1e441f).

Usage: make-producing-inputs.py [output]
       (default producing-inputs.json beside this file)

The manifest is written by the shared `campaign_inputs.py`; this file declares
which files are this family's closure: the gf2 production crates the arm
compiles, the harness crates (this family's arm, the vector family's lane
library and the two byte-field survey crates it reuses), the shared campaign
generators, the launcher, the shared lock wrapper and the shared campaign
tooling.
"""

import os
import sys

import locate
import campaign_inputs  # noqa: E402

ROOT = locate.ROOT
SHARED = locate.SHARED
SURVEY = locate.SURVEY
ISSUE = locate.ISSUE
ARM = locate.package("gf256-gemm-arm")
LANE = locate.package("gf256-axpy-arm")
ARM_COMMON = locate.package("byte-field-arm-common")
GF2_SIDE = locate.package("byte-field-gf2-side")
TOOL = locate.package("tuning-campaign-support")
LOCK_WRAPPER = f"{SHARED}/ccx1-bench-flock.sh"
LAUNCHER = locate.LAUNCHER
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
    f"{SHARED}/repository_files.py",
    f"{SHARED}/verify-campaign-log.py",
    f"{SURVEY}/locate.py",
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
    f"{LANE}/Cargo.lock",
    f"{LANE}/Cargo.toml",
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
    f"{ARM}/tests/request_mirror.rs",
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
    f"{LANE}/src",
    f"{ARM_COMMON}/src",
    f"{GF2_SIDE}/src",
    f"{TOOL}/src",
]


def main():
    root = ROOT
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        root, SURVEY, "producing-inputs.json")
    campaign_inputs.write_manifest(root, output, SOURCE_DIRS, BEHAVIOR_EXTRA, LIFECYCLE,
                                   BUILD_EXTRA)


if __name__ == "__main__":
    main()
