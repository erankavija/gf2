#!/usr/bin/env python3
"""Evidence for issue 3759f995: re-archive of epics babcf05e and f9717e7e.

Run from the repository root:
    python3 <this file>                      print the verification report
    python3 <this file> --reduce A.json ...  reduce `jit archive container <epic> --json`
                                             previews to the committed 3759f995-preexec.json

The report compares the working tree and the live tracker with the reduced
pre-execution previews beside this file. Exit 1 on any failed check.
"""
import hashlib
import json
import re
import subprocess
import sys
import tomllib
import urllib.parse
from pathlib import Path

HERE = Path(__file__).resolve().parent
PREEXEC = HERE / "3759f995-preexec.json"
LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")
REPOINTED = ("perf-evidence-catalog.md", "investigation.md", "036615b0-inventory-notes.md")
ROWS = (
    "dev/active/babcf05e-gf2-core-ppc-spiral/babcf05e-handoff-5.md",
    "dev/bench_results/2026-04-26-uncompetitiveness-profile.md",
    "dev/bench_results/2026-04-27-asm-audit.md",
    "dev/bench_results/2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md",
    "dev/bench_results/2026-04-29-3abb755e-benchmark-gap-closure.md",
    "dev/bench_results/2026-04-29-gf2m-batch-fieldmatrix-gemm.md",
    "dev/bench_results/2026-04-29-strassen-matmul-crossover.md",
    "dev/bench_results/c7791a20/2026-04-26-profile-release-delta.md",
    "dev/benchmarks/gf2-sim/README.md",
)


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
                if not name.parent.joinpath(urllib.parse.unquote(target.split("#", 1)[0])).exists():
                    unresolved += 1
                    print(f"  {name.relative_to(root)}:{number}: unresolved: {target}")
    return len(files), links, unresolved


def main():
    root = Path.cwd()
    failed = 0
    docs = {}
    for epic, pre in json.loads(PREEXEC.read_text()).items():
        print(f"== {epic}")
        print(f"pre-execution preview: eligible={pre['eligible']} destination_root={pre['destination_root']} "
              f"blockers={len(pre['blockers'])} counts={json.dumps(pre['action_counts'], sort_keys=True)}")
        marker = (root / pre["destination_root"] / ".jit-container").read_text().strip()
        print(f"marker: {pre['destination_root']}/.jit-container names {marker}")
        failed += not pre["eligible"] or bool(pre["blockers"]) or marker != pre["target"]

        placed = [a for a in pre["artifacts"] if a["destination"]]
        equal = sum(hashlib.sha256((root / a["destination"]).read_bytes()).hexdigest() == a["sha256"] for a in placed)
        deletes = [s for a in pre["artifacts"] for s in a["deletes"]]
        gone = sum(not (root / s).exists() for s in deletes)
        print(f"byte verification: {equal}/{len(placed)} destinations carry the pre-execution sha256; "
              f"{gone}/{len(deletes)} deleted sources are absent")
        failed += equal != len(placed) or gone != len(deletes)

        fresh = jit("archive", "container", epic)
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

        files, links, unresolved = link_scan(sorted((root / pre["destination_root"]).rglob("*.md")), root)
        print(f"link scan of {pre['destination_root']}: {files} Markdown files, {links} local links, {unresolved} unresolved")
        failed += bool(unresolved)

    print("== repointed files")
    files, links, unresolved = link_scan([HERE / f for f in REPOINTED], root)
    print(f"link scan: {files} Markdown files, {links} local links, {unresolved} unresolved")
    failed += bool(unresolved)

    print("== manifest rows")
    rows = {r["path"]: r for r in tomllib.loads((HERE / "migration/manifest.toml").read_text())["artifacts"]}
    for path in ROWS:
        print(f"{rows[path]['status']} {path} -> {rows[path]['destination']}")
        failed += rows[path]["status"] != "complete"
    return 1 if failed else 0


if __name__ == "__main__":
    if sys.argv[1:2] == ["--reduce"]:
        reduce(sys.argv[2:])
    else:
        sys.exit(main())
