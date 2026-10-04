#!/usr/bin/env python3
"""Evidence for the 026fc832 re-archive (jit:f29a9225).

Run from anywhere in the checkout:
    python3 <this file> [--extract PREVIEW.json]

`--extract` rewrites `f29a9225-identities.tsv` beside this file from a
`jit archive container 026fc832 --json` preview: destination, sha256 and byte
size per planned move or copy. Without it the script checks the tree and prints
one line per check; exit 1 on any failure.

The archive directory is the one whose `.jit-container` marker names the epic.
The sources this unit removed are read from history: every path a `jit:f29a9225`
commit deletes or renames away. The mirror of a source is its path below the
development root, re-rooted at the archive directory.
"""
import hashlib
import json
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path

EPIC = "026fc832-a480-4d07-8d25-47b8bfcb69a3"
UNIT = "jit:f29a9225"
TABLE = Path(__file__).with_name("f29a9225-identities.tsv")
LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


ROOT = Path(git("rev-parse", "--show-toplevel").strip())


def extract(preview: str) -> None:
    plan = json.load(open(preview))
    rows = [f"{a['destination']}\t{a['content_identity']['sha256']}\t{a['content_identity']['byte_size']}\n"
            for a in plan["artifacts"] if a["action"] in ("move", "copy")]
    TABLE.write_text("".join(sorted(rows)))


def removed_sources() -> dict[str, str]:
    """Maps each path a unit commit deletes or renames away to the blob id of its last version."""
    removed = {}
    log = git("-C", str(ROOT), "log", "--reverse", "--format=%H", "--grep", UNIT)
    for commit in log.split():
        raw = git("-C", str(ROOT), "diff-tree", "-r", "--no-commit-id", "--diff-filter=D", f"{commit}^", commit)
        for line in raw.splitlines():
            meta, path = line.split("\t")
            removed[path] = meta.split()[2]
    return removed


def scan(files: list[Path]) -> tuple[int, int]:
    """Counts inline local links and those whose path is missing; `/x` resolves from the repository root."""
    links = unresolved = 0
    for f in files:
        fenced = False
        for number, line in enumerate(f.read_text().splitlines(), 1):
            if re.match(r"\s*(```|~~~)", line):
                fenced = not fenced
            if fenced:
                continue
            for target in LINK.findall(line):
                if SKIP.match(target):
                    continue
                links += 1
                path = urllib.parse.unquote(target.split("#", 1)[0])
                if not (ROOT / path.lstrip("/") if path.startswith("/") else f.parent / path).exists():
                    unresolved += 1
                    print(f"{f.relative_to(ROOT)}:{number}: unresolved: {target}")
    return links, unresolved


def main() -> int:
    if sys.argv[1:2] == ["--extract"]:
        extract(sys.argv[2])
        return 0
    failures = 0
    markers = [ROOT / m for m in git("-C", str(ROOT), "ls-files", "*/.jit-container").split()]
    archives = [m.parent for m in markers if m.read_text().strip() == EPIC]
    print(f"archive directories whose marker names {EPIC}: {len(archives)}")
    if len(archives) != 1:
        return 1
    archive = archives[0]
    development_root = archive.parent.parent

    rows = [line.split("\t") for line in TABLE.read_text().splitlines()]
    mismatched = 0
    for destination, digest, size in rows:
        path = ROOT / destination
        if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != digest or path.stat().st_size != int(size):
            mismatched += 1
            print(f"byte mismatch: {destination}")
    print(f"planned destinations {len(rows)}: sha256 or size mismatches {mismatched}")
    failures += mismatched

    documents = [d["path"] for issue in sorted((ROOT / ".jit/issues").glob("*.json"))
                 for d in json.loads(issue.read_text()).get("documents", [])]
    removed = removed_sources()
    lost = unreferenced = stale = 0
    for source, blob in sorted(removed.items()):
        mirror = archive / (ROOT / source).relative_to(development_root)
        relative = mirror.relative_to(ROOT).as_posix()
        if not mirror.is_file() or git("-C", str(ROOT), "hash-object", relative).strip() != blob:
            lost += 1
            print(f"last bytes not in the archive: {source} ({blob})")
        if relative not in documents:
            unreferenced += 1
            print(f"no tracker reference: {relative}")
        if source in documents:
            stale += 1
            print(f"tracker reference names the removed source: {source}")
    print(f"sources removed by {UNIT} commits {len(removed)}: mirror differing from the last source blob {lost}, "
          f"mirror without a tracker reference {unreferenced}, tracker references naming the source {stale}")
    failures += lost + unreferenced + stale

    touched = git("-C", str(ROOT), "log", "--format=", "--name-only", "--grep", UNIT).split()
    edited = sorted({ROOT / f for f in touched if f.endswith(".md") and (ROOT / f).is_file()} - set(archive.rglob("*.md")))
    for label, files in (("archive directory", sorted(archive.rglob("*.md"))), (f"Markdown files edited by {UNIT} commits", edited)):
        links, unresolved = scan(files)
        print(f"link scan, {label}: files {len(files)}, local links {links}, unresolved {unresolved}")
        failures += unresolved
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
