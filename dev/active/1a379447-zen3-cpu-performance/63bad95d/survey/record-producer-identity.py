#!/usr/bin/env python3
"""Record what the matvec sweep does to the offline tuning producer's identity (jit:63bad95d).

Builds the core producer from the working tree under Rust 1.95 in release mode
and writes `producer-identity.json` beside this script: the digests of the
producer source and executable beside those the earlier producer-identity
record holds for the tree before the sweep, the behaviour token the codec
names, and the committed pins the change must leave alone: the measured owner
the baked constants pin, and every committed core owner envelope by digest.
Untimed: it builds and hashes, and runs no cell.

Usage: record-producer-identity.py, with no arguments
"""

import hashlib
import json
import os
import re
import subprocess

from locate import HERE, ROOT, package, tracked

FEATURES = "parallel,simd,test-support,tuning-profile"
RECORD = "producer-identity.json"


def sha256(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def main():
    core = package("gf2-core")
    producer = f"{core}/benches/tuning_calibration.rs"
    build = subprocess.run(
        ["nice", "-n", "19", "./scripts/cargo-budget.sh", "cargo", "build", "--release",
         "-p", "gf2-core", "--bench", "tuning_calibration", "--features", FEATURES,
         "--message-format", "json"],
        cwd=ROOT, capture_output=True, check=True, text=True,
        env={**os.environ, "RUSTUP_TOOLCHAIN": "1.95.0"},
    ).stdout
    executables = [
        message["executable"] for message in map(json.loads, build.splitlines())
        if message.get("reason") == "compiler-artifact" and message.get("executable")
    ]
    if len(executables) != 1:
        raise SystemExit(f"{len(executables)} producer executables built")
    binary = hashlib.sha256(open(executables[0], "rb").read()).hexdigest()

    own = str((HERE / RECORD).relative_to(ROOT))
    earlier = [path for path in tracked(f"**/{RECORD}") if path != own]
    if len(earlier) != 1:
        raise SystemExit(f"{len(earlier)} earlier producer-identity records")
    before = json.loads((ROOT / earlier[0]).read_text())
    if before["producer"] != producer or before["features"] != FEATURES:
        raise SystemExit("the earlier record describes another producer build")

    token = re.search(r'CORE_HARNESS_SCHEMA: &str = "([^"]*)"',
                      (ROOT / core / "src/tuning/mod.rs").read_text()).group(1)
    baked = (ROOT / core / "src/tuning/baked.rs").read_text()
    owner = f"{core}/data/tuning-profiles/" + re.search(
        r"data/tuning-profiles/([a-z0-9-]+\.json)", baked).group(1)
    pinned = re.search(r"SHA-256 `([0-9a-f]{64})`", baked).group(1)
    owners = {path: sha256(path) for path in tracked(f"{core}/data/tuning-profiles/*.json")}
    after = {"source_sha256": sha256(producer), "binary_sha256": binary}
    record = {
        "issue": "63bad95d",
        "producer": producer,
        "features": FEATURES,
        "behavior_token": token,
        "behavior_token_equal": token == before["behavior_token"],
        "before": {"record": earlier[0], **before["working_tree"]},
        "working_tree": after,
        "source_equal": after["source_sha256"] == before["working_tree"]["source_sha256"],
        "binary_equal": after["binary_sha256"] == before["working_tree"]["binary_sha256"],
        "baked_owner_pin": {
            "owner": owner,
            "pinned_sha256": pinned,
            "owner_sha256": owners[owner],
            "pin_holds": pinned == owners[owner],
        },
        "committed_core_owners": owners,
    }
    (HERE / RECORD).write_text(json.dumps(record, indent=1) + "\n")
    print(json.dumps({key: record[key] for key in (
        "behavior_token", "behavior_token_equal", "source_equal", "binary_equal")}))
    print(record["baked_owner_pin"]["pin_holds"], len(owners), "committed core owners")


if __name__ == "__main__":
    main()
