#!/usr/bin/env python3
"""Declare the content closure of the residual-route A/B family."""

import os
import subprocess
import sys

ROOT = subprocess.run(
    ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True
).stdout.strip()
sys.path.insert(0, os.path.join(ROOT, "dev/scripts"))
import campaign_inputs

ACTIVE = "dev/active/00dd43c3"
SURVEY = f"{ACTIVE}/survey"
TOOL = "dev/tools/tuning-campaign-support"
LAUNCHER = f"{SURVEY}/run-campaign.sh"
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


def main():
    output = sys.argv[1] if len(sys.argv) > 1 else f"{SURVEY}/producing-inputs.json"
    campaign_inputs.write_manifest(
        ROOT,
        output,
        source_dirs=("crates/gf2-core/src", "crates/gf2-kernels-simd/src", f"{TOOL}/src"),
        behavior_extra=("crates/gf2-core/benches/shifts.rs", LAUNCHER, LOCK, f"{SURVEY}/make-plan.py"),
        lifecycle=LIFECYCLE,
        build_extra=(
            ".cargo/config.toml",
            "Cargo.lock",
            "Cargo.toml",
            "crates/gf2-core/Cargo.toml",
            "crates/gf2-kernels-simd/Cargo.toml",
            f"{TOOL}/Cargo.toml",
            f"{ACTIVE}/pilot-addendum.json",
            f"{ACTIVE}/profile-context.json",
            f"{SURVEY}/freeze-pilot.py",
            f"{SURVEY}/freeze-confirmation.py",
            f"{SURVEY}/make-producing-inputs.py",
            f"{SURVEY}/verify-smoke.py",
            "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/find-shift-executable.py",
            "dev/scripts/campaign_inputs.py",
            "dev/scripts/verify-campaign-log.py",
            "dev/scripts/check-campaign-producing-closure.py",
            "dev/bench_results/c04dd4ac/residual-shift-profile/receipt.json",
            "dev/bench_results/c04dd4ac/residual-shift-profile/acceptance-summary.json",
            "scripts/cargo-budget.sh",
        ),
    )


if __name__ == "__main__":
    main()
