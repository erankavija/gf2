#!/usr/bin/env python3
"""Run the witness calls of the caller-trusted entry points (jit:bfc2ceab).

Builds the `witness` example of the `check-prototype` crate in release mode
under Rust 1.95, runs it once per entry it covers, each in its own process,
and writes `witness-runs.json` beside itself: how each process ends. A witness
passes arguments that break the entry point's unchecked condition, so its
behaviour is undefined; the record states what one build on one host does.

Usage: run-witnesses.py, with no arguments
"""

import json
import os
import re
import signal
import subprocess

import review
from locate import HERE, ISSUE, ROOT

CRATE = HERE / "check-prototype"
TOOLCHAIN = "1.95.0"
CONTROL = "control"
ARM = re.compile(r'^        "([a-z0-9-]+)" => \{$', re.M)


def outcome(status):
    if status < 0:
        return f"killed by {signal.Signals(-status).name}"
    return f"exit status {status}"


def main():
    environment = os.environ | {"RUSTUP_TOOLCHAIN": TOOLCHAIN}
    subprocess.run(
        ["cargo", "build", "--release", "--example", "witness"],
        cwd=CRATE, check=True, env=environment, capture_output=True,
    )
    covered = ARM.findall((CRATE / "examples" / "witness.rs").read_text())
    entries = {entry[0] for interface in review.INTERFACES for entry in interface["entries"]}
    unknown = set(covered) - entries - {CONTROL}
    if unknown:
        raise SystemExit(f"witnesses name no inventory entry: {sorted(unknown)}")
    runs = []
    for entry in covered:
        done = subprocess.run(
            [str(CRATE / "target" / "release" / "examples" / "witness"), entry],
            capture_output=True, text=True,
        )
        runs.append(
            {
                "entry": entry,
                "outcome": outcome(done.returncode),
                "returned": done.stdout.strip() == f"{entry}: returned",
            }
        )
    rustc = subprocess.run(
        ["rustc", "--version"], env=environment, capture_output=True, check=True, text=True
    ).stdout.strip()
    record = {
        "schema": "caller-trusted-witness-runs-v1",
        "issue": ISSUE,
        "toolchain": rustc,
        "profile": "release",
        "entries_without_a_witness": sorted(entries - set(covered)),
        "runs": runs,
    }
    output = HERE / "witness-runs.json"
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"{output.relative_to(ROOT)}: " + "; ".join(
        f"{run['entry']} {run['outcome']}" for run in runs))


if __name__ == "__main__":
    main()
