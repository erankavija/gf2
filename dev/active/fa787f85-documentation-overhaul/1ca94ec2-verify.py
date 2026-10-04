#!/usr/bin/env python3
"""Evidence for issue 1ca94ec2: re-archive of epics e095a100, 806eb14e, 2928ccce and d4851c3d.

Run from any directory of the checkout:
    python3 <this file>                      print the verification report
    python3 <this file> --reduce A.json ...  reduce `jit archive container <epic> --json`
                                             previews to the committed 1ca94ec2-preexec.json

The report compares the working tree and the live tracker with the reduced
pre-execution previews beside this file. A link target starting with `/` resolves
from the repository root. Exit 1 on any failed check.
"""
import hashlib
import json
import re
import subprocess
import sys
import tomllib
import urllib.parse
from pathlib import Path

LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")
ISSUE = "1ca94ec2"


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


ROOT = Path(git("rev-parse", "--show-toplevel").strip())
PREEXEC = Path(__file__).with_name(f"{ISSUE}-preexec.json")


def jit(*args):
    return json.loads(subprocess.run(["jit", *args, "--json"], check=True, capture_output=True, text=True).stdout)


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


def placed_before(path, sha256):
    """True when a commit tagged jit:ISSUE changed `path` from the bytes with `sha256`."""
    for commit in git("-C", str(ROOT), "log", "--format=%H", "--grep", f"jit:{ISSUE}", "--", path).split():
        before = subprocess.run(["git", "-C", str(ROOT), "show", f"{commit}^:{path}"], capture_output=True)
        if before.returncode == 0 and hashlib.sha256(before.stdout).hexdigest() == sha256:
            return True
    return False


def link_scan(files, root):
    links = unresolved = 0
    for name in files:
        fenced = False
        for number, line in enumerate(name.read_text().splitlines(), 1):
            if re.match(r"\s*(```|~~~)", line):
                fenced = not fenced
            if fenced:
                continue
            for target in LINK.findall(line):
                if SKIP.match(target):
                    continue
                links += 1
                path = urllib.parse.unquote(target.split("#", 1)[0])
                base = root / path.lstrip("/") if path.startswith("/") else name.parent / path
                if not base.exists():
                    unresolved += 1
                    print(f"  {name.relative_to(root)}:{number}: unresolved: {target}")
    return len(files), links, unresolved


def main():
    failed = 0
    docs, placed_paths = {}, set()
    epics = json.loads(PREEXEC.read_text())
    for epic, pre in epics.items():
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
        placed_paths.update(a["destination"] for a in placed)
        differ = [a for a in placed if hashlib.sha256((ROOT / a["destination"]).read_bytes()).hexdigest() != a["sha256"]]
        edited = [a["destination"] for a in differ if placed_before(a["destination"], a["sha256"])]
        equal = len(placed) - len(differ)
        deletes = [s for a in pre["artifacts"] for s in a["deletes"]]
        gone = sum(not (ROOT / s).exists() for s in deletes)
        print(f"byte verification: {equal}/{len(placed)} destinations carry the pre-execution sha256; "
              f"{len(edited)} carried it before a commit tagged jit:{ISSUE} changed them; "
              f"{gone}/{len(deletes)} deleted sources are absent")
        for path in edited:
            print(f"  changed after placement: {path}")
        failed += equal + len(edited) != len(placed) or gone != len(deletes)

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

        files, links, unresolved = link_scan(sorted(archive.rglob("*.md")), ROOT)
        print(f"link scan of {fresh['destination_root']}: {files} Markdown files, {links} local links, {unresolved} unresolved")
        failed += bool(unresolved)

    print(f"== Markdown files touched by the commits tagged jit:{ISSUE}")
    touched = git("-C", str(ROOT), "log", "--format=", "--name-only", "--grep", f"jit:{ISSUE}").split("\n")
    files, links, unresolved = link_scan(sorted({ROOT / f for f in touched if f.endswith(".md") and (ROOT / f).is_file()}), ROOT)
    print(f"link scan: {files} Markdown files, {links} local links, {unresolved} unresolved")
    failed += bool(unresolved)

    print("== manifest rows whose destination the pre-execution plans place")
    (manifest,) = PREEXEC.parent.rglob("manifest.toml")
    rows = tomllib.loads(manifest.read_text())["artifacts"]
    for row in rows:
        if row["destination"] in placed_paths:
            print(f"{row['status']} {row['path']} -> {row['destination']}")
            failed += row["status"] != "complete"
    print("== manifest rows by epic")
    for epic in epics:
        own = [row for row in rows if row["epic"] == epic]
        pending = sum(row["status"] == "pending" for row in own)
        print(f"{epic}: {len(own)} rows, {pending} pending")
        failed += bool(pending)
    return 1 if failed else 0


if __name__ == "__main__":
    if sys.argv[1:2] == ["--reduce"]:
        reduce(sys.argv[2:])
    else:
        sys.exit(main())
