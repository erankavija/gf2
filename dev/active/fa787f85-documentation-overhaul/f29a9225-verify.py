#!/usr/bin/env python3
"""Evidence for the 026fc832 re-archive (jit:f29a9225).

Run from the repository root:
    python3 dev/active/fa787f85-documentation-overhaul/f29a9225-verify.py [--extract PREVIEW.json]

`--extract` rewrites `f29a9225-identities.tsv` from a `jit archive container
026fc832 --json` preview: one line per planned move or copy with destination,
sha256, byte size, already-archived flag and the relinked `issue:index` owners.
Without it the script checks the working tree against that table and prints the
marker, byte, tracker-reference, relocated-crate and link-scan results. Exit 1
on any failure.
"""
import hashlib
import json
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path

HERE = Path(__file__).parent
TABLE = HERE / "f29a9225-identities.tsv"
ARCHIVE = Path("dev/archive/026fc832-gf2-core-sota-stretch")
EPIC = "026fc832-a480-4d07-8d25-47b8bfcb69a3"
EXECUTED = "b977e603b"
CRATE = "research/blas_sgemm_gf251"
RELOCATED = ("Cargo.toml", "build.rs", ".gitignore", "src/bin/bench_blas_gf251.rs")
REPOINTED = ("dev/active/7d7c647c/design.md", "dev/active/fa787f85-documentation-overhaul/investigation.md")
LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def extract(preview: str) -> None:
    plan = json.load(open(preview))
    rows = []
    for a in plan["artifacts"]:
        if a["action"] in ("move", "copy"):
            owners = ",".join(f"{c['issue']}:{c['document_index']}" for c in a["reference_changes"])
            identity = a["content_identity"]
            rows.append(f"{a['destination']}\t{identity['sha256']}\t{identity['byte_size']}\t"
                        f"{int(a['already_archived'])}\t{owners}\n")
    TABLE.write_text("".join(sorted(rows)))


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
                if not (Path(path.lstrip("/")) if path.startswith("/") else f.parent / path).exists():
                    unresolved += 1
                    print(f"{f}:{number}: unresolved: {target}")
    return links, unresolved


def main() -> int:
    if sys.argv[1:2] == ["--extract"]:
        extract(sys.argv[2])
        return 0
    failures = 0
    marker = (ARCHIVE / ".jit-container").read_text().strip()
    print(f"marker {ARCHIVE}/.jit-container: {marker} ({'matches' if marker == EPIC else 'DIFFERS from'} {EPIC})")
    failures += marker != EPIC

    rows = [line.split("\t") for line in TABLE.read_text().splitlines()]
    mismatched = moved = references = stale = 0
    for destination, digest, size, archived, owners in rows:
        moved += archived == "0"
        path = Path(destination)
        if not path.is_file() or sha(path.read_bytes()) != digest or path.stat().st_size != int(size):
            mismatched += 1
            print(f"byte mismatch: {destination}")
        for owner in filter(None, owners.split(",")):
            issue, index = owner.split(":")
            documents = json.loads(Path(f".jit/issues/{issue}.json").read_text())["documents"]
            references += 1
            if documents[int(index)]["path"] != destination:
                stale += 1
                print(f"stale reference: {issue[:8]} document {index}: {documents[int(index)]['path']}")
    print(f"destinations {len(rows)} (moved by this execution {moved}): sha256 and size mismatches {mismatched}")
    print(f"tracker references of those destinations {references}: not naming the archive path {stale}")
    failures += mismatched + stale

    differing = 0
    for name in RELOCATED:
        before = subprocess.run(["git", "show", f"{EXECUTED}:dev/{CRATE}/{name}"], check=True, capture_output=True).stdout
        differing += sha(before) != sha((ARCHIVE / CRATE / name).read_bytes())
    print(f"relocated crate files {len(RELOCATED)}: differing from dev/{CRATE} at {EXECUTED}: {differing}; "
          f"dev/{CRATE} exists: {Path('dev', CRATE).exists()}")
    failures += differing + Path("dev", CRATE).exists()

    for label, files in (("archive", sorted(ARCHIVE.rglob("*.md"))), ("repointed", [Path(p) for p in REPOINTED])):
        links, unresolved = scan(files)
        print(f"link scan {label}: files {len(files)}, local links {links}, unresolved {unresolved}")
        failures += unresolved
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
