#!/usr/bin/env python3
"""Write the producing manifest of the decoder calibration and its digests (jit:0a77ecfc).

Usage: make-calibration-producing-inputs.py [--check]
"""

import argparse
import hashlib
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # a check writes nothing, however invoked

from locate import CALIBRATION, HERE, ROOT, SHARED, package, producing_closure

GENERATOR = f"{CALIBRATION}/make-calibration-producing-inputs.py"
MANIFEST = HERE / "producing-build-inputs.json"
DIGESTS = HERE / "producing-build-inputs.sha256"
DECLARATION = f"{CALIBRATION}/campaign-declaration.json"
QUEUE_LAUNCHER = f"{CALIBRATION}/run-campaign.sh"
TOOL = package("tuning-campaign-support")
COMPOSER = package("tuning-profile-compose")
CODING = package("gf2-coding")
CRATES = [CODING, *(package(name) for name in ("gf2-core", "gf2-algebra", "gf2-kernels-simd"))]
LAUNCHER = f"{SHARED}/tuning-extent-campaign.sh"
LOCK_WRAPPER = f"{SHARED}/ccx1-bench-flock.sh"
INCLUDED = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"\s*\)')

LIFECYCLE = [
    DECLARATION,
    QUEUE_LAUNCHER,
    LAUNCHER,
    LOCK_WRAPPER,
    f"{TOOL}/src/bin/tuning-extent-campaign-driver.rs",
    f"{TOOL}/src/campaign.rs",
    f"{TOOL}/src/host.rs",
    f"{TOOL}/src/journal.rs",
    f"{TOOL}/src/process.rs",
    f"{TOOL}/src/provenance.rs",
    f"{TOOL}/src/transport.rs",
]

BEHAVIOR_EXTRA = [
    *LIFECYCLE,
    f"{CODING}/benches/tuning_calibration.rs",
    f"{SHARED}/validate-tuning-extent-campaign.py",
]

BUILD_EXTRA = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    *(f"{crate}/Cargo.toml" for crate in CRATES),
    f"{TOOL}/Cargo.toml",
    f"{COMPOSER}/Cargo.lock",
    f"{COMPOSER}/Cargo.toml",
    "scripts/cargo-budget.sh",
]


def included(sources):
    """Root-relative files the Rust `sources` include at compile time."""
    found = set()
    for source in sources:
        for relative in INCLUDED.findall((ROOT / source).read_text()):
            found.add(str((ROOT / source).parent.joinpath(relative).resolve().relative_to(ROOT)))
    return found


def digest_lines(paths):
    return "".join(
        f"{hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}  {path}\n" for path in paths)


def check_digests(paths):
    """Exits non-zero, naming each moved row, when the digest record differs from this tree."""
    relative = str(DIGESTS.relative_to(ROOT))
    recorded = DIGESTS.read_text().splitlines() if DIGESTS.is_file() else []
    expected = digest_lines(paths).splitlines()
    if recorded != expected:
        moved = sorted({line.split("  ", 1)[-1] for line in set(recorded) ^ set(expected)})
        raise SystemExit("\n".join(
            [f"{relative} does not pin this tree; regenerate it with {GENERATOR}", *moved]))
    print(f"{relative}: pins this tree")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="compare the committed records instead of writing them")
    arguments = parser.parse_args()
    behavior = set(BEHAVIOR_EXTRA)
    for directory in (*(f"{crate}/src" for crate in CRATES), f"{TOOL}/src", f"{COMPOSER}/src"):
        behavior.update(producing_closure.rust_sources(ROOT, directory))
    behavior |= included(path for path in behavior if path.endswith(".rs"))
    build = behavior | set(BUILD_EXTRA)
    missing = [path for path in sorted(build) if not (ROOT / path).is_file()]
    if missing:
        raise SystemExit(f"producing inputs missing from the tree: {missing}")
    closure = producing_closure.document(behavior, LIFECYCLE, build)
    producing_closure.emit(MANIFEST, closure, arguments.check, GENERATOR)
    pinned = sorted({*closure["build_inputs"], str(MANIFEST.relative_to(ROOT))})
    if arguments.check:
        check_digests(pinned)
    else:
        DIGESTS.write_text(digest_lines(pinned))
        print(f"wrote {DIGESTS.relative_to(ROOT)}: {len(pinned)} digests")


if __name__ == "__main__":
    main()
