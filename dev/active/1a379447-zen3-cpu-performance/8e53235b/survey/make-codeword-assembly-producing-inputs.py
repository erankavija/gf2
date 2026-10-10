#!/usr/bin/env python3
"""Write the producing-input closure of the codeword-assembly campaigns (jit:8e53235b).

Usage: make-codeword-assembly-producing-inputs.py [--check]

Writes `producing-inputs.json` beside itself: the gf2 production crates the
consumer arm compiles, the arm crate, the campaign tooling, this family's
generators, its launcher and the shared lock wrapper. `--check` writes nothing
and exits non-zero when the committed closure differs from the tree.
"""

import argparse

from locate import FAMILY_DIRECTORY, HERE, LAUNCHER, ROOT, SHARED, SURVEY, package

import producing_closure  # noqa: E402

ARM = package("consumer-profile-gf2-side")
TOOL = package("tuning-campaign-support")
CRATES = [package(name) for name in ("gf2-core", "gf2-coding", "gf2-kernels-simd")]
LOCK_WRAPPER = f"{SHARED}/ccx1-bench-flock.sh"
GENERATOR = f"{SURVEY}/make-codeword-assembly-producing-inputs.py"

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
    f"{SHARED}/repository_files.py",
    f"{SURVEY}/locate.py",
    f"{SURVEY}/make-plan.py",
]

BUILD_EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    *(f"{crate}/Cargo.toml" for crate in CRATES),
    f"{TOOL}/Cargo.toml",
    "scripts/cargo-budget.sh",
    f"{ARM}/Cargo.lock",
    f"{ARM}/Cargo.toml",
    f"{ARM}/tests/arm_paths.rs",
    f"{ARM}/tests/declared_codes.rs",
    f"{SHARED}/producing_closure.py",
    f"{SHARED}/smoke-campaign-arms.sh",
    f"{SURVEY}/make-pilot-addendum.py",
    f"{SURVEY}/smoke-arms.sh",
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
    for directory in (*(f"{crate}/src" for crate in CRATES), f"{ARM}/src", f"{TOOL}/src"):
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
