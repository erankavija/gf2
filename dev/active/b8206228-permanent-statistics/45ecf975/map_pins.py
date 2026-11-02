#!/usr/bin/env python3
"""Maps pinned document commits in .jit/issues through a filter-repo commit-map.

Usage: map_pins.py <commit-map>; run from the rewritten checkout's root.
"""
import glob
import json
import sys

mapping = dict(
    line.split() for line in open(sys.argv[1]) if not line.startswith("old")
)
mapped = 0
for path in glob.glob(".jit/issues/*.json"):
    text = open(path).read()
    issue = json.loads(text)
    changed = False
    for document in issue.get("documents", []):
        commit = document.get("commit")
        if not commit:
            continue
        full = next(key for key in mapping if key.startswith(commit))
        if set(mapping[full]) == {"0"}:
            raise SystemExit(f"{path}: commit {commit} pruned by the rewrite")
        document["commit"] = mapping[full]
        changed = True
        mapped += 1
    if changed:
        ending = "\n" if text.endswith("\n") else ""
        open(path, "w").write(
            json.dumps(issue, indent=2, ensure_ascii=False) + ending
        )
print(f"mapped {mapped} pinned commits")
