#!/usr/bin/env python3
"""Print where the field-dispatch `dev/active` entries of issue 616e1d7c live.

  616e1d7c-entry-state.py > 616e1d7c-entry-state.txt

Entries are the Appendix A rows of `investigation.md` whose top epic includes
CONTAINER, minus EXCLUDED. The manifest and the archive root are located through
git; the tracker is the `.jit/issues` of the checkout. One tab-separated line
per entry:

  entry, owner issues, membership labels of the owners, tracked files under the
  flat path, flat files with a byte-identical archive copy, flat files without an
  archive copy, tracked files under the archive path, tracker references to the
  flat path, tracker references to the archive path, complete manifest rows,
  pending manifest rows, destination root of the pending rows.

A header line states the tracker state of CONTAINER and of CO_OWNER.
"""
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

CONTAINER = "6dc81018"
CO_OWNER = "86b9c719"
EXCLUDED = {"a83583e0", "dbd8787d"}  # code-pinned entries with their own move issues


def git(root, *args):
    return [p for p in subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, text=True, check=True,
    ).stdout.split("\0") if p]


def digest(path):
    return hashlib.sha256(path.read_bytes()).digest()


def main():
    root = Path(git(".", "rev-parse", "--show-toplevel")[0].strip())
    (manifest,) = git(root, "ls-files", "-z", "--", ":(glob)**/migration/manifest.toml")
    plan_dir = (root / manifest).parent.parent
    active = plan_dir.parent.relative_to(root).as_posix()
    issues = {}
    for f in (root / ".jit/issues").glob("*.json"):
        issue = json.loads(f.read_text())
        issues[issue["id"][:8]] = issue
    markers = [m for m in git(root, "ls-files", "-z", "--", ":(glob)**/.jit-container")
               if (root / m).read_text().strip() == issues[CONTAINER]["id"]]
    if len(markers) != 1:
        sys.exit(f"{len(markers)} archive markers for container {CONTAINER}; exactly one must exist")
    archive = f"{Path(markers[0]).parent.as_posix()}/active"

    appendix = (plan_dir / "investigation.md").read_text().split("## Appendix A", 1)[1].split("\n## ", 1)[0]
    entries = []
    for line in appendix.splitlines():
        cells = [c.strip().strip("`") for c in line.split("|")]
        if len(cells) == 9 and CONTAINER in cells[6] and cells[1] not in EXCLUDED:
            entries.append(cells[1])

    rows = tomllib.loads((root / manifest).read_text())["artifacts"]
    refs = [(short, doc["path"]) for short, issue in issues.items() for doc in issue.get("documents", [])]
    print(f"# {CONTAINER} {issues[CONTAINER]['state']}; {CO_OWNER} {issues[CO_OWNER]['state']}")
    for entry in sorted(entries):
        flat, arch = f"{active}/{entry}", f"{archive}/{entry}"
        under = lambda path, base: path == base or path.startswith(base + "/")
        flat_files = git(root, "ls-files", "-z", "--", flat)
        arch_files = git(root, "ls-files", "-z", "--", arch)
        copies = [f for f in flat_files if (root / arch / f[len(flat) + 1:]).is_file()]
        identical = [f for f in copies if digest(root / f) == digest(root / arch / f[len(flat) + 1:])]
        flat_refs = [r for r in refs if under(r[1], flat)]
        arch_refs = [r for r in refs if under(r[1], arch)]
        owners = sorted({short for short, _ in flat_refs + arch_refs})
        labels = sorted({l for o in owners for l in issues[o]["labels"] if l.startswith("epic:")})
        mine = [r for r in rows if under(r["path"], flat)]
        pending = [r for r in mine if r["status"] == "pending"]
        roots = sorted({re.sub(rf"(/{re.escape(entry)})(/.*)?$", r"\1", r["destination"]) or "(in place)" for r in pending})
        print("\t".join(str(x) for x in (
            entry, ",".join(owners) or "-", ",".join(labels) or "-", len(flat_files), len(identical),
            len(flat_files) - len(copies), len(arch_files), len(flat_refs), len(arch_refs),
            len(mine) - len(pending), len(pending), ",".join(roots) or "-",
        )))


if __name__ == "__main__":
    main()
