#!/usr/bin/env python3
"""Documentation migration progress checker for epic fa787f85.

Migration tool with a tracked removal: issue af84619e deletes this checker and
its self-tests after the final migration check, and keeps `manifest.toml` as an
epic record. Lasting enforcement belongs to invariants and review gates.

Run from the repository root:
    python3 dev/active/fa787f85-documentation-overhaul/migration/check.py [--require-complete]
Self-tests:
    python3 -m unittest discover -s dev/active/fa787f85-documentation-overhaul/migration -p '*_test.py'

The manifest holds two arrays of inline tables, one row per line, sorted by
their first field. `ARTIFACT_FIELDS` and `POLICY_FIELDS` define the row schema.
Row assertions against the working tree:
- pending artifact: `path` exists.
- complete deletion: `path` is absent.
- complete artifact with an empty destination or one equal to `path`: `path`
  exists; with a differing destination: `destination` exists, `path` is absent.
- pending policy entry: `value` is listed under the dotted `key` of
  `.jit/config.toml`; complete: it is absent.

Prints `<row>: <finding>` lines, then completeness per disposition and for
policy entries. Exit status 1 on any schema violation or failed assertion, or
with `--require-complete` on a manifest without artifact rows or with any
pending row; 0 otherwise.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

DISPOSITIONS = (
    "jit-container-archive",
    "legacy-archive",
    "retained-operational",
    "deletion",
    "rewritten-topic",
)
EVIDENCE = ("doc-ref", "issue-id", "commit", "none")  # D-24 admissible kinds
STATUSES = ("pending", "complete")
LEGACY_ROOT = "dev/archive/legacy/"

ARTIFACT_FIELDS = {
    "path": "repository-relative source path of the pre-overhaul artifact",
    "evidence": "association evidence kind from EVIDENCE",
    "owner": "8-hex owning issue short id; empty exactly when evidence is none",
    "bundle": "path of the bundle head row this artifact travels with; empty for a head",
    "disposition": "one of DISPOSITIONS",
    "destination": "target path; empty for deletion and for in-place retention",
    "status": "one of STATUSES",
}
POLICY_FIELDS = {
    "key": "dotted key into .jit/config.toml naming a string list",
    "value": "temporary list entry",
    "removal": "8-hex short id of the issue that removes the entry",
    "status": "one of STATUSES; complete means the entry is removed",
}
SHORT_ID = re.compile(r"[0-9a-f]{8}")


def bad_path(p: str) -> bool:
    return not p or p.startswith("/") or ".." in Path(p).parts or p != p.strip("/")


def schema_rows(rows: object, fields: dict[str, str], name: str, out: list[str]) -> list[dict]:
    if not isinstance(rows, list):
        out.append(f"{name}: not an array")
        return []
    valid, first = [], next(iter(fields))
    for i, row in enumerate(rows):
        label = f"{name}[{i}]"
        if not isinstance(row, dict) or set(row) != set(fields):
            out.append(f"{label}: fields must be exactly {', '.join(fields)}")
        elif not all(isinstance(v, str) for v in row.values()):
            out.append(f"{label}: every field must be a string")
        else:
            valid.append(row)
    keys = [r[first] for r in valid]
    if keys != sorted(keys) or len(set(keys)) != len(keys):
        out.append(f"{name}: rows must be unique and sorted by {first}")
    return valid


def check_artifact(row: dict, heads: set[str], out: list[str]) -> None:
    path, dest, disp = row["path"], row["destination"], row["disposition"]
    errs = []
    if bad_path(path):
        errs.append("path must be repository-relative")
    if row["evidence"] not in EVIDENCE:
        errs.append(f"evidence must be one of {', '.join(EVIDENCE)}")
    elif (row["evidence"] == "none") != (row["owner"] == ""):
        errs.append("owner must be empty exactly when evidence is none")
    if row["owner"] and not SHORT_ID.fullmatch(row["owner"]):
        errs.append("owner must be an 8-hex short id")
    if row["bundle"] and row["bundle"] not in heads:
        errs.append("bundle must name a head row")
    if row["status"] not in STATUSES:
        errs.append(f"status must be one of {', '.join(STATUSES)}")
    if disp not in DISPOSITIONS:
        errs.append(f"disposition must be one of {', '.join(DISPOSITIONS)}")
    elif disp == "deletion" and dest:
        errs.append("deletion takes no destination")
    elif disp == "legacy-archive" and dest != LEGACY_ROOT + path:
        errs.append(f"legacy-archive destination must be {LEGACY_ROOT}{path}")
    elif disp == "jit-container-archive" and not dest.startswith("dev/archive/"):
        errs.append("jit-container-archive destination must be under dev/archive/")
    elif disp == "rewritten-topic" and not dest:
        errs.append("rewritten-topic needs a destination")
    if dest and bad_path(dest):
        errs.append("destination must be repository-relative")
    out.extend(f"{path}: {e}" for e in errs)


def assert_artifact(row: dict, root: Path, out: list[str]) -> None:
    path, dest = row["path"], row["destination"]

    def exists(p: str) -> bool:
        return (root / p).exists() or (root / p).is_symlink()

    if row["status"] == "pending":
        if not exists(path):
            out.append(f"{path}: pending source is missing")
        return
    final = "" if row["disposition"] == "deletion" else dest or path
    if final and not exists(final):
        out.append(f"{path}: complete location {final} is missing")
    if final != path and exists(path):
        out.append(f"{path}: complete source still exists")


def assert_policy(row: dict, config: dict, out: list[str]) -> None:
    label = f"policy {row['key']}={row['value']}"
    if not SHORT_ID.fullmatch(row["removal"]) or row["status"] not in STATUSES:
        out.append(f"{label}: removal must be an 8-hex short id and status one of {', '.join(STATUSES)}")
        return
    node: object = config
    for part in row["key"].split("."):
        node = node.get(part) if isinstance(node, dict) else None
    listed = isinstance(node, list) and row["value"] in node
    if listed != (row["status"] == "pending"):
        out.append(f"{label}: {row['status']} entry is {'present' if listed else 'absent'} in .jit/config.toml")


def check(manifest: Path, root: Path) -> tuple[list[str], dict[str, list[int]]]:
    """Return findings and [complete, total] counts per disposition and for policy."""
    out: list[str] = []
    try:
        data = tomllib.loads(manifest.read_text())
        config = tomllib.loads((root / ".jit/config.toml").read_text())
    except (OSError, tomllib.TOMLDecodeError) as exc:
        return [f"{manifest}: {exc}"], {}
    if set(data) != {"artifacts", "policy"}:
        out.append(f"{manifest}: top level must be exactly artifacts and policy")
    artifacts = schema_rows(data.get("artifacts", []), ARTIFACT_FIELDS, "artifacts", out)
    policy = schema_rows(data.get("policy", []), POLICY_FIELDS, "policy", out)
    heads = {r["path"] for r in artifacts if not r["bundle"]}
    counts = {d: [0, 0] for d in DISPOSITIONS}
    for row in artifacts:
        before = len(out)
        check_artifact(row, heads, out)
        if len(out) == before:
            assert_artifact(row, root, out)
            counts[row["disposition"]][0] += row["status"] == "complete"
            counts[row["disposition"]][1] += 1
    counts["policy"] = [sum(r["status"] == "complete" for r in policy), len(policy)]
    for row in policy:
        assert_policy(row, config, out)
    return out, counts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--manifest", type=Path, default=Path(__file__).with_name("manifest.toml"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--require-complete", action="store_true")
    args = parser.parse_args()
    findings, counts = check(args.manifest, args.root)
    for line in findings:
        print(line)
    for disp, (done, total) in counts.items():
        print(f"{disp}: {done}/{total} complete")
    rows = sum(counts.get(d, [0, 0])[1] for d in DISPOSITIONS)
    incomplete = not rows or any(done < total for done, total in counts.values())
    return 1 if findings or (args.require_complete and incomplete) else 0


if __name__ == "__main__":
    sys.exit(main())
