#!/usr/bin/env python3
"""Per-crate comment census of tracked Rust sources under `crates/`.

A comment line is a line whose first non-whitespace characters are `//`
(covers `//`, `///`, `//!`); block comments and trailing comments are not
counted. A non-blank line contains a non-whitespace character. Share is
comment lines over non-blank lines. The sweep pattern is a Python `re`
expression applied case-insensitively to comment lines only.

Usage: python3 62f0d0e6-comment-census.py MATCHES_FILE
Writes the table to stdout and the matching `path:line:text` lines to
MATCHES_FILE. Files come from `git ls-files 'crates/**/*.rs'` and are read
from the working tree of the repository containing the current directory.
"""

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

PATTERN = (
    r"non-goal|out of scope|not in scope|future work|deferred to|follow-on"
    r"|phase [a-e]\b|wave [a-z0-9]|(task|issue|story) `?[0-9a-f]{8}"
    r"|previously|no longer|legacy|migrat"
)
SWEEP = re.compile(PATTERN, re.IGNORECASE)


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout


def main() -> None:
    root = Path(git("rev-parse", "--show-toplevel").strip())
    commit = git("rev-parse", "HEAD").strip()
    files = sorted(git("ls-files", "-z", "crates/**/*.rs").split("\0")[:-1])
    stats = defaultdict(lambda: [0, 0, 0])  # comment, non-blank, pattern
    matches = []
    for rel in files:
        crate = rel.split("/")[1]
        text = (root / rel).read_text(encoding="utf-8", errors="replace")
        for n, line in enumerate(text.split("\n"), 1):
            s = line.strip()
            if not s:
                continue
            stats[crate][1] += 1
            if s.startswith("//"):
                stats[crate][0] += 1
                if SWEEP.search(line):
                    stats[crate][2] += 1
                    matches.append(f"{rel}:{n}:{line.rstrip()}")
    rows = [(f"crates/{c}", *v) for c, v in sorted(stats.items())]
    rows.append(("total", *(sum(r[i] for r in rows) for i in (1, 2, 3))))
    print(f"commit: {commit}")
    print(f"files: {len(files)}")
    print(f"{'crate':<26}{'comment':>9}{'non-blank':>11}{'share':>8}{'pattern':>9}")
    for name, c, nb, p in rows:
        print(f"{name:<26}{c:>9}{nb:>11}{100 * c / nb:>7.2f}%{p:>9}")
    Path(sys.argv[1]).write_text("".join(m + "\n" for m in matches), encoding="utf-8")


if __name__ == "__main__":
    main()
