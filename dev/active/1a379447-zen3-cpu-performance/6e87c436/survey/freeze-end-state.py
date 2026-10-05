#!/usr/bin/env python3
"""Freeze the tree the verdict's records describe, by content (jit:66698c3c).

Writes `dense-verdict-end-state.json` beside itself: the SHA-256 of every file
the measured production packages hold at COMMIT. The script fails unless every
`current_sha256` of `production-drift.json` is the digest recorded there, so the
frozen tree is the one the committed records were generated from. The commit id
is an informational field; the checks read digests only.

Usage: freeze-end-state.py COMMIT
"""

import json
import sys

from locate import END_STATE_FILE, END_STATE_SCHEMA, HERE, PACKAGES, ROOT, content_anchor, repository_files


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    directories = [repository_files.package_directory(ROOT, name) for name in PACKAGES]
    pinned = content_anchor.Anchor(ROOT, HERE / END_STATE_FILE)
    count = pinned.freeze(sys.argv[1], directories, END_STATE_SCHEMA, "66698c3c")
    drift = json.loads((HERE / "production-drift.json").read_bytes())
    recorded = pinned.digests()
    disagree = sorted(
        entry["path"]
        for baseline in drift["baselines"]
        for entry in baseline["files"]
        if "current_sha256" in entry and recorded.get(entry["path"]) != entry["current_sha256"]
    )
    if disagree:
        (HERE / END_STATE_FILE).unlink()
        raise SystemExit(f"{sys.argv[1]} is not the tree of the records: {disagree}")
    print(f"{pinned.baseline.relative_to(ROOT)}: {count} files")


if __name__ == "__main__":
    main()
