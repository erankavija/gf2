#!/usr/bin/env python3
"""Reconcile migration manifest rows with the re-archive of container 6dc81018.

  616e1d7c-rows.py            rewrite the manifest rows, print the count per decision
  616e1d7c-rows.py --check    exit 1 if a row differs from its decision

The plan is the reduced pre-execution preview `616e1d7c-preexec.json` beside
this file. A manifest row is in scope when its `epic` is the container, its
`path` is a plan source under the active area and its entry is not in
EXCLUDED. The working tree decides each row:

- `archived`: the plan moves the source, the source is absent and the
  destination exists. The row takes the plan destination, disposition
  `jit-container-archive` and status `complete`.
- `retained`: the plan copies or retains the source and the source exists. The
  row takes an empty destination, disposition `retained-operational` and
  status `complete`.

Any other state of an in-scope row stops the run. The run is idempotent.
"""
import collections
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

CONTAINER = "6dc81018"
EXCLUDED = ("a83583e0", "dbd8787d")  # code-pinned entries with their own move issues


def set_field(line, key, value):
    line, count = re.subn(rf'(, {key} = )"[^"]*"', lambda m: m.group(1) + json.dumps(value), line, count=1)
    if count != 1:
        sys.exit(f"manifest row lacks a string field {key}: {line[:120]}")
    return line


def main():
    if sys.argv[1:] not in ([], ["--check"]):
        sys.exit(__doc__)
    root = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip())
    here = Path(__file__).resolve().parent
    active = here.parent.relative_to(root).as_posix() + "/"
    plan = {a["source"]: a for a in json.loads((here / "616e1d7c-preexec.json").read_text())["artifacts"]
            if a["action"] != "retain" or "pinned-historical" in a["evidence"]}
    manifest = here / "migration" / "manifest.toml"
    rows = {r["path"]: r for r in tomllib.loads(manifest.read_text())["artifacts"]}

    updates, decisions = {}, collections.Counter()
    for path, row in sorted(rows.items()):
        if row["epic"] != CONTAINER or path not in plan or not path.startswith(active):
            continue
        if path[len(active):].split("/", 1)[0] in EXCLUDED:
            continue
        artifact, present = plan[path], (root / path).exists()
        if artifact["action"] == "move" and not present and (root / artifact["destination"]).exists():
            decision = "archived"
            updates[path] = {"destination": artifact["destination"], "disposition": "jit-container-archive", "status": "complete"}
        elif artifact["action"] in ("copy", "retain") and present:
            decision = "retained"
            updates[path] = {"destination": "", "disposition": "retained-operational", "status": "complete"}
        else:
            sys.exit(f"{path}: plan action {artifact['action']} does not match the working tree")
        decisions[f"{decision} ({path[len(active):].split('/', 1)[0]})"] += 1

    stale = sorted(p for p, fields in updates.items() if any(rows[p][k] != v for k, v in fields.items()))
    for key, count in sorted(decisions.items()):
        print(f"{count:4d} {key}")
    print(f"{len(stale)} rows differ from their decision")
    if sys.argv[1:]:
        return 1 if stale else 0
    lines = manifest.read_text().splitlines(keepends=True)
    for i, line in enumerate(lines):
        head = re.match(r'\s*\{ ?path = "([^"]*)"', line)
        if head and head.group(1) in stale:
            for key, value in updates[head.group(1)].items():
                line = set_field(line, key, value)
            lines[i] = line
    manifest.write_text("".join(lines))
    rows = {r["path"]: r for r in tomllib.loads(manifest.read_text())["artifacts"]}
    if missed := [p for p in stale if any(rows[p][k] != v for k, v in updates[p].items())]:
        sys.exit(f"manifest rows not rewritten: {', '.join(missed)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
