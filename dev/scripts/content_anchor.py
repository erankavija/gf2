"""Pins a task anchor by content.

A baseline record holds the SHA-256 of every file of the named package
directories at the anchor commit, and a snapshot directory holds the anchor
bytes of the paths the working tree changes, as a receipt snapshots its
producing inputs. Readers use the record and its snapshots only; the commit id
is an informational field. A path without a snapshot is read by digest from the
blobs git holds, never from the working tree.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path


def _git(root: Path, *arguments: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(root), *arguments], capture_output=True, check=True
    ).stdout


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def history_blob(root: Path, path: str, digest: str) -> bytes | None:
    """The newest committed content of `path` with SHA-256 `digest`, or `None`.

    Searches every commit of any ref that changed `path`.
    """
    history = _git(root, "log", "--all", "--format=%H", "--", path).decode().split()
    for commit in history:
        shown = subprocess.run(
            ["git", "-C", str(root), "show", f"{commit}:{path}"], capture_output=True
        )
        if shown.returncode == 0 and sha256(shown.stdout) == digest:
            return shown.stdout
    return None


class Anchor:
    """A baseline record at `baseline`, with byte snapshots under `snapshot` when it has any.

    The record also serves as a pinned end state: `require_matching_tree`
    refuses a working tree that differs from it.
    """

    def __init__(self, root: Path, baseline: Path, snapshot: Path | None = None):
        self.root, self.baseline, self.snapshot = root, baseline, snapshot

    def digests(self) -> dict[str, str]:
        """Root-relative path to SHA-256 for every package file at the anchor."""
        return json.loads(self.baseline.read_bytes())["sha256"]

    def identity(self) -> dict[str, str]:
        """The baseline record's root-relative path and digest."""
        return {
            "baseline": str(self.baseline.relative_to(self.root)),
            "baseline_sha256": sha256(self.baseline.read_bytes()),
        }

    def bytes_or_none(self, path: str) -> bytes | None:
        """The anchor bytes of `path`, or `None` when the anchor has no such file."""
        return self.bytes(path) if path in self.digests() else None

    def bytes(self, path: str) -> bytes:
        """The anchor bytes of `path`, checked against the baseline digest.

        Read from the snapshot when it holds `path` and from the committed
        blobs of `path` otherwise.
        """
        digest = self.digests()[path]
        held = None if self.snapshot is None else self.snapshot / path
        data = held.read_bytes() if held is not None and held.is_file() else history_blob(
            self.root, path, digest
        )
        if data is None or sha256(data) != digest:
            raise SystemExit(f"no file holds the baseline digest of {path}")
        return data

    def changed(self) -> list[str]:
        """Sorted package paths whose working-tree bytes differ from the anchor's, or exist on one side only."""
        digests = self.digests()
        packages = json.loads(self.baseline.read_bytes())["packages"]
        tracked = _git(self.root, "ls-files", "--", *packages).decode().split()
        changed = set(tracked) ^ set(digests)
        for path in set(tracked) & set(digests):
            if sha256((self.root / path).read_bytes()) != digests[path]:
                changed.add(path)
        return sorted(changed)

    def require_matching_tree(self, what: str) -> None:
        """Exits naming the differing paths unless the working tree is the recorded one."""
        differing = self.changed()
        if differing:
            raise SystemExit(
                f"{what} describes the tree pinned by {self.baseline.relative_to(self.root)}; "
                f"{len(differing)} paths of the working tree differ from it, so nothing is "
                "written: " + ", ".join(differing)
            )

    def freeze(self, commit: str, directories: list[str], schema: str, issue: str) -> int:
        """Writes the baseline for `commit` and snapshots each path the working tree changes.

        Returns the number of anchor files. Without a snapshot directory no
        bytes are kept.
        """
        commit = _git(self.root, "rev-parse", commit).decode().strip()
        digests = {}
        listing = _git(self.root, "ls-tree", "-r", "--name-only", commit, "--", *directories)
        for path in listing.decode().split():
            held = _git(self.root, "show", f"{commit}:{path}")
            digests[path] = sha256(held)
            current = self.root / path
            if self.snapshot is not None and (
                not current.is_file() or current.read_bytes() != held
            ):
                snapshot = self.snapshot / path
                snapshot.parent.mkdir(parents=True, exist_ok=True)
                snapshot.write_bytes(held)
        self.baseline.write_text(
            json.dumps(
                {
                    "schema": schema,
                    "issue": issue,
                    "commit_informational": commit,
                    "packages": directories,
                    "sha256": digests,
                },
                indent=2,
            )
            + "\n"
        )
        return len(digests)


def digest_changes(before: Anchor, after: Anchor) -> list[str]:
    """Sorted paths whose digests differ between two baselines, or exist in one only."""
    old, new = before.digests(), after.digests()
    return sorted(path for path in set(old) | set(new) if old.get(path) != new.get(path))
