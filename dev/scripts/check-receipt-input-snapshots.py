#!/usr/bin/env python3
"""Checks that every committed benchmark receipt carries the input files it pins.

`benchmark-acceptance` re-reads a receipt's frozen input snapshot and compares
it against the digests the receipt pins, so a snapshot file that reaches `main`
uncommitted turns an accepted receipt into a rejected one on a fresh checkout.
`.gitignore` excludes nested `Cargo.lock` files, which is the route such an
omission takes.

The check reads committed content only: paths come from the git index, or from
a named revision, and bytes from the blobs they name, so the verdict is the
verdict a fresh checkout gets whatever untracked files sit in the working tree.

A pinned file whose content survives nowhere in the repository cannot be
restored. `dev/scripts/receipt-input-omissions.json` registers each such file
with its cause and the documents that state which conclusions it affects; the
check prints those and passes, and fails on an unregistered omission and on a
registered one that is no longer missing.

Usage:
  dev/scripts/check-receipt-input-snapshots.py [--revision <rev>] [--self-test]
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
OMISSIONS = "dev/scripts/receipt-input-omissions.json"
OMISSIONS_SCHEMA = "receipt-input-omissions-v1"

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


def committed_oids(root: Path, revision: str | None = None) -> dict[str, str]:
    """Maps every committed path to the blob it names, in `revision` or the index."""
    listing = (
        git(root, "ls-tree", "-r", "-z", revision)
        if revision is not None
        else git(root, "ls-files", "-s", "-z")
    )
    oids: dict[str, str] = {}
    for record in listing.decode().split("\0"):
        if record:
            metadata, path = record.split("\t", 1)
            oids[path] = metadata.split(" ")[2 if revision is not None else 1]
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


def check(root: Path, revision: str | None = None) -> list[Finding]:
    """Reports every pinned input a fresh checkout of `root` cannot reproduce."""
    oids = committed_oids(root, revision)
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


def registered_omissions(
    read: Callable[[str], bytes | None]
) -> tuple[set[tuple[str, str]], list[str]]:
    """Reads the registry of pinned files no committed content can restore."""
    registry = decode(read, OMISSIONS)
    if registry is None:
        return set(), []
    if not isinstance(registry, dict) or registry.get("schema") != OMISSIONS_SCHEMA:
        return set(), [f"{OMISSIONS}: schema is not {OMISSIONS_SCHEMA}"]
    return {
        (entry["receipt"], entry["path"]) for entry in registry["omissions"]
    }, []


def partition(
    findings: list[Finding], registered: set[tuple[str, str]]
) -> tuple[list[Finding], list[Finding], list[str]]:
    """Splits findings into unregistered and registered, and names stale entries."""
    keys = {(finding.receipt, finding.path) for finding in findings}
    unregistered = [
        finding
        for finding in findings
        if (finding.receipt, finding.path) not in registered
    ]
    recorded = [
        finding for finding in findings if (finding.receipt, finding.path) in registered
    ]
    stale = [
        f"{OMISSIONS} registers {path} of {receipt}, which is not missing"
        for receipt, path in sorted(registered - keys)
    ]
    return unregistered, recorded, stale


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


def write_registry(root: Path, receipt: str, path: str) -> None:
    """Stages a registry that records one pinned file as unrestorable."""
    registry = root / OMISSIONS
    registry.parent.mkdir(parents=True, exist_ok=True)
    registry.write_text(
        json.dumps(
            {
                "schema": OMISSIONS_SCHEMA,
                "omissions": [
                    {
                        "receipt": receipt,
                        "path": path,
                        "cause": "fixture",
                        "recorded_in": [],
                    }
                ],
            }
        )
    )
    git(root, "add", "-f", OMISSIONS)


def self_test() -> int:
    """Asserts the check accepts a complete snapshot and rejects each defect."""
    fixture_receipt = "dev/bench_results/fixture/pilot/receipt.json"
    fixture_lock = "dev/bench_results/fixture/pilot/inputs/producing/dev/Cargo.lock"
    cases = (
        ("complete", None, None, None, ([], [])),
        ("omitted", "dev/Cargo.lock", None, None, ([ABSENT], [])),
        ("changed", None, "dev/Cargo.lock", None, ([DIFFERS], [])),
        ("registered", "dev/Cargo.lock", None, fixture_lock, ([], [ABSENT])),
        ("stale", None, None, fixture_lock, ([], [])),
    )
    with tempfile.TemporaryDirectory() as directory:
        for case, omit, corrupt, register, expected in cases:
            root = Path(directory) / case
            root.mkdir()
            git(root, "init", "-q")
            write_fixture(root, omit=omit, corrupt=corrupt)
            if register is not None:
                write_registry(root, fixture_receipt, register)
            registered, errors = registered_omissions(
                committed_reader(root, committed_oids(root))
            )
            unregistered, recorded, stale = partition(check(root), registered)
            observed = (
                [finding.problem for finding in unregistered],
                [finding.problem for finding in recorded],
            )
            expected_stale = case == "stale"
            if observed != expected or errors or bool(stale) != expected_stale:
                print(
                    f"self-test case {case}: expected {expected} and "
                    f"stale={expected_stale}, observed {observed} and "
                    f"stale={stale} with {errors}",
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
    parser.add_argument(
        "--revision",
        help="check this revision instead of the git index",
    )
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").decode().strip())
    oids = committed_oids(root, arguments.revision)
    registered, errors = registered_omissions(committed_reader(root, oids))
    findings = check(root, arguments.revision)
    unregistered, recorded, stale = partition(findings, registered)
    if recorded:
        print(f"recorded in {OMISSIONS}:")
        report(recorded, sys.stdout)
    for message in errors + stale:
        print(message, file=sys.stderr)
    if unregistered:
        report(unregistered, sys.stderr)
        print(
            f"{len(unregistered)} pinned receipt inputs are missing from the commit",
            file=sys.stderr,
        )
    if unregistered or errors or stale:
        return 1
    print(
        "check-receipt-input-snapshots: every pinned receipt input is committed"
        + (f", {len(recorded)} recorded as unrestorable" if recorded else "")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
