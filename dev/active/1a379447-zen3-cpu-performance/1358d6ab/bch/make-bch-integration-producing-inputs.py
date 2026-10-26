#!/usr/bin/env python3
"""Write the producing-input closure of the BCH integration campaigns (jit:1358d6ab).

Usage: make-bch-integration-producing-inputs.py [--check]

Writes `producing-inputs.json` beside itself: the gf2 production crates both
arms compile, the consumer harness, the external arm crate, the campaign
tooling, this family's generators and launcher and the shared scripts they run.
`--check` writes nothing and exits non-zero when the committed closure differs
from the tree.
"""

import argparse

from locate import FAMILY_DIRECTORY, HERE, ROOT, SHARED, package, repository_files

import producing_closure  # noqa: E402

CONSUMER = package("consumer-profile-gf2-side")
EXTERNAL = package("bch-external-arms")
TOOL = package("tuning-campaign-support")
CRATES = [package(name) for name in ("gf2-core", "gf2-coding", "gf2-kernels-simd")]
LOCK_WRAPPER = f"{SHARED}/ccx1-bench-flock.sh"
LAUNCHER = f"{FAMILY_DIRECTORY}/window.sh"
GENERATOR = f"{FAMILY_DIRECTORY}/make-bch-integration-producing-inputs.py"
# The story campaign launcher the window script runs and the modules it loads.
CAMPAIGN = [repository_files.live_file(ROOT, name) for name in (
    "run-family-campaign.py", "record-arm-validation.py", "record-peak-memory.py",
    "record-intra-quality.py", "smoke-gf2-arms.py", "arm_build.py")]

LIFECYCLE = [
    LOCK_WRAPPER,
    LAUNCHER,
    *CAMPAIGN,
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
    *CAMPAIGN,
    f"{SHARED}/campaign_plan.py",
    f"{SHARED}/verify-campaign-log.py",
    f"{SHARED}/repository_files.py",
    f"{FAMILY_DIRECTORY}/locate.py",
    f"{FAMILY_DIRECTORY}/make-plan.py",
    f"{EXTERNAL}/build.rs",
    f"{EXTERNAL}/cpp/aff3ct_bch_shim.cpp",
]

BUILD_EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    *(f"{crate}/Cargo.toml" for crate in CRATES),
    f"{TOOL}/Cargo.toml",
    "scripts/cargo-budget.sh",
    f"{CONSUMER}/Cargo.lock",
    f"{CONSUMER}/Cargo.toml",
    f"{EXTERNAL}/Cargo.lock",
    f"{EXTERNAL}/Cargo.toml",
    f"{SHARED}/producing_closure.py",
    f"{FAMILY_DIRECTORY}/make-pilot-addendum.py",
    f"{FAMILY_DIRECTORY}/record-build.py",
    GENERATOR,
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the committed closure with this tree's instead of writing it",
    )
    arguments = parser.parse_args()
    behavior = set(BEHAVIOR_EXTRA)
    sources = (*(f"{crate}/src" for crate in CRATES), f"{CONSUMER}/src", f"{EXTERNAL}/src",
               f"{TOOL}/src")
    for directory in sources:
        behavior.update(producing_closure.rust_sources(ROOT, directory))
    build = behavior | set(BUILD_EXTRA)
    missing = [path for path in sorted(build) if not (ROOT / path).is_file()]
    if missing:
        raise SystemExit(f"producing inputs missing from the tree: {missing}")
    producing_closure.emit(
        HERE / "producing-inputs.json",
        producing_closure.document(behavior, LIFECYCLE, build),
        arguments.check,
        GENERATOR,
    )


if __name__ == "__main__":
    main()
