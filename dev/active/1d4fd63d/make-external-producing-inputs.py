#!/usr/bin/env python3
"""Write the producing-input manifest of the 1d4fd63d comparator family.

Usage: make-external-producing-inputs.py [--check] [output]
       (default external-producing-inputs.json beside this script)

`--check` writes nothing and exits non-zero when the closure at `output` differs
from the tree.

The comparator family measures gf2 lanes against M4RI and Bitshuffle, so its
closure differs from the lane-selection family's: it carries the kernel crate
the gf2-side arm compiles rather than the whole production stack, and it adds
6fb89a3c's committed C arm sources, its build script and its build evidence,
because those bytes decide what the external executables are. The file lists
are derived from the tree, so an added source file enters the closure without
an edit here.
"""

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
)


def _scripts():
    """Directory of the one live `producing_closure.py`, outside receipt snapshots."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-z", "--cached", "--others", "--exclude-standard",
         "--", ":(glob)**/producing_closure.py"],
        check=True, capture_output=True, text=True,
    ).stdout.split("\0")
    live = [path for path in listing if path and "inputs" not in Path(path).parts[:-1]]
    if len(live) != 1:
        raise SystemExit(f"{len(live)} live producing_closure.py files; exactly one must exist")
    return ROOT / Path(live[0]).parent


sys.dont_write_bytecode = True  # `--check` writes nothing, however invoked
sys.path.insert(0, str(_scripts()))
import producing_closure  # noqa: E402

files = producing_closure.repository_files


def located(name):
    return files.live_file(ROOT, name)


ISSUE = HERE.relative_to(ROOT).as_posix()
OUTPUT = f"{ISSUE}/external-producing-inputs.json"
SURVEY = Path(located("bitshuffle_transpose_arm.c")).parent.as_posix()
TOOL = files.package_directory(ROOT, "tuning-campaign-support")
LAUNCHER = located("run-transpose-lane.sh")
LOCK_WRAPPER = located("ccx1-bench-flock.sh")
SHARED = [located(name) for name in ("producing_closure.py", "repository_files.py")]

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

EXTRA_BUILD = EXTERNAL + SHARED + [
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
    located("cargo-budget.sh"),
]

SOURCE_DIRECTORIES = (
    "crates/gf2-kernels-simd/src",
    f"{ISSUE}/external-arms/src",
    f"{TOOL}/src",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", nargs="?", type=Path, default=ROOT / OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the closure at output with this tree's instead of writing it",
    )
    arguments = parser.parse_args()
    behavior = set()
    for directory in SOURCE_DIRECTORIES:
        behavior.update(producing_closure.rust_sources(ROOT, directory))
    behavior.update([LAUNCHER, LOCK_WRAPPER])
    behavior.update(EXTERNAL)
    missing = [
        path for path in sorted(behavior | set(EXTRA_BUILD)) if not (ROOT / path).is_file()
    ]
    if missing:
        raise SystemExit(f"producing inputs missing from the tree: {missing}")
    producing_closure.emit(
        arguments.output,
        producing_closure.document(behavior, LIFECYCLE, behavior | set(EXTRA_BUILD)),
        arguments.check,
        f"{ISSUE}/make-external-producing-inputs.py",
    )


if __name__ == "__main__":
    main()
