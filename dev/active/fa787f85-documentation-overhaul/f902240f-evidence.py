#!/usr/bin/env python3
"""Hash, state, preview and link-scan figures for the b7157be6 container archive.

Run from the repository root. Compares the tree before the execution commit
(`EXEC^`) with the working tree, summarizes the live archive preview, and scans
inline local links of the Markdown files that commits tagged `jit:f902240f`
touch, of this unit's record, and of the archive directory. URL, `mailto:` and
anchor-only targets and fenced code are skipped. Exit 1 on any failed figure.
"""
import hashlib
import json
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path

EXEC = "7335791e4dbb83e123d2e3c794e92b0b6344fe9e"
PIN = "744aa9b037c8c5c0c61e75dffab0f2e891ba926a"
EPIC = "b7157be6-16d8-4050-834c-e996d4fa27c3"
ARCHIVE = "dev/archive/b7157be6-osd"
REVIEW = "dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md"
RECEIPT = "bench_results/2026-08-27-258be082-osd-campaign-worker-scaling.md"
DATASET = "dev/simulation_results/osd-ebch-128-64"
RECORD = "dev/active/fa787f85-documentation-overhaul/f902240f-osd-archive.md"
LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")


def git(*args, text=True):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=text).stdout


def sha(data):
    return hashlib.sha256(data).hexdigest()


def before(path):
    return sha(git("show", f"{EXEC}^:{path}", text=False))


def now(path):
    return sha(Path(path).read_bytes()) if Path(path).is_file() else "absent"


def listed(rev, path):
    return git("ls-tree", "-r", "--name-only", rev, "--", path).split()


failed = 0


def figure(label, ok, detail):
    global failed
    failed += not ok
    print(f"{'ok  ' if ok else 'FAIL'} {label}: {detail}")


hand = listed(f"{EXEC}^", ARCHIVE)
kept = [p for p in hand if before(p) == sha(git("show", f"{EXEC}:{p}", text=False))]
figure("REQ-01 hand archive across the execution commit", len(kept) == len(hand), f"{len(kept)}/{len(hand)} files byte-identical")
changed = sorted(p for p in hand if before(p) != now(p))
figure("REQ-01 hand archive in the working tree", all(Path(p).is_file() for p in hand), f"{len(hand)} present, changed since: {changed}")

marker = Path(ARCHIVE, ".jit-container").read_text()
figure("REQ-02 marker", marker == EPIC + "\n", marker.strip())
state = json.loads(next(Path(".jit/issues").glob("b7157be6-*.json")).read_text())["state"]
figure("REQ-02 epic state in .jit/issues", state == "archived", state)

copy = f"{ARCHIVE}/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md"
figure("REQ-03 review live == archive copy", now(REVIEW) == now(copy) == before(REVIEW), now(copy))
figure("receipt moved", before("dev/" + RECEIPT) == now(f"{ARCHIVE}/{RECEIPT}") and now("dev/" + RECEIPT) == "absent", now(f"{ARCHIVE}/{RECEIPT}"))

data = listed(f"{EXEC}^", DATASET)
changed = sorted(p for p in data if before(p) != now(p))
figure("REQ-04 dataset", all(Path(p).is_file() for p in data), f"{len(data)} files present, changed since: {changed}")

plan = json.loads(subprocess.run(["jit", "archive", "container", "b7157be6", "--json"], check=True, capture_output=True, text=True).stdout)
todo = [a["source"] for a in plan["artifacts"] if a["action"] in ("move", "copy", "block") and not a["already_archived"]]
dels = [d["source"] for a in plan["artifacts"] for d in a["pending_deletions"]]
figure("REQ-05 preview", plan["eligible"] and not plan["blockers"] and not todo and not dels,
       f"eligible {plan['eligible']}, blockers {len(plan['blockers'])}, unarchived move/copy/block {len(todo)}, pending deletions {len(dels)}")

pins = {doc["path"]: doc["commit"] for issue in ("cef1ae5f", "a82f2dd9")
        for doc in json.loads(next(Path(".jit/issues").glob(issue + "-*.json")).read_text())["documents"]}
differ = sorted(p for p, c in pins.items() if sha(git("show", f"{c}:{p}", text=False)) != now(p))
retained = [a["source"] for a in plan["artifacts"] if a["source"] in pins and a["action"] == "retain" and "pinned-historical" in a["evidence"]]
figure("pinned references", len(retained) == len(pins) and set(pins.values()) == {PIN},
       f"{len(pins)} pinned to {PIN[:9]}, {len(retained)} retained by the preview, {len(pins) - len(differ)} with the pinned blob in the working tree, differing: {differ}")
for path in differ:
    print(git("diff", "--stat", PIN, "--", path).strip().splitlines()[-1].strip())

touched = git("log", "--format=", "--name-only", "--grep", "jit:f902240f").split()
files = sorted({f for f in [*touched, RECORD, *listed("HEAD", ARCHIVE)] if f.endswith(".md") and Path(f).is_file()})
unresolved = 0
for name in files:
    fenced, links = False, 0
    for number, line in enumerate(Path(name).read_text().splitlines(), 1):
        if re.match(r"\s*(```|~~~)", line):
            fenced = not fenced
        if fenced:
            continue
        for target in LINK.findall(line):
            if SKIP.match(target):
                continue
            links += 1
            path = urllib.parse.unquote(target.split("#", 1)[0])
            base = Path(".") if path.startswith("/") else Path(name).parent
            if not base.joinpath(path.lstrip("/")).exists():
                unresolved += 1
                print(f"{name}:{number}: unresolved: {target}")
    print(f"scanned {name}: {links} local links")
figure("REQ-06 link scan", unresolved == 0, f"{unresolved} unresolved local links in {len(files)} files")
sys.exit(1 if failed else 0)
