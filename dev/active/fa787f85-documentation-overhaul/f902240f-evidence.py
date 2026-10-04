#!/usr/bin/env python3
"""Hash, state, preview and link-scan figures for the b7157be6 container archive.

Run from any directory of the checkout. Paths come from metadata: the
execution result beside this file (publications, deleted sources), the live
archive preview (destination root, pinned references), the container marker and
`git log --grep`. The tree before the execution commit (`EXEC^`) is compared
with the working tree, and inline local links are scanned in the Markdown files
that commits tagged `jit:f902240f` touch, in the record beside this file and in
the archive directory. URL, `mailto:` and anchor-only targets and fenced code
are skipped. Exit 1 on any failed figure.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path

ISSUE = "f902240f"
EXEC = "7335791e4dbb83e123d2e3c794e92b0b6344fe9e"
DATASET_OWNER = "cef1ae5f"
LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")


def run(*args, text=True):
    return subprocess.run(args, check=True, capture_output=True, text=text, cwd=ROOT).stdout


ROOT = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
                           cwd=Path(__file__).parent).stdout.strip())
RESULT = json.loads(Path(__file__).with_name(f"{ISSUE}-exec.json").read_text())
RECORD = Path(__file__).with_name(f"{ISSUE}-osd-archive.md").relative_to(ROOT).as_posix()
EPIC = RESULT["target"]["id"]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def at(rev, path):
    return sha(run("git", "show", f"{rev}:{path}", text=False))


def now(path):
    return sha((ROOT / path).read_bytes()) if (ROOT / path).is_file() else "absent"


def listed(rev, path):
    return run("git", "ls-tree", "-r", "--name-only", rev, "--", path).split()


failed = 0


def figure(label, ok, detail):
    global failed
    failed += not ok
    print(f"{'ok  ' if ok else 'FAIL'} {label}: {detail}")


plan = json.loads(run("jit", "archive", "container", EPIC, "--json"))
archive = plan["destination_root"]
marked = sorted(m.parent.relative_to(ROOT).as_posix() for m in (ROOT / archive).parent.glob("*/.jit-container")
                if m.read_text() == EPIC + "\n")
published = RESULT["publications"]
deleted = set(RESULT["deleted_sources"])
hand = [p["destination"] for p in published if p["adopted"]]
copies = [p for p in published if p["source"] and not p["adopted"] and p["source"] not in deleted]
moves = [p for p in published if p["source"] in deleted]

kept = [p for p in hand if at(f"{EXEC}^", p) == at(EXEC, p)]
figure("REQ-01 hand archive across the execution commit", len(kept) == len(hand) == len(listed(f"{EXEC}^", archive)),
       f"{len(kept)}/{len(hand)} adopted files byte-identical")
changed = sorted(p for p in hand if at(f"{EXEC}^", p) != now(p))
figure("REQ-01 hand archive in the working tree", all((ROOT / p).is_file() for p in hand), f"{len(hand)} present, changed since: {changed}")

figure("REQ-02 marker", marked == [archive], f"{EPIC} marks {marked}")
state = json.loads(run("jit", "issue", "show", EPIC, "--json"))
state = state.get("data", state)["state"]
figure("REQ-02 epic state", state == "archived", state)

for p in copies:
    same = now(p["source"]) == now(p["destination"]) == at(f"{EXEC}^", p["source"]) == p["content_identity"]["sha256"]
    figure("REQ-03 copy: live == archive == before", same, f"{p['source']} {now(p['destination'])}")
for p in moves:
    same = at(f"{EXEC}^", p["source"]) == now(p["destination"]) and now(p["source"]) == "absent"
    figure("move: archive == before, source absent", same, f"{p['source']} {now(p['destination'])}")

pins = {a["source"]: a for a in plan["artifacts"] if a["version"] != "working-tree"}
dataset = os.path.commonpath([p for p, a in pins.items() if any(o["issue"].startswith(DATASET_OWNER) for o in a["owners"])])
data = listed(f"{EXEC}^", dataset)
changed = sorted(p for p in data if at(f"{EXEC}^", p) != now(p))
figure("REQ-04 dataset", all((ROOT / p).is_file() for p in data), f"{len(data)} files present under {dataset}, changed since: {changed}")

todo = [a["source"] for a in plan["artifacts"] if a["action"] in ("move", "copy", "block") and not a["already_archived"]]
dels = [d["source"] for a in plan["artifacts"] for d in a["pending_deletions"]]
figure("REQ-05 preview", plan["eligible"] and not plan["blockers"] and not todo and not dels,
       f"eligible {plan['eligible']}, blockers {len(plan['blockers'])}, unarchived move/copy/block {len(todo)}, pending deletions {len(dels)}")

differ = sorted(p for p, a in pins.items() if at(a["version"], p) != now(p))
retained = [p for p, a in pins.items() if a["action"] == "retain" and "pinned-historical" in a["evidence"]]
commits = sorted({a["version"][:9] for a in pins.values()})
figure("pinned references", len(retained) == len(pins) and len(commits) == 1,
       f"{len(pins)} pinned to {commits}, {len(retained)} retained by the preview, {len(pins) - len(differ)} with the pinned blob in the working tree, differing: {differ}")
for path in differ:
    print(run("git", "diff", "--stat", pins[path]["version"], "--", path).strip().splitlines()[-1].strip())

touched = run("git", "log", "--format=", "--name-only", "--grep", f"jit:{ISSUE}").split()
files = sorted({f for f in [*touched, RECORD, *listed("HEAD", archive)] if f.endswith(".md") and (ROOT / f).is_file()})
unresolved = 0
for name in files:
    fenced, links = False, 0
    for number, line in enumerate((ROOT / name).read_text().splitlines(), 1):
        if re.match(r"\s*(```|~~~)", line):
            fenced = not fenced
        if fenced:
            continue
        for target in LINK.findall(line):
            if SKIP.match(target):
                continue
            links += 1
            path = urllib.parse.unquote(target.split("#", 1)[0])
            base = ROOT if path.startswith("/") else (ROOT / name).parent
            if not base.joinpath(path.lstrip("/")).exists():
                unresolved += 1
                print(f"{name}:{number}: unresolved: {target}")
    print(f"scanned {name}: {links} local links")
figure("REQ-06 link scan", unresolved == 0, f"{unresolved} unresolved local links in {len(files)} files")
sys.exit(1 if failed else 0)
