#!/usr/bin/env python3
"""Generate or check the tracker document links for manifest rows owned by identifier or commit provenance.

Run from the repository root. Reads migration/manifest.toml beside this file and
.jit/issues/*.json of the current directory.

  67048b47-doc-links.py             print the idempotent link script
  67048b47-doc-links.py --check     list selected (owner, path) pairs lacking a link; exit 1 if any
  67048b47-doc-links.py --unassociated | --absent | --owner-missing | --directory
                                    list the rows deliberately not linked, by category
  67048b47-doc-links.py --selected | --multi-owner | --misplaced | --binary
                                    list selected files: all; with several owners; outside the
                                    owner's canonical directory (`jit doc dir`); scanned as non-UTF-8
"""
import json
import os
import shlex
import subprocess
import sys
import tomllib
from pathlib import Path

MANIFEST = Path(__file__).resolve().parent / "migration" / "manifest.toml"
PROVENANCE = ("issue-id", "commit")
CODE = {".py", ".sh", ".rs", ".c", ".cpp", ".h", ".cmd", ".lean", ".sage", ".css", ".g"}
SUFFIX_TYPE = {".md": "notes", ".log": "log", ".html": "presentation", ".png": "figure", ".svg": "figure"}


def doc_type(path):
    if os.path.isdir(path):
        return "other"
    suffix = os.path.splitext(path)[1]
    return "tool" if suffix in CODE else SUFFIX_TYPE.get(suffix, "data")


def is_binary(path):
    try:
        Path(path).read_bytes().decode("utf-8")
        return False
    except UnicodeDecodeError:
        return True


def issue_areas():
    config = tomllib.loads(Path(".jit/config.toml").read_text())
    return config["documentation"]["issue_scoped_areas"]


_CANONICAL = {}


def canonical_dir(owner, area):
    if (owner, area) not in _CANONICAL:
        out = subprocess.run(["jit", "doc", "dir", owner, area], capture_output=True, text=True, check=True)
        _CANONICAL[(owner, area)] = out.stdout.strip()
    return _CANONICAL[(owner, area)]


def misplaced(owner, path):
    for area in issue_areas():
        if path.startswith(area + "/"):
            canon = canonical_dir(owner, area)
            return not (path == canon or path.startswith(canon + "/"))
    return False


def load_tracker():
    """Map short id to the set of document paths the issue links."""
    issues = {}
    for f in sorted(Path(".jit/issues").glob("*.json")):
        data = json.loads(f.read_text())
        issues[data.get("short_id") or f.name[:8]] = {d["path"].rstrip("/") for d in data.get("documents", [])}
    return issues


_RENAMES = []


def renamed_to(path):
    """Most recent rename target of the file `path`; git records no directory renames."""
    if not _RENAMES:
        log = subprocess.run(
            ["git", "log", "-M", "--diff-filter=R", "--name-status", "--format="],
            capture_output=True, text=True, check=True,
        ).stdout
        _RENAMES.extend(line.split("\t")[1:] for line in log.splitlines())
    return next((new for old, new in _RENAMES if old == path), None)


def current_path(row):
    """The row's path, else its destination, else where history renamed the file; None when it exists nowhere."""
    for p in (row["path"], row["destination"]):
        if p and os.path.lexists(p):
            return p
    p = row["path"]
    for _ in range(8):
        p = renamed_to(p)
        if p is None:
            return None
        if os.path.lexists(p):
            return p
    return None


SELECTED = []


def classify(rows, issues):
    """Return (missing, absent, owner_missing, unassociated, directory)."""
    missing, absent, owner_missing, unassociated, directory = [], [], [], [], []
    SELECTED.clear()
    for r in rows:
        if r["evidence"] == "none":
            unassociated.append(r["path"])
        if r["evidence"] not in PROVENANCE or not r["owners"]:
            continue
        path = current_path(r)
        for owner in r["owners"]:
            if owner not in issues:
                owner_missing.append((owner, r["path"]))
            elif path is None:
                absent.append((owner, r["path"]))
            elif os.path.isdir(path):
                directory.append((owner, path))
            else:
                SELECTED.append((owner, path, len(r["owners"])))
                if path.rstrip("/") not in issues[owner]:
                    missing.append((owner, path))
    return sorted(set(missing)), sorted(set(absent)), sorted(set(owner_missing)), sorted(set(unassociated)), sorted(set(directory))


LISTINGS = ("--unassociated", "--absent", "--owner-missing", "--directory",
            "--selected", "--multi-owner", "--misplaced", "--binary")


def main(argv):
    rows = tomllib.loads(MANIFEST.read_text())["artifacts"]
    missing, absent, owner_missing, unassociated, directory = classify(rows, load_tracker())
    mode = argv[0] if argv else ""
    if mode == "--check":
        for owner, path in missing:
            print(owner, path)
        return 1 if missing else 0
    if mode == "--unassociated":
        listing = unassociated
    elif mode == "--absent":
        listing = [f"{o} {p}" for o, p in absent]
    elif mode == "--owner-missing":
        listing = [f"{o} {p}" for o, p in owner_missing]
    elif mode == "--directory":
        listing = [f"{o} {p}" for o, p in directory]
    elif mode == "--selected":
        listing = sorted({p for _, p, _ in SELECTED})
    elif mode == "--multi-owner":
        listing = sorted({p for _, p, n in SELECTED if n > 1})
    elif mode == "--misplaced":
        listing = sorted({p for o, p, _ in SELECTED if misplaced(o, p)})
    elif mode == "--binary":
        listing = sorted({p for _, p, _ in SELECTED if is_binary(p)})
    if mode in LISTINGS:
        for line in listing:
            print(line)
    elif mode == "":
        print("#!/usr/bin/env bash\n# Generated by 67048b47-doc-links.py; run from the repository root.")
        print("set -euo pipefail\nrun() { printf '%s\\n' \"$*\"; \"$@\" || \"$@\" --skip-scan; }")
        for owner, path in missing:
            label = os.path.basename(path.rstrip("/"))
            print(f"run jit doc add {owner} {shlex.quote(path)} --doc-type {doc_type(path)} --label {shlex.quote(label)}{' --skip-scan' if is_binary(path) else ''}")
    else:
        sys.exit(__doc__)
    return 0


if __name__ == "__main__":
    sys.dont_write_bytecode = True
    sys.exit(main(sys.argv[1:]))
