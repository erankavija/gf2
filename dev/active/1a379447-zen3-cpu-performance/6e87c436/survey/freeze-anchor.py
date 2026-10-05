#!/usr/bin/env python3
"""Freeze the task anchor of the dense-parity outcome by content (jit:6e87c436).

Writes `anchor-baseline.json` beside itself: the SHA-256 of every file the
measured production packages hold at COMMIT. For each path whose working-tree
bytes differ, the anchor bytes are snapshotted under `inputs/anchor/`, as a
receipt snapshots its producing inputs. The generators read this record and
its snapshots only; the commit id is kept as an informational field.

Usage: freeze-anchor.py COMMIT
"""

import hashlib
import json
import subprocess
import sys

from locate import ANCHOR_BASELINE, ANCHOR_SNAPSHOT, PACKAGES, ROOT, repository_files


def git(*arguments):
    return subprocess.run(
        ["git", "-C", str(ROOT), *arguments], capture_output=True, check=True
    ).stdout


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    commit = git("rev-parse", sys.argv[1]).decode().strip()
    directories = [repository_files.package_directory(ROOT, name) for name in PACKAGES]
    digests = {}
    for path in git("ls-tree", "-r", "--name-only", commit, "--", *directories).decode().split():
        held = git("show", f"{commit}:{path}")
        digests[path] = hashlib.sha256(held).hexdigest()
        current = ROOT / path
        if not current.is_file() or current.read_bytes() != held:
            snapshot = ANCHOR_SNAPSHOT / path
            snapshot.parent.mkdir(parents=True, exist_ok=True)
            snapshot.write_bytes(held)
    ANCHOR_BASELINE.write_text(
        json.dumps(
            {
                "schema": "dense-outcome-anchor-v1",
                "issue": "6e87c436",
                "commit_informational": commit,
                "packages": directories,
                "sha256": digests,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"{ANCHOR_BASELINE.relative_to(ROOT)}: {len(digests)} anchor files")


if __name__ == "__main__":
    main()
