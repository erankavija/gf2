#!/usr/bin/env python3
"""Write the producing manifest of the decoder calibration campaign (jit:0a77ecfc).

Usage: make-calibration-producing-inputs.py [--check]

Writes `producing-build-inputs.json` beside itself and
`producing-build-inputs.sha256`, the SHA-256 of every build input the manifest
names. The behaviour sources are every Rust source of the packages the coding
producer and the composer compile, the files those sources include, the
producer, the composer, the campaign tooling and the campaign declaration.

`--check` writes nothing. It compares the manifest with this tree's
enumeration, and the digests with the tree of the commit that last changed
the digest record, so the record keeps naming the declared tree after later
changes to its sources. An uncommitted record is compared with the working
tree.
"""

import argparse
import hashlib
import re
import subprocess
from pathlib import Path

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


def committed(revision, path):
    return subprocess.run(
        ["git", "-C", str(ROOT), "show", f"{revision}:{path}"], capture_output=True, check=True,
    ).stdout


def digest_lines(paths, read):
    return "".join(f"{hashlib.sha256(read(path)).hexdigest()}  {path}\n" for path in paths)


def check_digests(paths):
    """Compares the digest record with the tree it was written from."""
    relative = str(DIGESTS.relative_to(ROOT))
    revision = subprocess.run(
        ["git", "-C", str(ROOT), "log", "-1", "--format=%H", "--", relative],
        capture_output=True, check=True, text=True,
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "-C", str(ROOT), "status", "--porcelain", "--", relative],
        capture_output=True, check=True, text=True,
    ).stdout
    if revision and not dirty:
        expected = digest_lines(paths, lambda path: committed(revision, path))
    else:
        expected = digest_lines(paths, lambda path: (ROOT / path).read_bytes())
    if not DIGESTS.is_file() or DIGESTS.read_text() != expected:
        raise SystemExit(f"{relative} differs from its declared tree; regenerate it with {GENERATOR}")
    print(f"{relative}: pins its declared tree")


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
        DIGESTS.write_text(digest_lines(pinned, lambda path: (ROOT / path).read_bytes()))
        print(f"wrote {DIGESTS.relative_to(ROOT)}: {len(pinned)} digests")


if __name__ == "__main__":
    main()
