#!/usr/bin/env python3
"""List the non-comment hunks of the sweep commits.

Usage: python3 030496bd-token-diff.py COMMIT < UNIT_IDS

UNIT_IDS is the whitespace-separated list of sweep-unit short ids on stdin.
Walks `git log COMMIT` of the repository containing the current directory and
selects the commits whose subject scope is `jit:<unit>`. For each file such a
commit changes against its first parent (for a merge: each file that differs
from every parent), the Rust token sequences of both sides are compared after
comments and whitespace are dropped; string and character literals are tokens.

Prints `unit | commit | path` for every file whose token sequences differ,
followed by one `@ line` per hunk (line in the commit's version) with the
removed (`-`) and added (`+`) tokens. A changed file that is not `.rs` prints
`unit | commit | path | not Rust source` and is not tokenized. The last line
counts the commits and Rust file versions compared.
"""

import difflib
import re
import subprocess
import sys

TOKEN = re.compile(
    r"""(?P<line>//[^\n]*)
      | (?P<block>/\*)
      | \b(?:br|cr|r)(?P<h>\#*)".*?"(?P=h)
      | (?:\b[bc])?"(?:\\.|[^"\\])*"
      | b?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[^}]*\}|.)|[^'\\\n])'
      | '\w+
      | \w+
      | \S""",
    re.S | re.X,
)
NEST = re.compile(r"/\*|\*/")


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True, errors="replace"
    ).stdout


def tokens(src: str) -> list[tuple[str, int]]:
    out, pos, line = [], 0, 1
    while m := TOKEN.search(src, pos):
        line += src.count("\n", pos, m.start())
        pos = m.end()
        if m["block"]:
            depth = 1  # Rust block comments nest.
            while depth and (n := NEST.search(src, pos)):
                depth += 1 if n[0] == "/*" else -1
                pos = n.end()
        elif not m["line"]:
            out.append((m[0], line))
        line += src.count("\n", m.start(), pos)
    return out


def blob(rev: str, path: str) -> str:
    r = subprocess.run(
        ["git", "show", f"{rev}:{path}"], capture_output=True, text=True, errors="replace"
    )
    return r.stdout if r.returncode == 0 else ""


def main() -> None:
    units = set(sys.stdin.read().split())
    commits = files = 0
    for row in git("log", "--format=%h %s", sys.argv[1]).splitlines():
        commit, subject = row.split(" ", 1)
        scope = re.match(r"\w+\(jit:([0-9a-f]{8})\):", subject)
        if not scope or scope[1] not in units:
            continue
        commits += 1
        paths = git("diff-tree", "-r", "-c", "--no-commit-id", "--name-only", commit)
        for path in paths.splitlines():
            head = f"{scope[1]} | {commit} | {path}"
            if not path.endswith(".rs"):
                print(f"{head} | not Rust source")
                continue
            files += 1
            old, new = tokens(blob(f"{commit}^", path)), tokens(blob(commit, path))
            a, b = [t for t, _ in old], [t for t, _ in new]
            if a == b:
                continue
            print(head)
            sm = difflib.SequenceMatcher(None, a, b, autojunk=False)
            for tag, i1, i2, j1, j2 in sm.get_opcodes():
                if tag == "equal":
                    continue
                line = new[min(j1, len(new) - 1)][1] if new else 0
                print(f"  @ line {line}")
                if i2 > i1:
                    print("  - " + " ".join(a[i1:i2]))
                if j2 > j1:
                    print("  + " + " ".join(b[j1:j2]))
    print(f"compared: {commits} commits, {files} Rust file versions")


if __name__ == "__main__":
    main()
