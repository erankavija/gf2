#!/usr/bin/env python3
"""List the `planned work:` entries of the sweep commits.

Usage: python3 030496bd-planned-work.py COMMIT < UNIT_IDS

UNIT_IDS is the whitespace-separated list of sweep-unit short ids on stdin.
Walks `git log COMMIT` of the repository containing the current directory and
selects the commits whose subject scope is `jit:<unit>`. In each body, a
`planned work` field runs from a line starting with those words to the next
blank line; a field with `- ` bullets holds one entry per bullet, any other
field is one entry. An entry of the form `statement -> id` names a tracked
issue.

Section 1 prints `unit | commit | statement | id` for every such entry.
Section 2 prints `unit | commit | text` for every entry without an id.
Section 3 prints each id of section 1 with the units that name it.
"""

import re
import subprocess
import sys
from collections import defaultdict

# A field ends at a blank line or at the next `name:` field header.
FIELD = re.compile(
    r"^planned work\b.*?(?=\n[ \t]*\n|\n[a-z][a-z -]*(?: \([^)\n]*\))?:|\Z)",
    re.I | re.M | re.S,
)
ENTRY = re.compile(r"^(.*?)\s*->\s*([0-9a-f]{8})\b")


def entries(field: str) -> list[str]:
    _, _, rest = field.partition(":")
    parts = re.split(r"^- ", rest, flags=re.M)
    if len(parts) > 1:
        # Text between the colon and the first bullet belongs to the header.
        parts = parts[1:]
    return [" ".join(p.split()) for p in parts]


def main() -> None:
    units = set(sys.stdin.read().split())
    log = subprocess.run(
        ["git", "log", "--format=%h%x1f%s%x1f%b%x1e", sys.argv[1]],
        check=True, capture_output=True, text=True,
    ).stdout
    named, unnamed, by_id = [], [], defaultdict(set)
    for record in log.split("\x1e"):
        if not record.strip():
            continue
        commit, subject, body = record.strip("\n").split("\x1f")
        scope = re.match(r"\w+\(jit:([0-9a-f]{8})\):", subject)
        if not scope or scope[1] not in units:
            continue
        for field in FIELD.findall(body):
            for entry in entries(field):
                m = ENTRY.match(entry)
                if m:
                    named.append(f"{scope[1]} | {commit} | {m[1]} | {m[2]}")
                    by_id[m[2]].add(scope[1])
                else:
                    unnamed.append(f"{scope[1]} | {commit} | {entry}")
    print("# 1. unit | commit | statement | id")
    print("\n".join(sorted(named)))
    print("# 2. entries without an id: unit | commit | text")
    print("\n".join(sorted(unnamed)))
    print("# 3. jit issue show <id> | units")
    for issue, us in sorted(by_id.items()):
        print(f"{issue} | {' '.join(sorted(us))}")


if __name__ == "__main__":
    main()
