#!/usr/bin/env python3
"""Evidence for issue 616e1d7c: re-archive of epic 6dc81018.

Run from any directory of the checkout:
    python3 <this file>                print the verification report
    python3 <this file> --reduce RAW   reduce a `jit archive container 6dc81018 --json`
                                       preview to the committed 616e1d7c-preexec.json

The report compares the execution commit, the working tree and the tracker of
the checkout with the reduced pre-execution preview beside this file, and takes
a fresh preview. Sections:

  preview     the pre-execution plan: eligibility, blockers, counts per action.
  marker      the archive root's `.jit-container` names the container.
  bytes       at EXECUTION, each destination the plan publishes holds the planned
              sha256, each move source is absent and each copy source holds the
              planned sha256; at HEAD, the destinations whose bytes differ from
              the plan are the files commits after EXECUTION changed. Lists the
              copy sources under the active area that differ from their archive
              file; a difference does not fail.
  excluded    no path under the entries of EXCLUDED differs between the parent
              of EXECUTION and HEAD; the plan publishes and deletes none.
  references  each tracker document the plan relinks names its archive path.
  rerun       a fresh preview publishes and deletes nothing.
  links       inline Markdown links with a local target, outside fenced code, in
              the Markdown files of the archive root and in the Markdown files
              under the active area that a commit of this issue changed after
              EXECUTION. Each target resolves from the file's own directory,
              or from the checkout root when it starts with `/`.
  manifest    the manifest rows under the active area that name the container
              as epic or owner, entries of EXCLUDED left out: count per entry,
              disposition, status and directory holding the entry. Each row is
              complete and its file is at its destination, or at its path when
              the destination is empty.
  consumers   the `consumers` fields of those rows: each consumer lies inside
              an entry of which a row has a destination.
  citations   each citation at SWEEP, outside the tracker directory, of a path
              under an entry of `manifest` (as `<active area>/<entry>...` or
              `../<entry>/...`) that is absent from the working tree, counted
              per citing file and action. `repointed`: the line carries the
              path of the file in the working tree. `kept` names the first of: manifest file, digest
              pin (manifest `digest_pinned`, or the file's sha256 or blob id
              in a tracked file other than the tracker event log and the
              files of this issue), `.jit-container` root, equal bytes with
              the archive file the plan copies it to, tracker reference at a
              commit, tracker document type, file suffix other than `.md`. A
              citation with neither fails, and so does a working-tree citation
              with an absent target that SWEEP lacks. The cited
              `:line` suffixes are not checked.

Exit 1 on any failed check.
"""
import collections
import glob
import hashlib
import json
import re
import subprocess
import sys
import tomllib
import urllib.parse
from pathlib import Path

ISSUE = "616e1d7c"
CONTAINER = "6dc81018"
EXCLUDED = ("a83583e0", "dbd8787d")  # code-pinned entries with their own move issues
EXECUTION = "50de04a6b28b28f9710218e4d6509354ad9bb2e2"
SWEEP = "42929def5cc0e8d02b59758c439b4e6a78974737"  # the tree the citation sweep reads
FOOTER = f"jit:{ISSUE}"
LINK = re.compile(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
SKIP = re.compile(r"^(#|[A-Za-z][A-Za-z0-9+.-]*:)")


def git(*args, text=True):
    return subprocess.run(["git", "-C", str(ROOT), *args], check=True, capture_output=True, text=text).stdout


ROOT = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True).stdout.strip())
HERE = Path(__file__).resolve().parent
PREEXEC = HERE / f"{ISSUE}-preexec.json"
ACTIVE = HERE.parent.relative_to(ROOT).as_posix()
FAILED = []


def check(label, ok, detail=""):
    print(f"{label}: {'ok' if ok else 'FAIL'}{' ' + detail if detail else ''}")
    if not ok:
        FAILED.append(label)


def reduce(raw):
    p = json.loads(Path(raw).read_text())
    PREEXEC.write_text(json.dumps({
        "target": p["target"]["id"],
        "destination_root": p["destination_root"],
        "eligible": p["eligible"],
        "blockers": p["blockers"] + [b for a in p["artifacts"] for b in a["blockers"]],
        "action_counts": p["action_counts"],
        "artifacts": [{
            "source": a["source"],
            "action": a["action"],
            "already_archived": a["already_archived"],
            "destination": a["destination"],
            "sha256": a["content_identity"] and a["content_identity"]["sha256"],
            "evidence": a["evidence"],
            "relinks": [[r["issue"], r["document_index"], r["to_path"]] for r in a["reference_changes"]],
            "deletes": [d["source"] for d in a["pending_deletions"]],
        } for a in p["artifacts"]],
    }, indent=1, sort_keys=True) + "\n")


def blob_sha(rev, path):
    done = subprocess.run(["git", "-C", str(ROOT), "cat-file", "blob", f"{rev}:{path}"], capture_output=True)
    return hashlib.sha256(done.stdout).hexdigest() if done.returncode == 0 else None


def file_sha(path):
    path = ROOT / path
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


def entry(path):
    return path[len(ACTIVE) + 1:].split("/", 1)[0] if path.startswith(ACTIVE + "/") else None


def unresolved_links(files):
    total, bad = 0, []
    for name in files:
        fenced = False
        for number, line in enumerate((ROOT / name).read_text(errors="replace").splitlines(), 1):
            if line.lstrip().startswith(("```", "~~~")):
                fenced = not fenced
            if fenced:
                continue
            for target in LINK.findall(line):
                if SKIP.match(target):
                    continue
                total += 1
                local = urllib.parse.unquote(target.split("#", 1)[0])
                where = ROOT / local.lstrip("/") if local.startswith("/") else (ROOT / name).parent / local
                if not where.exists():
                    bad.append(f"{name}:{number} {target}")
    return total, bad


def present(path):
    brace = re.search(r"\{([^{}]*)\}", path)
    if brace:
        return all(present(path[:brace.start()] + alt + path[brace.end():]) for alt in brace.group(1).split(","))
    return bool(glob.glob(str(ROOT / path))) if "*" in path else (ROOT / path).exists()


def cited(names, rev=None):
    """(file, line number, token, path) per citation of an entry in `rev`, or in the working tree."""
    found = set()
    for name in names:
        args = ["grep", "-nIF", "-e", f"{ACTIVE}/{name}", "-e", f"../{name}/", *([rev] if rev else []), "--", ":!.jit"]
        out = subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True).stdout
        token = re.compile(rf"(?:{re.escape(ACTIVE)}/|(?:\.\./)+){re.escape(name)}(?![\w-])(?:/[\w.{{}},*+@=/-]*)?")
        for row in out.splitlines():
            name_, number, text = row[len(rev) + 1 if rev else 0:].split(":", 2)
            for match in token.finditer(text):
                cite = match.group(0).rstrip(".,/")
                path = cite if cite.startswith(ACTIVE) else Path(name_).parent.joinpath(cite).as_posix()
                parts = []
                for part in path.split("/"):
                    parts.pop() if part == ".." and parts else parts.append(part)
                found.add((name_, int(number), cite, "/".join(parts)))
    return found


def main():
    if sys.argv[1:2] == ["--reduce"] and len(sys.argv) == 3:
        return reduce(sys.argv[2])
    if sys.argv[1:]:
        sys.exit(__doc__)
    plan = json.loads(PREEXEC.read_text())
    arts = plan["artifacts"]
    root = plan["destination_root"]
    new = [a for a in arts if a["action"] in ("move", "copy") and not a["already_archived"]]
    moves = [a for a in new if a["action"] == "move"]
    copies = [a for a in new if a["action"] == "copy"]
    deletes = [s for a in arts for s in a["deletes"]]

    print("== preview")
    check("eligible, no blocker", plan["eligible"] and not plan["blockers"], json.dumps(plan["action_counts"], sort_keys=True))
    print("moves by entry:", dict(sorted(collections.Counter(entry(a["source"]) for a in moves).items())))
    print("copies by entry:", dict(sorted(collections.Counter(entry(a["source"]) for a in copies).items())))
    check("each deletion is the source of a planned move", sorted(deletes) == sorted(a["source"] for a in moves), f"{len(deletes)} deletions")

    print("== marker")
    check(f"{root}/.jit-container", (ROOT / root / ".jit-container").read_text().strip() == plan["target"], plan["target"])

    print("== bytes")
    at_exec = [a for a in new if blob_sha(EXECUTION, a["destination"]) == a["sha256"]]
    check(f"destinations at {EXECUTION[:9]} with the planned sha256", len(at_exec) == len(new), f"{len(at_exec)}/{len(new)}")
    gone = [a for a in moves if blob_sha(EXECUTION, a["source"]) is None and not (ROOT / a["source"]).exists()]
    check("move sources absent at the execution commit and in the working tree", len(gone) == len(moves), f"{len(gone)}/{len(moves)}")
    kept = [a for a in copies if file_sha(a["source"]) == a["sha256"]]
    check("copy sources present with the planned sha256", len(kept) == len(copies), f"{len(kept)}/{len(copies)}")
    changed = set(git("diff", "--name-only", EXECUTION, "--", root).split())
    differing = {a["destination"] for a in new if file_sha(a["destination"]) != a["sha256"]}
    check("destinations differing from the plan are files changed after execution", differing <= changed and all((ROOT / a["destination"]).is_file() for a in new),
          f"{len(differing)} differ, {len(new) - len(differing)} hold the planned sha256")

    twins = [a for a in arts if a["action"] == "copy" and entry(a["source"]) and (ROOT / a["source"]).is_file()]
    apart = sorted(a["source"] for a in twins if file_sha(a["source"]) != file_sha(a["destination"]))
    print(f"copy sources under the active area: {len(twins)} present, {len(twins) - len(apart)} equal to their archive file")
    for path in apart:
        print("  differs from its archive file", path)

    print("== excluded")
    for short in EXCLUDED:
        base = f"{ACTIVE}/{short}"
        diff = git("diff", "--name-only", f"{EXECUTION}^", "--", base).split()
        touched = [a["source"] for a in new if a["source"].startswith(base + "/")] + [s for s in deletes if s.startswith(base + "/")]
        check(f"{base} unchanged, not published, not deleted", not diff and not touched, f"{len(git('ls-files', '--', base).split())} tracked files")

    print("== references")
    issues = {}
    for f in (ROOT / ".jit/issues").glob("*.json"):
        issue = json.loads(f.read_text())
        issues[issue["id"]] = issue
    relinks = [r for a in arts for r in a["relinks"]]
    named = [r for r in relinks if issues[r[0]]["documents"][r[1]]["path"] == r[2]]
    check("tracker documents naming the planned archive path", len(named) == len(relinks), f"{len(named)}/{len(relinks)} on {len({r[0] for r in relinks})} issues")
    pinned = [a for a in arts if a["action"] == "retain" and "pinned-historical" in a["evidence"] and entry(a["source"]) in EXCLUDED]
    check("pinned sources of the excluded entries present", all((ROOT / a["source"]).is_file() for a in pinned), f"{len(pinned)}")

    print("== rerun")
    fresh = json.loads(subprocess.run(["jit", "archive", "container", CONTAINER, "--json"], cwd=ROOT, check=True, capture_output=True, text=True).stdout)
    todo = [a["source"] for a in fresh["artifacts"] if a["action"] in ("move", "copy") and not a["already_archived"]]
    blockers = fresh["blockers"] + [b for a in fresh["artifacts"] for b in a["blockers"]]
    check("fresh preview eligible, no blocker, nothing to publish or delete",
          fresh["eligible"] and not blockers and not todo and fresh["action_counts"]["pending_deletions"] == 0,
          json.dumps(fresh["action_counts"], sort_keys=True))

    print("== links")
    archive_md = [f for f in git("ls-files", "--", root).split("\n") if f.endswith(".md")]
    total, bad = unresolved_links(archive_md)
    check(f"{root}: Markdown files {len(archive_md)}, local links {total}", not bad, f"unresolved {len(bad)}")
    outside = sorted(f for f in git("diff", "--name-only", EXECUTION, "--", ACTIVE).split("\n")
                     if f.endswith(".md") and (ROOT / f).is_file() and FOOTER in git("log", "--format=%s", f"{EXECUTION}..HEAD", "--", f))
    total, bad2 = unresolved_links(outside)
    check(f"files of this issue under the active area: Markdown files {len(outside)}, local links {total}", not bad2, f"unresolved {len(bad2)}")
    for line in bad + bad2:
        print("  unresolved", line)

    print("== manifest")
    (manifest,) = [f for f in git("ls-files", "--", ":(glob)**/migration/manifest.toml").split("\n") if f]
    rows = tomllib.loads((ROOT / manifest).read_text())["artifacts"]
    states = collections.Counter()
    astray = []
    for row in rows:
        name = entry(row["path"])
        if CONTAINER not in (row["epic"], *row["owners"]) or name in EXCLUDED + (None,):
            continue
        home = row["destination"] or row["path"]
        states[(name, row["disposition"], row["status"], home[:home.rindex(f"/{name}/")])] += 1
        if row["status"] != "complete" or not (ROOT / home).is_file() or (home != row["path"] and (ROOT / row["path"]).exists()):
            astray.append(row["path"])
    for key, count in sorted(states.items()):
        print(f"  {count:4d} {' / '.join(key)}")
    for path in astray:
        print("  not complete or not at its destination", path)
    check("rows complete, each file at its destination", not astray, f"{sum(states.values())} rows of {len({k[0] for k in states})} entries")

    print("== consumers")
    names = sorted({k[0] for k in states})
    unit = [row for row in rows if entry(row["path"]) in names]
    moved = {entry(row["path"]) for row in unit if row["destination"]}
    pairs = collections.Counter((entry(row["path"]), entry(c)) for row in unit for c in row["consumers"])
    for (owner, user), count in sorted(pairs.items(), key=str):
        print(f"  {count:4d} consumers of {owner} in {user}")
    outside = [f"{row['path']} <- {c}" for row in unit for c in row["consumers"] if entry(c) not in moved]
    for line in outside:
        print("  outside", line)
    check("each consumer lies in an entry with a moved or archived row", not outside,
          f"{sum(pairs.values())} consumers on {sum(1 for row in unit if row['consumers'])} rows, entries {' '.join(sorted(moved))}")

    print("== citations")
    homes = {row["path"]: row["destination"] for row in rows}
    pinned = {f for row in rows if row["digest_pinned"] for f in (row["path"], row["destination"])}
    absent = sorted(c for c in cited(names, SWEEP) if not present(c[3]))
    files = sorted({c[0] for c in absent if (ROOT / c[0]).is_file()})
    digests = {}
    for name in files:
        digests[file_sha(name)] = digests[git("hash-object", "--", name).strip()] = name
    grep = subprocess.run(["git", "-C", str(ROOT), "grep", "-noIF", "-f", "-", "--", ":!.jit/events.jsonl", f":!{HERE.relative_to(ROOT).as_posix()}/{ISSUE}-*"],
                          input="\n".join(digests), capture_output=True, text=True).stdout
    pins = {}
    for row in grep.splitlines():
        where, number, digest = row.rsplit(":", 2)
        pins.setdefault(digests[digest], f"{where}:{number}")
    tracked = {}
    for issue in issues.values():
        for doc in issue["documents"]:
            tracked.setdefault(doc["path"], []).append((doc["commit"] or "", issue["id"][:8], doc["doc_type"] or "untyped"))

    def ground(name):
        if name == manifest:
            return "manifest file"
        if name in pinned:
            return "digest pin: manifest digest_pinned"
        if name in pins:
            return f"digest pin: {pins[name]}"
        for parent in Path(name).parents:
            if (ROOT / parent / ".jit-container").is_file():
                return f"container root {parent.as_posix()}"
        if name in copied and file_sha(name) == file_sha(copied[name]):
            return f"equal bytes with {copied[name]}"
        refs = sorted(tracked.get(name, []), reverse=True)
        if refs and refs[0][0]:
            return f"tracker reference of {refs[0][1]} at {refs[0][0]}"
        if refs:
            return f"tracker document of {refs[0][1]}: {refs[0][2]}"
        return f"file suffix {Path(name).suffix}" if Path(name).suffix != ".md" else None

    copied = {a["source"]: a["destination"] for a in arts if a["action"] == "copy"}
    live = {}
    actions, changed = collections.Counter(), []
    for name, number, cite, path in absent:
        home = homes.get(path) or f"{root}/{Path(ACTIVE).name}/{path[len(ACTIVE) + 1:]}"
        if name not in live:
            live[name] = (ROOT / name).read_text(errors="replace").splitlines() if (ROOT / name).is_file() else []
        text = live[name][number - 1] if number <= len(live[name]) else ""
        if cite in text and ground(name):
            action = f"kept: {ground(name)}"
            actions[(action, "" if "container root" in action else name)] += 1
        elif cite not in text and (ROOT / home).exists() and home in text:
            actions[("repointed:", name)] += 1
        else:
            changed.append(f"{name}:{number} {cite}")
    for (action, name), count in sorted(actions.items()):
        print(f"  {count:4d} {action}{' ' + name if name else ''}")
    for line in changed:
        print("  neither repointed nor kept", line)
    check(f"citations at {SWEEP[:9]} with an absent target: each is repointed or kept on a ground", not changed,
          f"{len(absent)} citations in {len({c[0] for c in absent})} files, {sum(n for (a, _), n in actions.items() if a == 'repointed:')} repointed")
    after = sorted(c for c in cited(names) if not present(c[3]))
    fresh_cites = [c for c in after if (c[0], c[1], c[2]) not in {(a[0], a[1], a[2]) for a in absent}]
    for c in fresh_cites:
        print(f"  not at {SWEEP[:9]}: {c[0]}:{c[1]} {c[2]}")
    check("working-tree citations with an absent target are citations kept above", not fresh_cites, f"{len(after)}")
    return 1 if FAILED else 0


if __name__ == "__main__":
    sys.exit(main())
