#!/usr/bin/env python3
"""Generate the content closure for the residual-shift campaign.

Usage: make-shift-producing-inputs.py [--check] [output]
       (default shift-profile-producing-inputs.json beside this script)

`--check` writes nothing and exits non-zero when the closure at `output` differs
from the tree.
"""

import argparse
import subprocess
import sys
from pathlib import Path

SURVEY = Path(__file__).resolve().parent
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(SURVEY), "rev-parse", "--show-toplevel"],
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


sys.path.insert(0, str(_scripts()))
import producing_closure  # noqa: E402

files = producing_closure.repository_files


def located(name):
    return files.live_file(ROOT, name)


SURVEY_DIRECTORY = SURVEY.relative_to(ROOT).as_posix()
ACTIVE = SURVEY.parent.relative_to(ROOT).as_posix()
OUTPUT = f"{SURVEY_DIRECTORY}/shift-profile-producing-inputs.json"
TOOL = files.package_directory(ROOT, "tuning-campaign-support")
LAUNCHER = f"{SURVEY_DIRECTORY}/run-shift-profile.sh"
LOCK = located("ccx1-bench-flock.sh")
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
EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/gf2-core/Cargo.toml",
    "crates/gf2-kernels-simd/Cargo.toml",
    "crates/gf2-core/benches/shifts.rs",
    f"{ACTIVE}/shift-profile-addendum.json",
    f"{ACTIVE}/shift-profile-consumer-audit.json",
    f"{ACTIVE}/shift-profile-validation.json",
    f"{SURVEY_DIRECTORY}/find-shift-executable.py",
    f"{SURVEY_DIRECTORY}/freeze-shift-addendum.py",
    f"{SURVEY_DIRECTORY}/inspect-shift-consumers.py",
    f"{SURVEY_DIRECTORY}/make-shift-plan.py",
    f"{SURVEY_DIRECTORY}/make-shift-producing-inputs.py",
    f"{TOOL}/Cargo.toml",
    located("cargo-budget.sh"),
    located("producing_closure.py"),
    located("repository_files.py"),
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
    behavior = set()
    for directory in ("crates/gf2-core/src", "crates/gf2-kernels-simd/src", f"{TOOL}/src"):
        behavior.update(producing_closure.rust_sources(ROOT, directory))
    behavior.update(["crates/gf2-core/benches/shifts.rs", LAUNCHER, LOCK])
    all_inputs = behavior | set(EXTRA)
    missing = [path for path in sorted(all_inputs) if not (ROOT / path).is_file()]
    if missing:
        raise SystemExit(f"producing inputs missing from the tree: {missing}")
    producing_closure.emit(
        arguments.output,
        producing_closure.document(behavior, LIFECYCLE, all_inputs),
        arguments.check,
        f"{SURVEY_DIRECTORY}/make-shift-producing-inputs.py",
    )


if __name__ == "__main__":
    main()
