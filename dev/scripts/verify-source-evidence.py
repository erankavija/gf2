#!/usr/bin/env python3
"""Verifies a source-evidence ledger against the sources its rows cite.

A ledger is a JSON object whose `claims` rows each carry `project`, `commit`,
`path`, `line`, `verbatim` and `sha256` (the digest of the whole cited file),
and optionally `occurrences` (the number of lines holding the cited text).

A row holds in a tree when the file there has the recorded digest, its line
`line` is `verbatim` and the occurrence count matches. Two frames are checked:

  recorded-commits  each row against the file at the commit the row records
  tree              each row against one tree: the working tree, or the
                    revision `--tree` names

`class:` in the output is `tree` when every row holds in the tree, so the
ledger tracks it; `recorded-commits` when every row holds at its recorded
commit and some row differs from the tree, so the ledger is a record pinned to
those commits; `unverified` otherwise. The exit status is zero when every row
holds in the frame `--frame` names.

`--record` writes, for a ledger inside the repository and each row that
differs from the `--tree` revision, the file digest and the lines of its text
there and the commits that changed its file and displaced its line.

Usage: verify-source-evidence.py LEDGER --project NAME --frame FRAME
                                 [--tree REVISION] [--record PATH]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

from repository_files import repository_root

RECORD_SCHEMA = "source-evidence-verification-v1"
FRAMES = ("recorded-commits", "tree")


class Repository:
    def __init__(self, root: Path):
        self.root = root

    def git(self, *arguments: str) -> bytes | None:
        """Standard output of a git command, or None when it fails."""
        done = subprocess.run(["git", "-C", str(self.root), *arguments], capture_output=True)
        return done.stdout if done.returncode == 0 else None

    def blob(self, revision: str, path: str) -> bytes | None:
        return self.git("show", f"{revision}:{path}")

    def working_file(self, path: str) -> bytes | None:
        try:
            return (self.root / path).read_bytes()
        except OSError:
            return None

    def changes(self, since: str, until: str, path: str) -> list[str]:
        """Commits after `since` up to `until` that change `path`, oldest first."""
        listing = self.git(
            "log", "--topo-order", "--reverse", "--format=%H", f"{since}..{until}", "--", path
        )
        return (listing or b"").decode().split()


def line_holds(row: dict, content: bytes) -> bool:
    lines = content.decode(errors="replace").splitlines()
    return 0 < row["line"] <= len(lines) and lines[row["line"] - 1] == row["verbatim"]


def positions(row: dict, content: bytes) -> list[int]:
    """One-based lines of `content` holding the cited text."""
    lines = content.decode(errors="replace").splitlines()
    return [index + 1 for index, line in enumerate(lines) if row["verbatim"] in line]


def faults(row: dict, content: bytes | None, where: str) -> list[str]:
    """Why `row` does not hold in `content`, the cited file in the frame `where`."""
    path = row["path"]
    if content is None:
        return [f"{path} does not resolve {where}"]
    found = []
    if hashlib.sha256(content).hexdigest() != row["sha256"]:
        found.append(f"digest of {path} {where} is not the recorded one")
    if not line_holds(row, content):
        found.append(f"line {row['line']} of {path} {where} is not the cited text")
    if "occurrences" in row and len(positions(row, content)) != row["occurrences"]:
        found.append(
            f"{row['occurrences']} occurrences recorded, {len(positions(row, content))} "
            f"in {path} {where}"
        )
    return found


def drift(repository: Repository, row: dict, tree: str) -> dict:
    """Where a row that differs from revision `tree` lies there, and since which commits."""
    content = repository.blob(tree, row["path"])
    changes = repository.changes(row["commit"], tree, row["path"])
    displaced = next(
        (
            commit
            for commit in changes
            if not line_holds(row, repository.blob(commit, row["path"]) or b"")
        ),
        None,
    )
    return {
        "path": row["path"],
        "verbatim": row["verbatim"],
        "recorded_commit": row["commit"],
        "recorded_line": row["line"],
        "tree_sha256": hashlib.sha256(content).hexdigest() if content is not None else None,
        "tree_lines": positions(row, content) if content is not None else [],
        "first_file_change": changes[0] if changes else None,
        "first_line_displacement": displaced,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ledger", type=Path)
    parser.add_argument("--project", required=True, help="the `project` of the rows to verify")
    parser.add_argument("--frame", required=True, choices=FRAMES)
    parser.add_argument("--tree", help="a revision to compare instead of the working tree")
    parser.add_argument("--record", type=Path, help="write the verification record here")
    arguments = parser.parse_args()
    if arguments.record and not arguments.tree:
        parser.error("--record compares a named revision; pass --tree")

    repository = Repository(repository_root(Path(__file__).resolve()))
    ledger_bytes = arguments.ledger.read_bytes()
    claims = json.loads(ledger_bytes)["claims"]
    rows = [row for row in claims if row["project"] == arguments.project]
    tree_name = f"the tree {arguments.tree}" if arguments.tree else "the working tree"

    recorded_faults, tree_faults, drifted = [], [], []
    holds_recorded = 0
    for row in rows:
        at_commit = faults(
            row, repository.blob(row["commit"], row["path"]), "at the recorded commit"
        )
        content = (
            repository.blob(arguments.tree, row["path"])
            if arguments.tree
            else repository.working_file(row["path"])
        )
        recorded_faults += at_commit
        holds_recorded += not at_commit
        if faults(row, content, f"in {tree_name}"):
            tree_faults.append(f"{row['path']}:{row['line']} differs from {tree_name}")
            if arguments.tree and not at_commit:
                drifted.append(drift(repository, row, arguments.tree))
    holds_tree = len(rows) - len(tree_faults)
    if rows and holds_tree == len(rows):
        ledger_class = "tree"
    elif rows and holds_recorded == len(rows):
        ledger_class = "recorded-commits"
    else:
        ledger_class = "unverified"

    print(f"rows of project {arguments.project}: {len(rows)}")
    print(f"rows of other projects, not verified: {len(claims) - len(rows)}")
    print(f"rows holding at their recorded commits: {holds_recorded}")
    print(f"rows holding in {tree_name}: {holds_tree}")
    print(f"class: {ledger_class}")

    if arguments.record:
        document = {
            "schema": RECORD_SCHEMA,
            "ledger": {
                "path": str(arguments.ledger.resolve().relative_to(repository.root)),
                "sha256": hashlib.sha256(ledger_bytes).hexdigest(),
            },
            "project": arguments.project,
            "compared_tree": arguments.tree,
            "class": ledger_class,
            "rows": len(rows),
            "rows_of_other_projects": len(claims) - len(rows),
            "holding_at_recorded_commits": holds_recorded,
            "holding_in_compared_tree": holds_tree,
            "differing_from_compared_tree": drifted,
        }
        arguments.record.write_text(json.dumps(document, indent=2) + "\n")

    failing = recorded_faults if arguments.frame == "recorded-commits" else tree_faults
    if not rows:
        failing = [f"no row of project {arguments.project}"]
    for fault in failing:
        print(fault, file=sys.stderr)
    return 1 if failing else 0


if __name__ == "__main__":
    raise SystemExit(main())
