#!/usr/bin/env python3
"""Build and record the integrated core arms with Rust 1.95 release.

Usage: build-core.py [--check]

Check mode compares the recorded executable digests and build commands with
local files and the shared build table, without building or writing anything.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
from locate import HERE, ROOT, live  # noqa: E402


def document(build):
    commands, arms = [], {}
    record = json.loads((ROOT / live("63bad95d/survey/planned-families.json")).read_text())
    manifest = live("63bad95d/survey/profile-arms/Cargo.toml")
    for level, declaration in record["builds"].items():
        target = f"target/31727c57-core-{level}"
        flags = declaration["rustflags"]
        command = ["nice", "-n", "19", "./scripts/cargo-budget.sh", "cargo", "+1.95", "build",
                   "--offline", "--locked", "--release", "--manifest-path", manifest,
                   "--target-dir", target]
        environment = {**os.environ, "CARGO_HOME": str(ROOT / "target/31727c57-cargo-home"),
                       "CARGO_NET_OFFLINE": "true", "CARGO_CI_NO_SCCACHE": "1"}
        for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "CARGO_TARGET_DIR"):
            environment.pop(key, None)
        if flags:
            environment["RUSTFLAGS"] = flags
        if build:
            subprocess.run(command, check=True, env=environment)
        executable = f"{target}/release/profile-arm"
        arms[level] = {"path": executable,
                       "executable_sha256": hashlib.sha256((ROOT / executable).read_bytes()).hexdigest(),
                       "rustflags": flags}
        commands.append({"argv": command, "rustflags": flags})
    return {"arms": arms, "build_commands": commands,
            "rustc": subprocess.check_output(["rustc", "+1.95", "-Vv"], text=True).strip(),
            "cargo": subprocess.check_output(["./scripts/cargo-budget.sh", "cargo", "+1.95", "-V"], text=True).strip()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = document(not args.check)
    path = HERE / "core/arm-builds.json"
    text = json.dumps(result, indent=2) + "\n"
    if args.check:
        if path.read_text() != text:
            parser.error("arm bytes or build commands differ from the record")
        print("core build record matches")
    else:
        path.write_text(text)
        print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
