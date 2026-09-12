#!/usr/bin/env python3
"""Restores the pinned receipt inputs a revision left uncommitted.

`dev/scripts/check-receipt-input-snapshots.py` reports every file a committed
receipt pins by digest and a checkout of that revision does not carry. This
script resolves each one to a file of the same revision whose bytes hash to the
pinned digest, writes it into the receipt's snapshot, stages it past
`.gitignore`, and records source and digest in
`dev/active/a203a23c/restored-inputs.json`.

Only content that already exists in the repository at the base revision is
written, so no measurement evidence is reconstructed and every restored byte
carries a committed source. A pinned file no committed file matches is recorded
as unrestored with its receipt.

Usage: dev/active/a203a23c/restore-receipt-inputs.py <base-revision>
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

RECORD = "dev/active/a203a23c/restored-inputs.json"
CHECKER = "dev/scripts/check-receipt-input-snapshots.py"
NO_SOURCE = "no file of the base revision carries this digest"


def load_checker(root: Path):
    """Imports the committed check so both tools share one notion of a pin."""
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("receipt_inputs", root / CHECKER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sources_by_digest(checker, root: Path, revision: str) -> dict[str, list[str]]:
    """Maps the sha256 of every file of `revision` to the paths carrying it."""
    oids = checker.committed_oids(root, revision)
    read = checker.committed_reader(root, oids)
    sources: dict[str, list[str]] = {}
    try:
        for path in oids:
            digest = hashlib.sha256(read(path)).hexdigest()
            sources.setdefault(digest, []).append(path)
    finally:
        read.close()
    return {digest: sorted(paths) for digest, paths in sources.items()}


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <base-revision>", file=sys.stderr)
        return 2
    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    )
    checker = load_checker(root)
    base = checker.git(root, "rev-parse", "--verify", f"{sys.argv[1]}^{{commit}}")
    base = base.decode().strip()

    oids = checker.committed_oids(root, base)
    read = checker.committed_reader(root, oids)
    findings = checker.check(root, base)
    sources = sources_by_digest(checker, root, base)
    pins = {
        (receipt, pin.path): pin
        for receipt in checker.receipt_paths(oids)
        for pin in checker.pinned_inputs(
            str(Path(receipt).parent), json.loads(read(receipt)), read
        )[0]
    }
    read.close()

    restored, unrestored = [], []
    for finding in findings:
        pin = pins.get((finding.receipt, finding.path))
        candidates = sources.get(pin.sha256, []) if pin else []
        entry = {
            "receipt": finding.receipt,
            "path": finding.path,
            "origin": finding.origin,
            "sha256": pin.sha256 if pin else None,
        }
        if not candidates:
            unrestored.append({**entry, "reason": NO_SOURCE})
            continue
        source = candidates[0]
        content = checker.git(root, "cat-file", "blob", f"{base}:{source}")
        destination = root / finding.path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
        checker.git(root, "add", "-f", finding.path)
        restored.append({**entry, "source": source, "source_revision": base})

    record = {
        "schema": "a203a23c-restored-receipt-inputs-v1",
        "purpose": (
            "Files added to committed receipt input snapshots so a fresh checkout "
            "reproduces the acceptance verdict, with the committed source of each."
        ),
        "generator": "dev/active/a203a23c/restore-receipt-inputs.py",
        "check": CHECKER,
        "base_revision": base,
        "restored": restored,
        "unrestored": unrestored,
        "result": {"restored": len(restored), "unrestored": len(unrestored)},
    }
    (root / RECORD).write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record["result"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
