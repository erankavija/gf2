#!/usr/bin/env python3
"""Checks that every committed benchmark receipt carries the input files it pins.

`benchmark-acceptance` re-reads a receipt's frozen input snapshot and compares
it against the digests the receipt pins, so a snapshot file that reaches `main`
uncommitted turns an accepted receipt into a rejected one on a fresh checkout.
`.gitignore` excludes nested `Cargo.lock` files, which is the route such an
omission takes.

The check reads committed content only: paths come from the git index and bytes
from the blobs it names, so the verdict is the verdict a fresh checkout gets
whatever untracked files sit in the working tree.

Usage:
  dev/scripts/check-receipt-input-snapshots.py [--self-test]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable, Iterator, NamedTuple

PRODUCING_ROOT = "inputs/producing"
PRODUCING_SNAPSHOT = f"{PRODUCING_ROOT}/producing-snapshot.json"
RESOLUTION_RECEIPT = "inputs/resolution-evidence/receipt.json"
RESOLUTION_ADDENDUM = "inputs/resolution-evidence/family-addendum.json"
RESOLUTION_LEDGER = "inputs/resolution-evidence/trial-ledger.jsonl"
PRIOR_TRIAL_ROOT = "inputs/prior-trials"

ABSENT = "absent from the commit"
DIFFERS = "committed content differs from the pinned digest"
UNREADABLE = "pinned by a snapshot that does not decode"


class Pin(NamedTuple):
    """One repository-relative file a receipt pins by content."""

    path: str
    sha256: str
    origin: str


class Finding(NamedTuple):
    """One pinned file a fresh checkout cannot reproduce."""

    receipt: str
    path: str
    origin: str
    problem: str


def git(root: Path, *arguments: str) -> bytes:
    """Runs one git command in `root` and returns its raw stdout."""
    return subprocess.run(
        ["git", "-C", str(root), *arguments], capture_output=True, check=True
    ).stdout


def index_oids(root: Path) -> dict[str, str]:
    """Maps every path in the git index to the blob it names."""
    oids: dict[str, str] = {}
    for record in git(root, "ls-files", "-s", "-z").decode().split("\0"):
        if record:
            metadata, path = record.split("\t", 1)
            oids[path] = metadata.split(" ")[1]
    return oids


def committed_reader(root: Path, oids: dict[str, str]) -> Callable[[str], bytes | None]:
    """Returns a reader of committed bytes by repository-relative path."""

    def read(path: str) -> bytes | None:
        oid = oids.get(path)
        return None if oid is None else git(root, "cat-file", "blob", oid)

    return read


def receipt_paths(oids: dict[str, str]) -> list[str]:
    """Lists every committed campaign receipt, snapshot copies excluded."""
    return sorted(
        path
        for path in oids
        if path.startswith("dev/bench_results/")
        and path.endswith("/receipt.json")
        and "/inputs/" not in path
    )


def artifact_pins(node: object) -> Iterator[tuple[str, str]]:
    """Yields every (snapshot, sha256) artifact pin nested in a document."""
    if isinstance(node, dict):
        snapshot, sha256 = node.get("snapshot"), node.get("sha256")
        if isinstance(snapshot, str) and isinstance(sha256, str):
            yield snapshot, sha256
        for value in node.values():
            yield from artifact_pins(value)
    elif isinstance(node, list):
        for value in node:
            yield from artifact_pins(value)


def decode(read: Callable[[str], bytes | None], path: str) -> object | None:
    """Decodes a committed JSON document, or returns None if it is unusable."""
    content = read(path)
    if content is None:
        return None
    try:
        return json.loads(content)
    except json.JSONDecodeError:
        return None


def pinned_inputs(
    receipt_dir: str, receipt: dict, read: Callable[[str], bytes | None]
) -> tuple[list[Pin], list[Finding]]:
    """Collects every file a receipt pins, with the snapshots it cannot read.

    The producing snapshot, the receipt's own artifact pins, the prior-trial
    snapshots its frozen addendum names, and — from protocol version 3 — the
    pilot addendum and ledger `benchmark-acceptance` re-derives from the
    resolution-evidence receipt.
    """
    pins: list[Pin] = []
    findings: list[Finding] = []

    def under(relative: str) -> str:
        return f"{receipt_dir}/{relative}"

    snapshot = decode(read, under(PRODUCING_SNAPSHOT))
    if isinstance(snapshot, dict):
        selected = {snapshot["manifest_path"]: snapshot["manifest_sha256"]}
        for group in ("behavior_sha256", "lifecycle_sha256", "build_inputs_sha256"):
            selected.update(snapshot.get(group, {}))
        pins.extend(
            Pin(under(f"{PRODUCING_ROOT}/{path}"), sha256, "producing snapshot")
            for path, sha256 in sorted(selected.items())
        )
    else:
        findings.append(
            Finding(
                under("receipt.json"),
                under(PRODUCING_SNAPSHOT),
                "producing snapshot",
                ABSENT if snapshot is None else UNREADABLE,
            )
        )

    pins.extend(
        Pin(under(relative), sha256, "receipt pin")
        for relative, sha256 in artifact_pins(receipt)
    )

    addendum_snapshot = receipt.get("addendum", {}).get("snapshot")
    addendum = (
        decode(read, under(addendum_snapshot))
        if isinstance(addendum_snapshot, str)
        else None
    )
    if isinstance(addendum, dict):
        for trial in addendum.get("family_wise", {}).get("prior_trials", []) or []:
            sha256 = trial["sha256"]
            pins.append(
                Pin(
                    under(f"{PRIOR_TRIAL_ROOT}/{sha256}.json"),
                    sha256,
                    "prior-trial snapshot",
                )
            )
        evidence = addendum.get("effect", {}).get("resolution_evidence")
        version = addendum.get("protocol", {}).get("version", 0)
        if evidence:
            pins.append(
                Pin(under(RESOLUTION_RECEIPT), evidence["sha256"], "resolution evidence")
            )
        if evidence and version >= 3:
            pilot = decode(read, under(RESOLUTION_RECEIPT))
            if isinstance(pilot, dict):
                pins.append(
                    Pin(
                        under(RESOLUTION_ADDENDUM),
                        pilot["addendum"]["sha256"],
                        "resolution evidence",
                    )
                )
                ledger = pilot.get("trial_ledger")
                if ledger:
                    pins.append(
                        Pin(
                            under(RESOLUTION_LEDGER),
                            ledger["sha256"],
                            "resolution evidence",
                        )
                    )
    return pins, findings


def check(root: Path) -> list[Finding]:
    """Reports every pinned input a fresh checkout of `root` cannot reproduce."""
    oids = index_oids(root)
    read = committed_reader(root, oids)
    findings: list[Finding] = []
    for path in receipt_paths(oids):
        receipt = decode(read, path)
        if not isinstance(receipt, dict):
            findings.append(Finding(path, path, "receipt", UNREADABLE))
            continue
        pins, snapshot_findings = pinned_inputs(str(Path(path).parent), receipt, read)
        findings.extend(snapshot_findings)
        for pin in pins:
            content = read(pin.path)
            if content is None:
                findings.append(Finding(path, pin.path, pin.origin, ABSENT))
            elif hashlib.sha256(content).hexdigest() != pin.sha256:
                findings.append(Finding(path, pin.path, pin.origin, DIFFERS))
    return findings


def report(findings: list[Finding], stream) -> None:
    """Prints one line per finding, grouped by receipt."""
    for receipt in sorted({finding.receipt for finding in findings}):
        print(receipt, file=stream)
        for finding in findings:
            if finding.receipt == receipt:
                print(
                    f"    {finding.path} ({finding.origin}): {finding.problem}",
                    file=stream,
                )


def write_fixture(root: Path, omit: str | None = None, corrupt: str | None = None) -> None:
    """Stages a minimal receipt whose producing snapshot pins two files."""
    receipt_dir = root / "dev/bench_results/fixture/pilot"
    producing = receipt_dir / PRODUCING_ROOT
    (producing / "dev").mkdir(parents=True)
    manifest = producing / "manifest.json"
    manifest.write_bytes(b'{"schema":"fixture"}\n')
    pinned = b"version = 4\n"
    (producing / "dev/Cargo.lock").write_bytes(
        b"version = 3\n" if corrupt == "dev/Cargo.lock" else pinned
    )
    (producing / "producing-snapshot.json").write_text(
        json.dumps(
            {
                "manifest_path": "manifest.json",
                "manifest_sha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
                "behavior_sha256": {},
                "lifecycle_sha256": {},
                "build_inputs_sha256": {
                    "dev/Cargo.lock": hashlib.sha256(pinned).hexdigest()
                },
            }
        )
    )
    (receipt_dir / "receipt.json").write_text(json.dumps({"campaign_id": "fixture"}))
    git(root, "add", "-f", "dev")
    if omit is not None:
        git(root, "rm", "--cached", "-q", f"{producing.relative_to(root)}/{omit}")


def self_test() -> int:
    """Asserts the check accepts a complete snapshot and rejects each defect."""
    with tempfile.TemporaryDirectory() as directory:
        for case, omit, corrupt, expected in (
            ("complete", None, None, []),
            ("omitted", "dev/Cargo.lock", None, [ABSENT]),
            ("changed", None, "dev/Cargo.lock", [DIFFERS]),
        ):
            root = Path(directory) / case
            root.mkdir()
            git(root, "init", "-q")
            write_fixture(root, omit=omit, corrupt=corrupt)
            problems = [finding.problem for finding in check(root)]
            if problems != expected:
                print(
                    f"self-test case {case}: expected {expected}, observed {problems}",
                    file=sys.stderr,
                )
                return 1
    print("check-receipt-input-snapshots: self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="check the checker against a synthetic complete and incomplete snapshot",
    )
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").decode().strip())
    findings = check(root)
    if findings:
        report(findings, sys.stderr)
        print(
            f"{len(findings)} pinned receipt inputs are missing from the commit",
            file=sys.stderr,
        )
        return 1
    print("check-receipt-input-snapshots: every committed receipt pins committed inputs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
