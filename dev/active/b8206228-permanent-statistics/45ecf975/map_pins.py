#!/usr/bin/env python3
"""Maps pinned document commits in the tracker's issue records through a
filter-repo commit-map.

Usage: map_pins.py <commit-map>. The data root is `JIT_DATA_DIR` when set,
otherwise `.jit` under the top level of the checkout it runs in; it rewrites
`<root>/issues/*.json` in place. A pinned commit absent from the map or pruned
by the rewrite fails naming its issue record.
"""
import glob
import json
import os
import subprocess
import sys


def data_root():
    root = os.environ.get("JIT_DATA_DIR")
    if root:
        return root
    top = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    return os.path.join(top, ".jit")


mapping = dict(
    line.split() for line in open(sys.argv[1]) if not line.startswith("old")
)
mapped = 0
for path in sorted(glob.glob(os.path.join(data_root(), "issues", "*.json"))):
    text = open(path).read()
    issue = json.loads(text)
    changed = False
    for document in issue.get("documents", []):
        commit = document.get("commit")
        if not commit:
            continue
        full = next((key for key in mapping if key.startswith(commit)), None)
        if full is None:
            raise SystemExit(f"{path}: commit {commit} absent from the commit map")
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
