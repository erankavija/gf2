#!/usr/bin/env python3
"""List the comment read of issue fa4939c3 and check it against the tree.

Usage: python3 -B fa4939c3-comment-read.py > fa4939c3-comment-read.txt

Reads `fa4939c3-hits.tsv`, `554c2935-citation-map.py` and its `.txt` listing
beside this script, the `*-comment-census.txt` beside it that names
CENSUS_COMMIT, and the tracked Rust files of the repository containing the
current directory. File selection and the comment-line definition are those
of the citation map script, loaded as a module.

Prints the commit read and the object ids of the listed trees; then
`path | lines read | lines beginning with //` per file and per crate; then
`path:line | disposition | key or reason | found by | comment` per hit; then
the hit counts.

Exits with status 1 when
- a hit's line holds no comment,
- a `keyed` or `new work` hit is absent from sections 1 and 5 of the map
  listing under one of its keys,
- a `not a citation` hit, or a line outside the hits file, is listed in
  section 1, 2 or 5,
- the `crates` tree of CENSUS_COMMIT differs from that of READ_COMMIT, or
- a per-crate total of lines beginning with `//` differs from the census.
"""

import csv
import importlib.util
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

READ_COMMIT = "04c00d0ba3116652526ba51728390b427fdcad8f"
CENSUS_COMMIT = "fa29c6a3976e162d8994dfc19ea85c90df848f7c"
HERE = Path(__file__).resolve().parent
MAP_STEM = "554c2935-citation-map"


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout


def load_map_script():
    spec = importlib.util.spec_from_file_location("citation_map", HERE / f"{MAP_STEM}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def map_entries() -> dict[tuple[str, int], set[tuple[str, str]]]:
    """`(path, line) -> {(section, key or reason)}` for sections 1, 2 and 5."""
    entries = defaultdict(set)
    section = ""
    for line in (HERE / f"{MAP_STEM}.txt").read_text(encoding="utf-8").split("\n"):
        if line.startswith("# "):
            section = line[2]
            continue
        if section not in "125" or not line:
            continue
        fields = line.split(" | ")
        if fields[-1] == "-":
            continue
        for token in fields[-1].split(" "):
            path, numbers = token.rsplit(":", 1)
            for n in numbers.split(","):
                entries[(path, int(n))].add((section, fields[0]))
    return entries


def census() -> dict[str, int]:
    """Per-crate comment counts of the census that names CENSUS_COMMIT."""
    for path in sorted(HERE.glob("*-comment-census.txt")):
        lines = path.read_text(encoding="utf-8").split("\n")
        if lines[0] == f"commit: {CENSUS_COMMIT}":
            return {
                f[0]: int(f[1])
                for f in (l.split() for l in lines)
                if f and f[0].startswith("crates/")
            }
    raise SystemExit(f"error: no census beside the script names {CENSUS_COMMIT}")


def main() -> int:
    sys.dont_write_bytecode = True
    map_script = load_map_script()
    root = Path(git("rev-parse", "--show-toplevel").strip())
    files = sorted(
        git(
            "-C", str(root), "ls-files", "-z", "crates/**/*.rs", "dev/tools/**/*.rs"
        ).split("\0")[:-1]
    )
    errors = []

    print(f"commit read: {READ_COMMIT}")
    for tree in ("crates", "dev/tools"):
        ids = [
            git("-C", str(root), "rev-parse", f"{rev}:{tree}").strip()
            for rev in (READ_COMMIT, "HEAD")
        ]
        print(f"tree {tree}: read {ids[0]} listed {ids[1]}")
    census_tree, read_tree = (
        git("-C", str(root), "rev-parse", f"{rev}:crates").strip()
        for rev in (CENSUS_COMMIT, READ_COMMIT)
    )
    if census_tree != read_tree:
        errors.append(f"crates tree of census commit {CENSUS_COMMIT} differs from the commit read")

    comments = {}
    totals = defaultdict(lambda: [0, 0, 0])  # files, lines read, `//` lines
    print()
    print("# 1. path | lines read | lines beginning with //")
    for rel in files:
        text = (root / rel).read_text(encoding="utf-8", errors="replace")
        read = slashes = 0
        for n, line in enumerate(text.split("\n"), 1):
            comment = map_script.comment_of(line)
            if comment is not None:
                comments[(rel, n)] = comment
                read += 1
            slashes += line.strip().startswith("//")
        print(f"{rel} | {read} | {slashes}")
        parts = rel.split("/")
        crate = "/".join(parts[:2] if parts[0] == "crates" else parts[:3])
        for i, v in enumerate((1, read, slashes)):
            totals[crate][i] += v

    print()
    print("# 2. crate | files | lines read | lines beginning with // | census")
    counted = census()
    for crate, (n_files, read, slashes) in sorted(totals.items()):
        expected = counted.get(crate)
        print(f"{crate} | {n_files} | {read} | {slashes} | {'-' if expected is None else expected}")
        if crate.startswith("crates/") and expected != slashes:
            errors.append(f"{crate}: {slashes} lines begin with //, census has {expected}")
    for crate in sorted(set(counted) - set(totals)):
        errors.append(f"{crate}: in the census, absent from the tree")
    sums = [sum(t[i] for t in totals.values()) for i in range(3)]
    print(f"total | {sums[0]} | {sums[1]} | {sums[2]} | -")

    listed = map_entries()
    with (HERE / "fa4939c3-hits.tsv").open(encoding="utf-8", newline="") as f:
        hits = list(csv.DictReader(f, delimiter="\t"))
    print()
    print("# 3. path:line | disposition | key or reason | found by | comment")
    seen = set()
    for hit in hits:
        where = (hit["path"], int(hit["line"]))
        seen.add(where)
        name = f"{where[0]}:{where[1]}"
        comment = comments.get(where)
        if comment is None:
            errors.append(f"{name}: no comment on the line")
        entry_keys = {key for _, key in listed.get(where, ())}
        if hit["disposition"] == "not a citation":
            if entry_keys:
                errors.append(f"{name}: not a citation, yet the map lists it")
        else:
            for key in hit["key or reason"].split(","):
                if ("1", key) not in listed.get(where, ()) and ("5", key) not in listed.get(where, ()):
                    errors.append(f"{name}: the map lists no {key} entry on the line")
        print(
            f"{name} | {hit['disposition']} | {hit['key or reason']} | {hit['found by']}"
            f" | {comment}"
        )
    for path, n in sorted(set(listed) - seen):
        errors.append(f"{path}:{n}: listed by the map, absent from the hits file")

    print()
    print("# 4. hit counts: disposition | found by | key or reason")
    by_class = Counter((h["disposition"], h["found by"]) for h in hits)
    for (disposition, found), count in sorted(by_class.items()):
        print(f"{count:5d}  {disposition} | {found}")
    by_reason = Counter(
        h["key or reason"] for h in hits if h["disposition"] == "not a citation"
    )
    for reason, count in sorted(by_reason.items()):
        print(f"{count:5d}  not a citation | {reason}")
    print(f"{len(hits):5d}  total")

    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
