#!/usr/bin/env python3
"""Marks build identity records that name an executable no package builds (jit:b2e09d41).

A build identity record is evidence: receipts pin its bytes, so it is never
rewritten. For every live `*build-identity.json` whose `executables` maps name
an executable for which no live Cargo package declares a binary target, this
script writes `<record stem>-supersession.json` beside the record. The sibling
names the record by digest, lists those executables with the digests the
record holds, and lists the packages' binary targets it compared against. A
sibling whose record names no such executable is removed.

Usage (from any directory of the checkout):
  build-identity-supersession.py
"""

import hashlib
import json
import pathlib
import subprocess
import tomllib

SCHEMA = "build-identity-supersession-v1"
RECORD_GLOB = "*build-identity.json"
SNAPSHOT_DIRECTORY = "inputs"

HERE = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path(subprocess.run(
    ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
    check=True, capture_output=True, text=True).stdout.strip())


def live(glob):
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--cached", "--others", "--exclude-standard",
         "--", f":(glob)**/{glob}"],
        check=True, capture_output=True, text=True).stdout.split("\n")
    return [pathlib.Path(path) for path in sorted(filter(None, listing))
            if SNAPSHOT_DIRECTORY not in pathlib.Path(path).parts and (ROOT / path).is_file()]


def binary_targets():
    """Binary target names of every live Cargo package, by Cargo's discovery rules."""
    targets = set()
    for manifest in live("Cargo.toml"):
        document = tomllib.loads((ROOT / manifest).read_text())
        package = document.get("package")
        if package is None:
            continue
        directory = ROOT / manifest.parent
        targets.update(target["name"] for target in document.get("bin", []) if "name" in target)
        if package.get("autobins", True):
            if (directory / "src/main.rs").is_file():
                targets.add(package["name"])
            targets.update(path.stem for path in (directory / "src/bin").glob("*.rs"))
            targets.update(path.parent.name for path in (directory / "src/bin").glob("*/main.rs"))
    return targets


def named_executables(node, trail=()):
    """(location, name, recorded value) of every entry of an `executables` map."""
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "executables" and isinstance(value, dict):
                for name, recorded in value.items():
                    yield "/".join((*trail, key)), name, recorded
            else:
                yield from named_executables(value, trail + (key,))


def main():
    targets = binary_targets()
    for record in live(RECORD_GLOB):
        data = (ROOT / record).read_bytes()
        sibling = ROOT / record.with_name(f"{record.stem}-supersession.json")
        without_target = [
            {"location": location, "executable": name, "recorded": recorded}
            for location, name, recorded in named_executables(json.loads(data))
            if name not in targets
        ]
        if not without_target:
            sibling.unlink(missing_ok=True)
            continue
        sibling.write_text(json.dumps({
            "schema": SCHEMA,
            "record": record.name,
            "record_sha256": hashlib.sha256(data).hexdigest(),
            "superseded": (
                "the record's executable set: no live Cargo package declares a binary target "
                "for the executables listed; the record's bytes and its other fields stand"
            ),
            "executables_without_binary_target": without_target,
            "recorded_executables_with_binary_target": sorted({
                name for _, name, _ in named_executables(json.loads(data)) if name in targets}),
        }, indent=2) + "\n")
        print(sibling.relative_to(ROOT))


if __name__ == "__main__":
    main()
