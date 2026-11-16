#!/usr/bin/env python3
"""Maps pinned document commits in the tracker's issue records through a
filter-repo commit-map.

Usage: map_pins.py <commit-map> <original-git-dir>. The data root is
`JIT_DATA_DIR` when set, otherwise `.jit` under the top level of the checkout
it runs in; it rewrites `<root>/issues/*.json` in place. A pinned commit that
the rewrite pruned (all-zero map entry) maps to the rewritten commit of its
nearest first-parent ancestor with a nonzero entry, found in the pre-rewrite
history at <original-git-dir>; that commit's tree equals the pruned one outside
the dropped paths. A pinned commit absent from the map, a pinned path that `drop()`
of rewrite_paths.py removes, or a pruned commit without a kept ancestor fails
naming its issue record.
"""
import glob
import gzip
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


rules = {}
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                       "rewrite_paths.py")).read(), rules)


def load_map(path):
    opener = gzip.open if str(path).endswith(".gz") else open
    with opener(path, "rt") as source:
        return dict(line.split() for line in source if not line.startswith("old"))


def kept_ancestor(commit, original, mapping):
    while True:
        parent = subprocess.run(
            ["git", "--git-dir", original, "rev-parse", commit + "^"],
            capture_output=True, text=True,
        )
        if parent.returncode:
            return None
        commit = parent.stdout.strip()
        new = mapping.get(commit)
        if new is not None and set(new) != {"0"}:
            return new


def main():
    original = sys.argv[2]
    mapping = load_map(sys.argv[1])
    mapped = through = 0
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
            where = document.get("path", "")
            if rules["drop"](where.encode()):
                raise SystemExit(f"{path}: pinned path {where} is dropped")
            new = mapping[full]
            if set(new) == {"0"}:
                new = kept_ancestor(full, original, mapping)
                if new is None:
                    raise SystemExit(f"{path}: pruned commit {commit} has no kept ancestor")
                through += 1
            document["commit"] = new
            changed = True
            mapped += 1
        if changed:
            ending = "\n" if text.endswith("\n") else ""
            open(path, "w").write(
                json.dumps(issue, indent=2, ensure_ascii=False) + ending
            )
    print(f"mapped {mapped} pinned commits: {mapped - through} directly, "
          f"{through} through a pruned commit")


if __name__ == "__main__":
    main()
