#!/usr/bin/env python3
"""Record the untimed reports of the coding tuning producer (jit:0a77ecfc).

Builds the producer as the campaign launcher builds it, under Rust 1.95 in
release mode, runs its capability report, grid listing and self-check, and
writes each response under `producer/` in the form a published session record
holds, beside `producer/identity.json` with the digests of the producer source
and executable. No mode installs a profile or runs a cell.

Usage: record-producer-reports.py, with no arguments
"""

import hashlib
import json
import os
import subprocess

from locate import HERE, ISSUE, ROOT, package, sha256

TOOLCHAIN = "1.95.0"
FEATURES = "parallel,simd,tuning-profile,test-support"
MODES = ("capability-report", "list-grid", "self-check")
RESULT_PREFIX = "GF2_TUNING_RESULT="
ENVIRONMENT = {"RUSTUP_TOOLCHAIN": TOOLCHAIN, "RAYON_NUM_THREADS": "4"}


def main():
    producer = f"{package('gf2-coding')}/benches/tuning_calibration.rs"
    environment = {**os.environ, **ENVIRONMENT}
    build = subprocess.run(
        ["nice", "-n", "19", "./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build",
         "--release", "-p", "gf2-coding", "--bench", "tuning_calibration",
         "--features", FEATURES, "--message-format=json"],
        cwd=ROOT, capture_output=True, check=True, text=True, env=environment,
    ).stdout
    executables = {
        message["executable"] for message in map(json.loads, build.splitlines())
        if message.get("reason") == "compiler-artifact" and message.get("executable")
        and message["target"]["name"] == "tuning_calibration"
    }
    if len(executables) != 1:
        raise SystemExit(f"{len(executables)} producer executables built")
    executable = executables.pop()
    reports = {}
    for mode in MODES:
        output = subprocess.run(
            [executable, f"--{mode}"], cwd=ROOT, capture_output=True, check=True, text=True,
            stdin=subprocess.DEVNULL, env=environment,
        ).stdout
        (line,) = output.splitlines()
        response = line.removeprefix(RESULT_PREFIX)
        if response == line or json.loads(response)["operation"] != mode:
            raise SystemExit(f"--{mode} did not answer with its own report")
        name = f"coding-{mode}.json"
        (HERE / "producer" / name).write_text(response)
        reports[name] = hashlib.sha256(response.encode()).hexdigest()
    identity = {
        "issue": ISSUE,
        "producer": producer,
        "features": FEATURES,
        "toolchain": subprocess.run(
            ["rustc", f"+{TOOLCHAIN}", "--version"], capture_output=True, check=True, text=True,
        ).stdout.strip(),
        "source_sha256": sha256(producer),
        "binary_sha256": hashlib.sha256(open(executable, "rb").read()).hexdigest(),
        "reports_sha256": reports,
    }
    (HERE / "producer" / "identity.json").write_text(json.dumps(identity, indent=1) + "\n")
    print(json.dumps(identity, indent=1))


if __name__ == "__main__":
    main()
