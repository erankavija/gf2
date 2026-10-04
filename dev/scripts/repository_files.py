"""Locates repository files under the runtime repository root.

Checks identify their inputs by file name and content among the files git
lists, never by a literal repository path (`@/inv/no-dev-path-coupling`).

Usage:
  repository_files.py shared-producing-manifest
  repository_files.py package-directory <name>
  repository_files.py document <name> <opening>

Each command prints one root-relative path.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tomllib
from pathlib import Path

# Receipt input snapshots hold byte copies of tracked files beside the receipt;
# a lookup for the live file excludes them.
SNAPSHOT_DIRECTORY = "inputs"

# `protocol::SHARED_PRODUCING_MANIFEST` and the opening `SharedInput::identity`
# requires of a protocol document, in the campaign support crate.
SHARED_PRODUCING_MANIFEST = "producing-inputs.json"
PROTOCOL_OPENING = b"# Zen 3 benchmark protocol\n\nProtocol `zen3-benchmark-protocol` version "


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


def distinct_contents(root: Path, paths: list[str]) -> list[str]:
    """The first of `paths` holding each distinct content, in their order."""
    first: dict[bytes, str] = {}
    for path in paths:
        first.setdefault((root / path).read_bytes(), path)
    return list(first.values())


def shared_producing_manifest(root: Path) -> str:
    """Root-relative path of the producing manifest beside the live protocol documents.

    A protocol document is a `protocol*.md` opening with the protocol's title and
    identity sentence. Byte-identical manifests are one, named by its
    lexicographically first path, as `repository::locate_live` names a document.
    """
    beside = sorted(
        {
            str(Path(path).with_name(SHARED_PRODUCING_MANIFEST))
            for path in tracked_files(root, "protocol*.md")
            if (root / path).read_bytes().startswith(PROTOCOL_OPENING)
        }
    )
    found = distinct_contents(root, [path for path in beside if (root / path).is_file()])
    if len(found) != 1:
        raise LookupError(
            f"{len(found)} distinct producing manifests lie beside live protocol "
            "documents; exactly one must"
        )
    return found[0]


def document(root: Path, name: str, opening: bytes) -> str:
    """Root-relative path of the one live file matching `name` that opens with `opening`.

    `name` is a file-name glob. Byte-identical files are one, named by the
    lexicographically first path.
    """
    found = distinct_contents(
        root,
        [path for path in tracked_files(root, name)
         if (root / path).read_bytes().startswith(opening)],
    )
    if len(found) != 1:
        raise LookupError(
            f"{len(found)} distinct live files match {name} and open with "
            f"{opening.decode(errors='replace')!r}; exactly one must"
        )
    return found[0]


def package_directory(root: Path, name: str) -> str:
    """Root-relative directory of the one live Cargo package called `name`."""
    found = [
        str(Path(manifest).parent)
        for manifest in tracked_files(root, "Cargo.toml")
        if tomllib.loads((root / manifest).read_text()).get("package", {}).get("name") == name
    ]
    if len(found) != 1:
        raise LookupError(f"{len(found)} live packages are named {name}; exactly one must be")
    return found[0]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("shared-producing-manifest")
    commands.add_parser("package-directory").add_argument("name")
    by_opening = commands.add_parser("document")
    by_opening.add_argument("name")
    by_opening.add_argument("opening")
    arguments = parser.parse_args()
    root = repository_root(Path(__file__).resolve())
    try:
        if arguments.command == "package-directory":
            print(package_directory(root, arguments.name))
        elif arguments.command == "document":
            print(document(root, arguments.name, arguments.opening.encode()))
        else:
            print(shared_producing_manifest(root))
    except LookupError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
