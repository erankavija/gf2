"""Pins a task anchor by content.

A baseline record holds the SHA-256 of every file of the named package
directories at the anchor commit, and a snapshot directory holds the anchor
bytes of the paths the working tree changes, as a receipt snapshots its
producing inputs. Readers use the record and its snapshots only; the commit id
is an informational field.
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


class Anchor:
    """A baseline record at `baseline` with its byte snapshots under `snapshot`."""

    def __init__(self, root: Path, baseline: Path, snapshot: Path):
        self.root, self.baseline, self.snapshot = root, baseline, snapshot

    def digests(self) -> dict[str, str]:
        """Root-relative path to SHA-256 for every package file at the anchor."""
        return json.loads(self.baseline.read_bytes())["sha256"]

    def identity(self) -> dict[str, str]:
        """The baseline record's root-relative path and digest."""
        return {
            "baseline": str(self.baseline.relative_to(self.root)),
            "baseline_sha256": hashlib.sha256(self.baseline.read_bytes()).hexdigest(),
        }

    def bytes(self, path: str) -> bytes:
        """The anchor bytes of `path`, checked against the baseline digest.

        Read from the snapshot when it holds `path` and from the working tree
        otherwise.
        """
        held = self.snapshot / path
        data = (held if held.is_file() else self.root / path).read_bytes()
        if hashlib.sha256(data).hexdigest() != self.digests()[path]:
            raise SystemExit(f"no file holds the baseline digest of {path}")
        return data

    def changed(self) -> list[str]:
        """Sorted package paths whose working-tree bytes differ from the anchor's, or exist on one side only."""
        digests = self.digests()
        packages = json.loads(self.baseline.read_bytes())["packages"]
        tracked = _git(self.root, "ls-files", "--", *packages).decode().split()
        changed = set(tracked) ^ set(digests)
        for path in set(tracked) & set(digests):
            if hashlib.sha256((self.root / path).read_bytes()).hexdigest() != digests[path]:
                changed.add(path)
        return sorted(changed)

    def freeze(self, commit: str, directories: list[str], schema: str, issue: str) -> int:
        """Writes the baseline for `commit` and snapshots each path the working tree changes.

        Returns the number of anchor files.
        """
        commit = _git(self.root, "rev-parse", commit).decode().strip()
        digests = {}
        listing = _git(self.root, "ls-tree", "-r", "--name-only", commit, "--", *directories)
        for path in listing.decode().split():
            held = _git(self.root, "show", f"{commit}:{path}")
            digests[path] = hashlib.sha256(held).hexdigest()
            current = self.root / path
            if not current.is_file() or current.read_bytes() != held:
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
