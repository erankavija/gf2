#!/usr/bin/env python3
"""Decide the migration manifest rows that are pending at BASE and report the row changes.

  a0ec34f2-rows.py REV       print decisions, row changes and pending rows of commit REV
  a0ec34f2-rows.py --apply   rewrite the working-tree manifest rows to the decisions at HEAD

Every fact comes from git objects of BASE and REV; the working tree and the
tracker are not read. The decision of an artifact row that is pending at BASE:

- `retained-operational`: `path` exists. The row takes an empty destination
  and status `complete`.
- `rewritten-topic`: `destination` exists and a commit scoped to an issue of
  the breakdown keymap beside this file touches it. The row takes status
  `complete`; the report names each such issue key with its first commit.
- `legacy-archive`, `jit-container-archive`: the table `a0ec34f2-archive-rows.tsv`
  beside this file gives the decision. `moved`: `path` is absent and
  `destination` holds the blob `path` has at BASE; status `complete`.
  `retained`: `path` exists; disposition `retained-operational`, empty
  destination, status `complete`; the report prints each holder line the table
  names and whether the holder file is a digest-pinned row. `held`: `path`
  exists; the row keeps its fields.
- `deletion`: the row keeps its fields.

Any other state stops the run. The report exits 1 when a row of REV differs
from its decision, a row is added or removed, a field other than FIELDS
differs, or a manifest line differs from its BASE line outside FIELDS.
"""
import collections
import csv
import io
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

BASE = "338275ff25cf19b275e844894401edef4257f007"
FIELDS = ("disposition", "destination", "status")
HERE = Path(__file__).resolve().parent
ROOT = Path(subprocess.run(["git", "-C", str(HERE), "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip())
BESIDE = HERE.relative_to(ROOT).as_posix()


def git(*args, check=True):
    return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True, check=check)


def show(rev, path):
    return git("show", f"{rev}:{path}").stdout


def blob(rev, path):
    return git("rev-parse", "--verify", "--quiet", f"{rev}:{path}", check=False).stdout.strip()


def set_field(line, key, value):
    line, count = re.subn(rf'(, {key} = )"[^"]*"', lambda m: m.group(1) + json.dumps(value), line, count=1)
    if count != 1:
        sys.exit(f"manifest row lacks a string field {key}: {line[:120]}")
    return line


def epic_commits(rev, path, keymap):
    """First commit per breakdown issue among the commits of `rev` that touch `path`."""
    first = {}
    for line in git("log", "--reverse", "--format=%h %s", rev, "--", path).stdout.splitlines():
        scope = re.match(r"(\S+) \w+\(jit:([0-9a-f]{8})\)", line)
        if scope and scope.group(2) in keymap:
            first.setdefault(scope.group(2), scope.group(1))
    return [f"{keymap[i]} {i} {sha}" for i, sha in first.items()]


def decide(rev, rows):
    """Return {path: target fields}, {path: decision label} and the report lines of the non-bulk rows."""
    keymap = {v[:8]: k for k, v in json.loads(show(rev, f"{BESIDE}/breakdown-keymap.json")).items()}
    table = {r["path"]: r for r in csv.DictReader(io.StringIO(show(rev, f"{BESIDE}/a0ec34f2-archive-rows.tsv")), delimiter="\t")}
    pinned = {p for p, r in rows.items() if r["digest_pinned"]}
    targets, labels, detail = {}, {}, []
    for path, row in sorted(rows.items()):
        if row["status"] != "pending":
            continue
        disp, dest, present = row["disposition"], row["destination"], bool(blob(rev, path))
        keep = {k: row[k] for k in FIELDS}
        if disp == "deletion":
            targets[path], labels[path] = keep, "left pending"
        elif disp == "retained-operational" and present:
            targets[path] = {**keep, "destination": "", "status": "complete"}
            labels[path] = "in place, destination cleared" if dest else "in place"
        elif disp == "rewritten-topic" and blob(rev, dest) and (commits := epic_commits(rev, dest, keymap)):
            targets[path], labels[path] = {**keep, "status": "complete"}, "rewritten"
            detail.append(f"rewritten {dest}: {'; '.join(commits)}")
        elif path in table and (entry := table[path])["decision"] == "moved" and not present and blob(rev, dest) == blob(BASE, path):
            targets[path], labels[path] = {**keep, "status": "complete"}, "moved"
            detail.append(f"moved {path} -> {dest} ({entry['ground']}, blob of BASE)")
        elif path in table and entry["decision"] in ("retained", "held") and present:
            held = entry["decision"] == "held"
            targets[path] = keep if held else {"disposition": "retained-operational", "destination": "", "status": "complete"}
            labels[path] = "left pending" if held else "retained in place"
            detail.append(f"{entry['decision']} {path} ({entry['ground']})")
            for holder in entry["holders"].split():
                name, number = holder.rsplit(":", 1)
                text = show(rev, name).splitlines()[int(number) - 1].strip()
                detail.append(f"  {holder}{' [digest-pinned row]' if name in pinned else ''}: {text[:110]}")
        else:
            sys.exit(f"{path}: no decision for a pending {disp} row")
    return targets, labels, detail


def manifest_at(rev):
    text = show(rev, f"{BESIDE}/migration/manifest.toml")
    return text, tomllib.loads(text)


def report(rev):
    rev = git("rev-parse", "--verify", f"{rev}^{{commit}}").stdout.strip()
    (base_text, base), (text, data) = manifest_at(BASE), manifest_at(rev)
    old, new = ({r["path"]: r for r in d["artifacts"]} for d in (base, data))
    targets, labels, detail = decide(rev, old)
    print(f"base {BASE}\nchecked {rev}\n\n[decisions for the artifact rows pending at base]")
    for (disp, label), count in sorted(collections.Counter((old[p]["disposition"], labels[p]) for p in targets).items()):
        print(f"{count:5d} {disp}: {label}")
    print("\n".join(detail))
    for label in ("in place, destination cleared", "left pending"):
        print(f"{label}:")
        print("\n".join(f"  {p}" for p in sorted(targets) if labels[p] == label))

    failures = [f"{p}: differs from its decision" for p, t in targets.items() if p in new and any(new[p][k] != v for k, v in t.items())]
    failures += [f"{p}: row added or removed" for p in sorted(set(old) ^ set(new))]
    changed = {p: [k for k in old[p] if old[p][k] != new[p][k]] for p in old if p in new and old[p] != new[p]}
    failures += [f"{p}: field {k} differs" for p, keys in changed.items() for k in keys if k not in FIELDS]
    print("\n[row changes base -> checked, by base disposition]")
    for disp in sorted({old[p]["disposition"] for p in changed}):
        paths = [p for p in changed if old[p]["disposition"] == disp]
        fields = collections.Counter(k for p in paths for k in changed[p])
        became = collections.Counter(new[p]["disposition"] for p in paths if "disposition" in changed[p])
        print(f"{disp}: {len(paths)} rows; " + ", ".join(f"{k} {fields[k]}" for k in FIELDS)
              + "".join(f"; disposition -> {d} {n}" for d, n in sorted(became.items())))
    lines, expected = text.splitlines(), base_text.splitlines()
    for i, line in enumerate(expected):
        head = re.match(r'\s*\{ ?path = "([^"]*)"', line)
        if head and head.group(1) in changed:
            for key in FIELDS:
                line = set_field(line, key, new[head.group(1)][key])
            expected[i] = line
    stray = sum(a != b for a, b in zip(lines, expected)) + abs(len(lines) - len(expected))
    if stray:
        failures.append(f"{stray} manifest lines differ from base outside {', '.join(FIELDS)}")
    print(f"artifact rows with another field changed: {sum(any(k not in FIELDS for k in v) for v in changed.values())}")
    print(f"artifact rows added or removed: {len(set(old) ^ set(new))}")
    print(f"manifest lines differing from base outside {', '.join(FIELDS)}: {stray}")

    print("\n[pending at checked]")
    for disp, count in sorted(collections.Counter(r["disposition"] for r in new.values() if r["status"] == "pending").items()):
        print(f"{count:5d} {disp}")
    print(f"{sum(r['status'] == 'pending' for r in data['policy']):5d} policy")
    print("\n".join(failures))
    return 1 if failures else 0


def apply():
    manifest = HERE / "migration" / "manifest.toml"
    rows = {r["path"]: r for r in tomllib.loads(show(BASE, manifest.relative_to(ROOT).as_posix()))["artifacts"]}
    targets, _, _ = decide("HEAD", rows)
    lines = manifest.read_text().splitlines(keepends=True)
    for i, line in enumerate(lines):
        head = re.match(r'\s*\{ ?path = "([^"]*)"', line)
        if head and head.group(1) in targets:
            for key, value in targets[head.group(1)].items():
                line = set_field(line, key, value)
            lines[i] = line
    manifest.write_text("".join(lines))
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(apply() if sys.argv[1] == "--apply" else report(sys.argv[1]))
