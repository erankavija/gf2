#!/usr/bin/env python3
"""Local-link scan over the Markdown files the jit:103a792a commits touched.

Prints `<file>:<line>: unresolved: <target>` per inline link whose path is
missing relative to its file, then a per-file link count and a total. URL,
`mailto:` and anchor-only targets and fenced code are skipped. Exit 1 on any
unresolved link.
"""
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path

LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


root = Path(git("rev-parse", "--show-toplevel").strip())
touched = git("-C", str(root), "log", "--format=", "--name-only", "--grep", "jit:103a792a")
files = sorted({f for f in touched.split() if f.endswith(".md") and (root / f).is_file()})
unresolved = 0
for name in files:
    fenced, links = False, 0
    for number, line in enumerate((root / name).read_text().splitlines(), 1):
        if re.match(r"\s*(```|~~~)", line):
            fenced = not fenced
        if fenced:
            continue
        for target in LINK.findall(line):
            if SKIP.match(target):
                continue
            links += 1
            path = urllib.parse.unquote(target.split("#", 1)[0])
            if not (root / name).parent.joinpath(path).exists():
                unresolved += 1
                print(f"{name}:{number}: unresolved: {target}")
    print(f"scanned {name}: {links} local links")
print(f"unresolved local links: {unresolved}")
sys.exit(1 if unresolved else 0)
