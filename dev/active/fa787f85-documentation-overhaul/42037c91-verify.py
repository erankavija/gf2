#!/usr/bin/env python3
"""Evidence for issue 42037c91: re-archive of epics bb85c68a and 6efb756b.

Run from any directory of the checkout:
    python3 <this file>                      print the verification report
    python3 <this file> --reduce A.json ...  reduce `jit archive container <epic> --json`
                                             previews to the committed 42037c91-preexec.json

The report compares the working tree and the tracker of the checkout with the
reduced pre-execution previews beside this file. Exit 1 on any failed check.
"""
import collections
import hashlib
import json
import re
import subprocess
import sys
import tomllib
import urllib.parse
from pathlib import Path

LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
ATTR = re.compile(r"""\b(?:href|src)\s*=\s*["']([^"']+)["']""")
SKIP = re.compile(r"^(#|//|[A-Za-z][A-Za-z0-9+.-]*:)")
ISSUE = "42037c91"


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


ROOT = Path(git("rev-parse", "--show-toplevel").strip())
PREEXEC = Path(__file__).with_name(f"{ISSUE}-preexec.json")


def jit(*args):
    return json.loads(subprocess.run(["jit", *args, "--json"], check=True, capture_output=True, text=True, cwd=ROOT).stdout)


def reduce(paths):
    out = {}
    for path in paths:
        p = json.loads(Path(path).read_text())
        out[p["target"]["id"][:8]] = {
            "target": p["target"]["id"],
            "destination_root": p["destination_root"],
            "eligible": p["eligible"],
            "blockers": p["blockers"] + [b for a in p["artifacts"] for b in a["blockers"]],
            "action_counts": p["action_counts"],
            "artifacts": [
                {
                    "source": a["source"],
                    "action": a["action"],
                    "already_archived": a["already_archived"],
                    "destination": a["destination"],
                    "sha256": a["content_identity"] and a["content_identity"]["sha256"],
                    "deletes": [d["source"] for d in a["pending_deletions"]],
                }
                for a in p["artifacts"]
            ],
        }
    PREEXEC.write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")


def link_scan(files):
    links = unresolved = 0
    for name in files:
        pattern, fenced = (ATTR if name.suffix == ".html" else LINK), False
        for number, line in enumerate(name.read_text().splitlines(), 1):
            if pattern is LINK and re.match(r"\s*(```|~~~)", line):
                fenced = not fenced
            if fenced:
                continue
            for target in pattern.findall(line):
                if SKIP.match(target):
                    continue
                links += 1
                if not name.parent.joinpath(urllib.parse.unquote(re.split(r"[#?]", target, maxsplit=1)[0])).exists():
                    unresolved += 1
                    print(f"  {name.relative_to(ROOT)}:{number}: unresolved: {target}")
    return len(files), links, unresolved


def scanned(paths):
    return sorted(p for p in paths if p.suffix in (".md", ".html") and p.is_file())


def main():
    failed = 0
    docs, deleted = {}, set()
    preexec = json.loads(PREEXEC.read_text())
    for epic, pre in preexec.items():
        print(f"== {epic}")
        print(f"pre-execution preview: eligible={pre['eligible']} destination_root={pre['destination_root']} "
              f"blockers={len(pre['blockers'])} counts={json.dumps(pre['action_counts'], sort_keys=True)}")
        fresh = jit("archive", "container", epic)
        archive = ROOT / fresh["destination_root"]
        marker = (archive / ".jit-container").read_text().strip()
        print(f"marker: {fresh['destination_root']}/.jit-container names {marker}")
        failed += (not pre["eligible"] or bool(pre["blockers"]) or marker != pre["target"]
                   or fresh["destination_root"] != pre["destination_root"])

        placed = [a for a in pre["artifacts"] if a["destination"]]
        outside = [a["destination"] for a in placed if archive not in (ROOT / a["destination"]).parents]
        equal = sum(hashlib.sha256((ROOT / a["destination"]).read_bytes()).hexdigest() == a["sha256"] for a in placed)
        deletes = [s for a in pre["artifacts"] for s in a["deletes"]]
        deleted.update(deletes)
        gone = sum(not (ROOT / s).exists() for s in deletes)
        print(f"byte verification: {equal}/{len(placed)} destinations carry the pre-execution sha256, "
              f"{len(outside)} outside the archive directory; {gone}/{len(deletes)} deleted sources are absent")
        failed += equal != len(placed) or gone != len(deletes) or bool(outside)

        open_work = [a["source"] for a in fresh["artifacts"]
                     if a["pending_deletions"] or (a["action"] in ("move", "copy") and not a["already_archived"])]
        blockers = fresh["blockers"] + [b for a in fresh["artifacts"] for b in a["blockers"]]
        print(f"fresh preview: eligible={fresh['eligible']} blockers={len(blockers)} "
              f"counts={json.dumps(fresh['action_counts'], sort_keys=True)} "
              f"artifacts left to move, copy or delete={len(open_work)}")
        failed += bool(open_work) or bool(blockers)

        changes = [c for a in fresh["artifacts"] for c in a["reference_changes"]]
        named = 0
        for c in changes:
            if c["issue"] not in docs:
                docs[c["issue"]] = jit("doc", "list", c["issue"])["documents"]
            named += docs[c["issue"]][c["document_index"]]["path"] == c["to_path"]
        print(f"tracker references: {named}/{len(changes)} planned references name the archive path")
        failed += named != len(changes)

        files, links, unresolved = link_scan(scanned(archive.rglob("*")))
        print(f"link scan of {fresh['destination_root']}: {files} Markdown and HTML files, {links} local links, {unresolved} unresolved")
        failed += bool(unresolved)

    print("== issues holding one path in several document references")
    for issue, documents in sorted(docs.items()):
        for path, count in collections.Counter(d["path"] for d in documents).items():
            if count > 1:
                print(f"{issue[:8]}: {count} references name {path}")

    print(f"== files touched by the commits tagged jit:{ISSUE}")
    touched = git("-C", str(ROOT), "log", "--format=", "--name-only", "--grep", f"jit:{ISSUE}").split("\n")
    files, links, unresolved = link_scan(scanned({ROOT / f for f in touched if f}))
    print(f"link scan: {files} Markdown and HTML files, {links} local links, {unresolved} unresolved")
    failed += bool(unresolved)

    print("== manifest rows")
    (manifest,) = PREEXEC.parent.rglob("manifest.toml")
    rows = tomllib.loads(manifest.read_text())["artifacts"]
    for epic in preexec:
        status = collections.Counter(row["status"] for row in rows if row["epic"] == epic)
        print(f"rows with epic {epic}: {json.dumps(status, sort_keys=True)}")
        failed += set(status) != {"complete"}
    for row in rows:
        if row["path"] in deleted:
            print(f"{row['status']} {row['disposition']} {row['path']} -> {row['destination']}")
            failed += row["status"] != "complete" or (ROOT / row["path"]).exists() or not (ROOT / row["destination"]).is_file()
    return 1 if failed else 0


if __name__ == "__main__":
    if sys.argv[1:2] == ["--reduce"]:
        reduce(sys.argv[2:])
    else:
        sys.exit(main())
