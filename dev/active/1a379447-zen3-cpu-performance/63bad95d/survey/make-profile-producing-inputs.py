#!/usr/bin/env python3
"""Generate the content closure of the profile-family campaigns (jit:63bad95d).

Usage: make-profile-producing-inputs.py [--check] [output]
       (default profile-producing-inputs.json beside this script)

`--check` writes nothing and exits non-zero when the closure at `output` differs
from the tree.
"""

import argparse
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # `--check` writes nothing, however invoked

from locate import HERE, ROOT, package, repository_files, tracked  # noqa: E402

sys.path.insert(
    0, str((ROOT / repository_files.live_file(ROOT, "producing_closure.py")).parent)
)
import producing_closure  # noqa: E402


def located(name):
    return repository_files.live_file(ROOT, name)


SURVEY = HERE.relative_to(ROOT).as_posix()
OUTPUT = f"{SURVEY}/profile-producing-inputs.json"
ARMS = package("profile-build-arms")
TOOL = package("tuning-campaign-support")
CORE = package("gf2-core")
KERNELS = package("gf2-kernels-simd")
LAUNCHER = located("run-profile-families.sh")
LOCK = located("ccx1-bench-flock.sh")
LOG_CHECKER = located("verify-campaign-log.py")
LIFECYCLE = [
    LAUNCHER,
    LOCK,
    LOG_CHECKER,
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", nargs="?", type=Path, default=ROOT / OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the closure at output with this tree's instead of writing it",
    )
    arguments = parser.parse_args()
    owners = tracked("**/gf2-core/**/tuning-profiles/gf2-63bad95d-*.json")
    behavior = set()
    for directory in (f"{CORE}/src", f"{KERNELS}/src", f"{TOOL}/src", f"{ARMS}/src"):
        behavior.update(producing_closure.rust_sources(ROOT, directory))
    behavior.update([
        LAUNCHER,
        LOCK,
        LOG_CHECKER,
        located("repository_files.py"),
        f"{SURVEY}/locate.py",
        f"{SURVEY}/make-family-plan.py",
        f"{SURVEY}/planned-families.json",
        *owners,
    ])
    extra = [
        ".cargo/config.toml",
        "Cargo.lock",
        "Cargo.toml",
        f"{CORE}/Cargo.toml",
        f"{KERNELS}/Cargo.toml",
        f"{TOOL}/Cargo.toml",
        f"{ARMS}/Cargo.toml",
        f"{ARMS}/Cargo.lock",
        f"{ARMS}/tests/contract.rs",
        f"{SURVEY}/make-family-addenda.py",
        f"{SURVEY}/make-profile-producing-inputs.py",
        located("cargo-budget.sh"),
        located("producing_closure.py"),
        located("repo_artifacts.py"),
    ]
    all_inputs = behavior | set(extra)
    missing = [path for path in sorted(all_inputs) if not (ROOT / path).is_file()]
    if missing:
        raise SystemExit(f"producing inputs missing from the tree: {missing}")
    producing_closure.emit(
        arguments.output,
        producing_closure.document(behavior, LIFECYCLE, all_inputs),
        arguments.check,
        f"{SURVEY}/make-profile-producing-inputs.py",
    )


if __name__ == "__main__":
    main()
