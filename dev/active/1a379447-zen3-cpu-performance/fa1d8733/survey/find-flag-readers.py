#!/usr/bin/env python3
"""List every file that names the XOR unroll configurations (jit:fa1d8733).

Writes `flag-readers.json` beside itself with the files matching PATTERN, each
with its class under RULE:

  before   the kernel package's files in the tree `before-baseline.json`
           identifies
  current  every tracked file of the working tree other than the record

Exits nonzero after writing when a `current` file of class `workspace-source`
or `workspace-manifest` matches.

Usage: find-flag-readers.py
"""

import json
import re
import subprocess
from pathlib import Path

from locate import BASELINES, HERE, ISSUE, ROOT, repository_files

PATTERN = r"gf2_xor_unroll|\bXOR_UNROLL\b"
RULE = (
    "A file matches when a line of its text matches PATTERN, comments and "
    "literals included. Classes, first match wins: `receipt-snapshot`, a path "
    "under an `inputs` directory; `tracker`, a path under `.jit`; "
    "`workspace-source`, a `.rs` file under the directory of a Cargo package; "
    "`workspace-manifest`, a `Cargo.toml` or `Cargo.lock`, or a `config.toml` "
    "in a `.cargo` directory; `receipt`, a file under a directory that holds a "
    "`receipt.json`; `record`, any other file. A Cargo package is a directory "
    "holding a tracked `Cargo.toml` outside receipt snapshots, whether or not "
    "the root workspace lists it."
)
FORBIDDEN = ("workspace-source", "workspace-manifest")
MATCH = re.compile(PATTERN)


def git_paths(*arguments):
    listing = subprocess.run(
        ["git", "-C", str(ROOT), *arguments], capture_output=True, check=True, text=True
    ).stdout
    return [path for path in listing.split("\0") if path]


def under(path, directories):
    return any(str(parent) in directories for parent in Path(path).parents)


def classifier(paths):
    packages = {
        str(Path(path).parent) for path in paths
        if Path(path).name == "Cargo.toml" and not repository_files.is_snapshot_copy(path)
    }
    receipts = {str(Path(path).parent) for path in paths if Path(path).name == "receipt.json"}

    def classify(path):
        name, parts = Path(path).name, Path(path).parts
        if repository_files.is_snapshot_copy(path):
            return "receipt-snapshot"
        if parts[0] == ".jit":
            return "tracker"
        if name.endswith(".rs") and under(path, packages):
            return "workspace-source"
        if name in ("Cargo.toml", "Cargo.lock") or (name == "config.toml" and ".cargo" in parts):
            return "workspace-manifest"
        if under(path, receipts):
            return "receipt"
        return "record"

    return classify


def matches(paths, read, classify):
    """One row per matching path, with its matching line numbers."""
    rows = []
    for path in sorted(paths):
        try:
            text = read(path).decode()
        except UnicodeDecodeError:
            continue
        lines = [number for number, line in enumerate(text.splitlines(), 1) if MATCH.search(line)]
        if lines:
            rows.append({"path": path, "class": classify(path), "lines": lines})
    return rows


def counts(rows):
    found = {}
    for row in rows:
        found[row["class"]] = found.get(row["class"], 0) + 1
    return dict(sorted(found.items()))


def main():
    before = BASELINES["before"]
    held = sorted(before.digests())
    before_rows = matches(held, before.bytes, classifier(held))
    tracked = git_paths("ls-files", "-z")
    output = HERE / "flag-readers.json"
    # A symbolic link is listed with its target, which carries the text.
    files = [
        path for path in tracked
        if not (ROOT / path).is_symlink() and ROOT / path != output
    ]
    current_rows = matches(files, lambda path: (ROOT / path).read_bytes(), classifier(tracked))
    record = {
        "schema": "xor-unroll-flag-readers-v1",
        "issue": ISSUE,
        "pattern": PATTERN,
        "rule": RULE,
        "before": before.identity() | {"counts": counts(before_rows), "files": before_rows},
        "current": {"counts": counts(current_rows), "files": current_rows},
    }
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"{output.relative_to(ROOT)}: before {json.dumps(record['before']['counts'])}, "
          f"current {json.dumps(record['current']['counts'])}")
    forbidden = [row["path"] for row in current_rows if row["class"] in FORBIDDEN]
    if forbidden:
        raise SystemExit(f"workspace sources or manifests name the configurations: {forbidden}")


if __name__ == "__main__":
    main()
