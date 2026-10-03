"""Locates repository files under the runtime repository root.

Checks identify their inputs by file name and content among the files git
lists, never by a literal repository path (`@/inv/no-dev-path-coupling`).
"""

from __future__ import annotations

import subprocess
from pathlib import Path

# Receipt input snapshots hold byte copies of tracked files beside the receipt;
# a lookup for the live file excludes them.
SNAPSHOT_DIRECTORY = "inputs"


def repository_root(anchor: Path) -> Path:
    """The git work tree containing `anchor`, a file or directory."""
    directory = anchor if anchor.is_dir() else anchor.parent
    listing = subprocess.run(
        ["git", "-C", str(directory), "rev-parse", "--show-toplevel"],
        capture_output=True,
        check=True,
        text=True,
    )
    return Path(listing.stdout.strip())


def is_snapshot_copy(path: str) -> bool:
    """Whether the repository-relative `path` lies in a receipt input snapshot."""
    return SNAPSHOT_DIRECTORY in Path(path).parts[:-1]


def tracked_files(root: Path, name: str) -> list[str]:
    """Sorted root-relative paths of the live files called `name`.

    Lists tracked and untracked, non-ignored files, as the campaign driver does.
    """
    listing = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z", "--cached", "--others",
         "--exclude-standard", "--", f":(glob)**/{name}"],
        capture_output=True,
        check=True,
        text=True,
    ).stdout
    return sorted(
        {path for path in listing.split("\0")
         if path and (root / path).is_file() and not is_snapshot_copy(path)}
    )
