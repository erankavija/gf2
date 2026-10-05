#!/usr/bin/env python3
"""Write the producing-input manifest of the entry-check cost campaigns (jit:bfc2ceab).

Usage: make-producing-inputs.py, with no arguments

Writes `producing-inputs.json` beside itself through the shared
`campaign_inputs.py`. This file declares the family's closure: the gf2
production crates the arm compiles, the arm crate, the campaign tooling, the
family's generators, its launcher and the shared lock wrapper.
"""

import sys

from locate import HERE, ROOT, package, repository_files

import campaign_inputs  # noqa: E402

SHARED = str(repository_files.live_file(ROOT, "campaign_inputs.py")).rsplit("/", 1)[0]
SURVEY = str(HERE.relative_to(ROOT))
ARM = package("entry-check-arm")
TOOL = package("tuning-campaign-support")
CORE, SIMD, ALGEBRA = package("gf2-core"), package("gf2-kernels-simd"), package("gf2-algebra")
LAUNCHER = repository_files.live_file(ROOT, "run-entry-check.sh")
LOCK_WRAPPER = f"{SHARED}/ccx1-bench-flock.sh"

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
    f"{SHARED}/verify-campaign-log.py",
    f"{SHARED}/repository_files.py",
    f"{SURVEY}/locate.py",
    f"{SURVEY}/make-plan.py",
]

BUILD_EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    f"{CORE}/Cargo.toml",
    f"{SIMD}/Cargo.toml",
    f"{ALGEBRA}/Cargo.toml",
    f"{TOOL}/Cargo.toml",
    "scripts/cargo-budget.sh",
    f"{ARM}/Cargo.lock",
    f"{ARM}/Cargo.toml",
    f"{ARM}/tests/workloads.rs",
    f"{SHARED}/campaign_inputs.py",
    f"{SHARED}/smoke-campaign-arms.sh",
    f"{SURVEY}/make-addendum.py",
    f"{SURVEY}/make-producing-inputs.py",
    f"{SURVEY}/smoke-arms.sh",
    f"{SURVEY}/pilot-smoke.json",
]

SOURCE_DIRS = [f"{CORE}/src", f"{SIMD}/src", f"{ALGEBRA}/src", f"{ARM}/src", f"{TOOL}/src"]


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    campaign_inputs.write_manifest(
        str(ROOT), str(HERE / "producing-inputs.json"), SOURCE_DIRS, BEHAVIOR_EXTRA, LIFECYCLE,
        BUILD_EXTRA,
    )


if __name__ == "__main__":
    main()
