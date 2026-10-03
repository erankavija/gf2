#!/usr/bin/env python3
"""Reconcile migration manifest rows with the executed archive of container 6dc81018.

  96cea1b9-archive-rows.py            rewrite the manifest rows, print each decision
  96cea1b9-archive-rows.py --check    print each decision; exit 1 if a row differs

The moves come from the container's `artifact_archive_executed` event in
`.jit/events.jsonl`, whose `publications` pair each source with its archive
destination; the `.jit-container` marker under the event's `destination_root`
must name the same container. The working tree decides each manifest row whose
`path` is a publication source:

- `moved`: the source is absent and the destination exists. The row takes the
  recorded destination, disposition `jit-container-archive` and status `complete`.
- `in-place`: the source exists. The row is untouched.

A publication source without a manifest row is skipped. A source and
destination both absent stop the run. The run is idempotent.
"""
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

CONTAINER = "6dc81018"
ARCHIVED = {"disposition": "jit-container-archive", "status": "complete"}


def tracked_manifest(root):
    found = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z", "--", ":(glob)**/migration/manifest.toml"],
        capture_output=True, text=True, check=True,
    ).stdout.split("\0")
    found = [f for f in found if f]
    if len(found) != 1:
        sys.exit(f"{len(found)} tracked migration manifests; exactly one must exist")
    return root / found[0]


def archive_event(root, short_id):
    events = [json.loads(line) for line in (root / ".jit/events.jsonl").read_text().splitlines() if line.strip()]
    found = [e for e in events if e.get("type") == "artifact_archive_executed" and e["target"]["id"].startswith(short_id)]
    if len(found) != 1:
        sys.exit(f"{len(found)} archive events for container {short_id}; exactly one must exist")
    event = found[0]
    marker = root / event["destination_root"] / ".jit-container"
    if not marker.is_file() or marker.read_text().strip() != event["target"]["id"]:
        sys.exit(f"{marker.relative_to(root)}: marker does not name container {event['target']['id']}")
    return event


def set_field(line, key, value):
    line, count = re.subn(rf'(, {key} = )"[^"]*"', lambda m: m.group(1) + json.dumps(value), line, count=1)
    if count != 1:
        sys.exit(f"manifest row lacks a string field {key}: {line[:120]}")
    return line


def main():
    if sys.argv[1:] not in ([], ["--check"]):
        sys.exit(__doc__)
    check = bool(sys.argv[1:])
    root = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip())
    manifest = tracked_manifest(root)
    event = archive_event(root, CONTAINER)
    rows = {r["path"]: r for r in tomllib.loads(manifest.read_text())["artifacts"]}

    def exists(p):
        return (root / p).exists() or (root / p).is_symlink()

    updates = {}
    for pub in sorted((p for p in event["publications"] if p["source"] in rows), key=lambda p: p["source"]):
        source, destination = pub["source"], pub["destination"]
        if exists(source):
            print(f"in-place {source}")
        elif exists(destination):
            print(f"moved {source} -> {destination}")
            updates[source] = {"destination": destination, **ARCHIVED}
        else:
            sys.exit(f"{source}: source and destination {destination} are both absent")

    stale = sorted(p for p, fields in updates.items() if any(rows[p][k] != v for k, v in fields.items()))
    if check:
        for path in stale:
            print(f"differs {path}")
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
