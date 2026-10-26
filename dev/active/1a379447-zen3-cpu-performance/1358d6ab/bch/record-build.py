#!/usr/bin/env python3
"""Build both arms of the BCH integration family and record their identity (jit:1358d6ab).

Usage (from the worktree root): record-build.py AFF3CT_ROOT BCHLIB_ROOT

Builds `consumer-arm` and `bch-external-arm` with Rust 1.95 into one release
target directory, runs the external arm package's tests and its verification,
and writes `build-identity.json` and `external-equivalence.json` beside this
script. The record pins each executable by digest, the commands and toolchain
that built it and the external trees it compiled; `run-family-campaign.py`
launches only executables a record pins. The equivalence record holds every
external engine's codewords to gf2's on every code class and W1 batch size.
"""

import hashlib
import json
import os
import pathlib
import subprocess
import sys

from locate import HERE, ISSUE, ROOT, package

TOOLCHAIN = "1.95"
TARGET = f"target/{ISSUE}-arm"
ARMS = {"consumer-arm": "consumer-profile-gf2-side", "bch-external-arm": "bch-external-arms"}
VERIFY = "bch-external-verify"


def output(*command, **options):
    return subprocess.run(command, check=True, capture_output=True, text=True,
                          **options).stdout.strip()


def sha256(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    aff3ct, bchlib = (pathlib.Path(argument).resolve() for argument in sys.argv[1:])
    environment = {"CARGO_CI_NO_SCCACHE": "1", "RUSTUP_TOOLCHAIN": TOOLCHAIN,
                   "CARGO_TARGET_DIR": str(ROOT / TARGET), "GF2_AFF3CT_ROOT": str(aff3ct),
                   "GF2_BCHLIB_ROOT": str(bchlib)}
    commands = []
    for name in dict.fromkeys(ARMS.values()):
        manifest = f"{package(name)}/Cargo.toml"
        for argv in (["./scripts/cargo-budget.sh", "--test", "cargo", "test", "--offline",
                      "--release", "--locked", "--manifest-path", manifest],
                     ["./scripts/cargo-budget.sh", "cargo", "build", "--offline", "--release",
                      "--locked", "--manifest-path", manifest]):
            subprocess.run(argv, check=True, env={**os.environ, **environment})
            commands.append({"environment": environment, "argv": argv})
    release = ROOT / TARGET / "release"
    equivalence = subprocess.run([release / VERIFY], capture_output=True, text=True)
    (HERE / "external-equivalence.json").write_text(equivalence.stdout)
    if equivalence.returncode != 0:
        sys.exit("an external engine writes other codewords than gf2; see external-equivalence.json")
    shell = {**os.environ, "RUSTUP_TOOLCHAIN": TOOLCHAIN}
    record = {
        "build_root": str(ROOT),
        "rustc": output("rustc", "-vV", env=shell),
        "cargo": output("cargo", "--version", env=shell),
        "cxx": output("c++", "--version").splitlines()[0],
        "cc": output("cc", "--version").splitlines()[0],
        "build_commands": commands,
        "external_sources": {
            "aff3ct": {"root": str(aff3ct), "revision": output("git", "-C", str(aff3ct), "rev-parse", "HEAD"),
                       "library_sha256": sha256(aff3ct / "build/lib/libaff3ct-4.7.0.a")},
            "bchlib": {"root": str(bchlib), "revision": output("git", "-C", str(bchlib), "rev-parse", "HEAD"),
                       "source_sha256": sha256(bchlib / "src/bch.c")},
        },
        "arms": {name: {"package": package_name, "path": f"{TARGET}/release/{name}",
                        "executable_sha256": sha256(release / name)}
                 for name, package_name in ARMS.items()},
    }
    (HERE / "build-identity.json").write_text(json.dumps(record, indent=2) + "\n")
    print(HERE / "build-identity.json")


if __name__ == "__main__":
    main()
