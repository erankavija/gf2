#!/usr/bin/env python3
"""Verification of the executed re-archive of container 97bf0879.

  83f9ce69-rearchive.py    print the report; exit 1 on any failed check

`83f9ce69-preexec-plan.json` holds the fields this script reads from the
`jit archive container 97bf0879 --json` preview taken on main immediately
before execution. Run from any directory of the checkout; needs `jit` on PATH.

Sections:
- marker: `.jit-container` under the destination root names the container.
- hashes: each planned destination has the planned sha256 and byte size, and
  each planned move source is absent.
- references: each planned reference change names its `to_path` in the tracker
  (`jit issue show <issue> --json`).
- rerun: the current preview lists no unarchived publication, present move
  source, deletion, blocker or reference change outside the verified set.
- links: each inline Markdown link and HTML `href`/`src` under the destination
  root resolves; each `dev/archive/...` path token in the repointed files
  resolves.
  URL targets, anchors and fenced Markdown code are skipped.
- manifest: no migration manifest row of epic 97bf0879 has status `pending`.
"""
import hashlib
import json
import re
import subprocess
import sys
import tomllib
import urllib.parse
from pathlib import Path

REPOINTED = (
    "benchmarks/analyze.py",
    "benchmarks/reference/fflas_sparse_bench.cpp",
    "benchmarks/reference/linbox_sparse_bench.cpp",
    "benchmarks/reference/ntl_bench.cpp",
    "benchmarks/reference/sparse_smoke.cpp",
    "dev/bench_results/run_41096af5_post_wire_in_bench.sh",
)
LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
HREF = re.compile(r"""(?:href|src)\s*=\s*["']([^"']+)["']""")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")
ARCHIVE_TOKEN = re.compile(r"dev/archive/[A-Za-z0-9_./-]*[A-Za-z0-9_-]")


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


root = Path(run("git", "rev-parse", "--show-toplevel").strip())
plan = json.loads(Path(__file__).with_name("83f9ce69-preexec-plan.json").read_text())
failures = 0


def fail(text):
    global failures
    failures += 1
    print(f"  FAIL {text}")


def jit(*args):
    return json.loads(subprocess.run(["jit", *args, "--json"], cwd=root, check=True, capture_output=True, text=True).stdout)


print("marker")
marker = root / plan["destination_root"] / ".jit-container"
print(f"  {marker.relative_to(root)}: {marker.read_text().strip()}")
if marker.read_text().strip() != plan["target"]:
    fail("marker does not name the container")

print("hashes")
published = [a for a in plan["artifacts"] if a["action"] in ("move", "copy")]
matched = 0
for a in published:
    data = (root / a["destination"]).read_bytes() if (root / a["destination"]).is_file() else None
    want = a["content_identity"]
    if data is None or hashlib.sha256(data).hexdigest() != want["sha256"] or len(data) != want["byte_size"]:
        fail(f"{a['destination']}: differs from the planned content identity")
    else:
        matched += 1
    if a["action"] == "move" and (root / a["source"]).exists():
        fail(f"{a['source']}: move source is present")
print(f"  destinations matching planned sha256 and size: {matched} of {len(published)}")
print(f"  move sources absent: {sum(not (root / a['source']).exists() for a in published if a['action'] == 'move')}"
      f" of {sum(a['action'] == 'move' for a in published)}")

print("references")
changes = [c for a in plan["artifacts"] for c in a["reference_changes"]]
documents = {issue: jit("issue", "show", issue)["documents"] for issue in sorted({c["issue"] for c in changes})}
named = 0
for c in changes:
    if documents[c["issue"]][c["document_index"]]["path"] == c["to_path"]:
        named += 1
    else:
        fail(f"{c['issue'][:8]} document {c['document_index']}: does not name {c['to_path']}")
print(f"  tracker references naming the archive path: {named} of {len(changes)} on {len(documents)} issues")

print("rerun")
now = jit("archive", "container", plan["target"][:8])
moves = [a for a in now["artifacts"] if a["action"] in ("move", "copy")]
unpublished = [a["source"] for a in moves if not a["already_archived"]]
present = [a["source"] for a in moves if a["action"] == "move" and (root / a["source"]).exists()]
deletions = [d["source"] for a in now["artifacts"] for d in a["pending_deletions"]]
extra = [c for a in now["artifacts"] for c in a["reference_changes"] if c not in changes]
print(f"  eligible: {now['eligible']}; blockers: {len(now['blockers'])}; action_counts: {json.dumps(now['action_counts'], sort_keys=True)}")
print(f"  planned publications not yet archived: {len(unpublished)}")
print(f"  move sources present: {len(present)}")
print(f"  pending deletions: {len(deletions)}")
print(f"  listed reference changes outside the verified set: {len(extra)}")
if not now["eligible"] or now["blockers"] or unpublished or present or deletions or extra:
    fail("the rerun plans a change")

print("links")
total = 0
for path in sorted((root / plan["destination_root"]).rglob("*")):
    if path.suffix not in (".md", ".html") or not path.is_file():
        continue
    fenced = False
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if path.suffix == ".md":
            if re.match(r"\s*(```|~~~)", line):
                fenced = not fenced
            if fenced:
                continue
        for target in (LINK if path.suffix == ".md" else HREF).findall(line):
            if SKIP.match(target):
                continue
            total += 1
            local = urllib.parse.unquote(target.split("#", 1)[0])
            resolved = root / local.lstrip("/") if local.startswith("/") else path.parent / local
            if not resolved.exists():
                fail(f"{path.relative_to(root)}:{number}: unresolved: {target}")
print(f"  {plan['destination_root']}: {total} local links in Markdown and HTML files")
for name in REPOINTED:
    tokens = [(n, t) for n, line in enumerate((root / name).read_text().splitlines(), 1) for t in ARCHIVE_TOKEN.findall(line)]
    for number, token in tokens:
        if not (root / token).exists():
            fail(f"{name}:{number}: unresolved: {token}")
    print(f"  {name}: {len(tokens)} dev/archive path tokens")

print("manifest")
rows = [r for r in tomllib.loads(Path(__file__).with_name("migration").joinpath("manifest.toml").read_text())["artifacts"]
        if r["epic"] == plan["target"][:8]]
open_rows = [r["path"] for r in rows if r["status"] != "complete"]
for path in open_rows:
    fail(f"{path}: row is not complete")
print(f"  rows of epic {plan['target'][:8]} with status complete: {len(rows) - len(open_rows)} of {len(rows)}")

print(f"result: {'FAIL' if failures else 'PASS'} ({failures} failed checks)")
sys.exit(1 if failures else 0)
