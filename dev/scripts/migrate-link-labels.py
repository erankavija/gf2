#!/usr/bin/env python3
"""Plan the qualification of `satisfies:` and `cites:` link labels.

Reads the tracker read-only (`.jit/issues/*.json` and `jit item list`) and
writes an idempotent shell script of `jit issue update` commands plus a
markdown record of every label it leaves unqualified.

`satisfies:<id>` becomes `satisfies:<owner>/<id>`. The owner is the nearest
container (an issue whose configured type is coarser than the finest tier and
is not `planning` or `breakdown`) that declares requirement `<id>` and whose
label-coverage walk reaches the labelled issue. The walk follows dependency
edges downward and never enters a `planning` or `breakdown` issue; distance is
the number of edges. A tie at the nearest distance, or no declaring container,
leaves the label unchanged and listed.

`cites:<key>` becomes `cites:@/citation/<key>` when `<key>` is a registered
citation; an unregistered key is left unchanged and listed.

`--verify MIGRATED_ROOT` compares requirement credit of `--root` under
unqualified matching with credit of the migrated copy under qualified matching,
and prints every changed (container, requirement) pair.
"""

import argparse
import collections
import glob
import json
import os
import re
import shlex
import subprocess
import tomllib

SHORT = 8
EXCLUDE = {"planning", "breakdown"}
UNMARKED = re.compile(r"^\s*-\s*(?:\[[ xX]\]\s*)?(REQ-\d+):", re.M)


def load_issues(root):
    issues = {}
    for path in glob.glob(os.path.join(root, ".jit", "issues", "*.json")):
        with open(path) as handle:
            issue = json.load(handle)
        issues[issue["id"]] = issue
    return issues


def items(root, jit, kind):
    out = subprocess.run([jit, "item", "list", "--kind", kind, "--json"], cwd=root,
                         check=True, capture_output=True, text=True).stdout
    return json.loads(out)["items"]


def issue_type(issue):
    return next((l[5:] for l in issue.get("labels", []) if l.startswith("type:")), None)


def hierarchy(root):
    """Container types and the membership namespace of each type."""
    with open(os.path.join(root, ".jit", "config.toml"), "rb") as handle:
        config = tomllib.load(handle)["type_hierarchy"]
    levels = config["types"]
    return ({t for t, level in levels.items() if level < max(levels.values())} - EXCLUDE,
            config.get("label_associations", {}))


def excluded(issue):
    return issue_type(issue) in EXCLUDE


def dependents_of(issues):
    dependents = collections.defaultdict(list)
    for issue in issues.values():
        for dep in issue.get("dependencies", []):
            if dep in issues:
                dependents[dep].append(issue["id"])
    return dependents


def ancestors(issue_id, issues, dependents):
    """Issues whose coverage walk reaches `issue_id`, at their edge distance."""
    found, frontier = {}, collections.deque((d, 1) for d in dependents[issue_id])
    while frontier:
        node, distance = frontier.popleft()
        if node in found:
            continue
        found[node] = distance
        if not excluded(issues[node]):
            frontier.extend((d, distance + 1) for d in dependents[node])
    return found


def descendants(container, issues):
    """Issues the coverage walk from `container` credits."""
    seen, frontier, out = {container["id"]}, list(container.get("dependencies", [])), []
    while frontier:
        node = frontier.pop()
        if node in seen or node not in issues:
            continue
        seen.add(node)
        if excluded(issues[node]):
            continue
        out.append(issues[node])
        frontier.extend(issues[node].get("dependencies", []))
    return out


def unqualified(issue, namespace):
    prefix = f"{namespace}:"
    return [l[len(prefix):] for l in issue.get("labels", [])
            if l.startswith(prefix) and "/" not in l[len(prefix):]]


def plan_satisfies(issues, declared, containers):
    dependents = dependents_of(issues)
    rows = []
    for issue in sorted(issues.values(), key=lambda i: i["id"]):
        values = unqualified(issue, "satisfies")
        if not values:
            continue
        reach = {} if excluded(issue) else ancestors(issue["id"], issues, dependents)
        for value in values:
            row = {"issue": issue["id"][:SHORT], "state": issue.get("state"),
                   "label": f"satisfies:{value}", "replacement": None}
            owners = sorted((d, a[:SHORT]) for a, d in reach.items()
                            if issue_type(issues[a]) in containers and value in declared.get(a[:SHORT], ()))
            if excluded(issue):
                row["kind"], row["reason"] = "orphan", "labelled issue is planning/breakdown; no coverage walk reaches it"
            elif not owners:
                unmarked = sorted(a[:SHORT] for a in reach if issue_type(issues[a]) in containers
                                  and value in UNMARKED.findall(issues[a].get("description", "")))
                row["kind"] = "orphan"
                row["reason"] = (f"declared only as an unmarked criterion by {', '.join(unmarked)}" if unmarked
                                 else "no reaching container declares the id")
            elif len(owners) > 1 and owners[0][0] == owners[1][0]:
                tied = [s for d, s in owners if d == owners[0][0]]
                row["kind"], row["reason"] = "ambiguous", f"tie at distance {owners[0][0]}: {', '.join(tied)}"
            else:
                row["kind"], row["replacement"] = "rewritten", f"satisfies:{owners[0][1]}/{value}"
                row["owner_distance"] = owners[0][0]
            row["declaring"] = [f"{s}@{d}" for d, s in owners]
            rows.append(row)
    return rows


def plan_cites(issues, keys):
    rows = []
    for issue in sorted(issues.values(), key=lambda i: i["id"]):
        for key in unqualified(issue, "cites"):
            row = {"issue": issue["id"][:SHORT], "state": issue.get("state"), "label": f"cites:{key}"}
            if key in keys:
                row["kind"], row["replacement"] = "rewritten", f"cites:@/citation/{key}"
            else:
                row["kind"], row["replacement"] = "unregistered", None
                row["reason"] = "key absent from .jit/references.toml"
            rows.append(row)
    return rows


def script(rows, issues):
    full = {i[:SHORT]: i for i in issues}
    by_issue = collections.defaultdict(list)
    for row in rows:
        if row["replacement"]:
            by_issue[row["issue"]] += ["--remove-label", row["label"], "--label", row["replacement"]]
    lines = ["#!/usr/bin/env bash",
             "# Generated by dev/scripts/migrate-link-labels.py. Idempotent: removing an absent",
             "# label and adding a present one are no-ops.",
             "set -euo pipefail", 'JIT="${JIT:-jit}"']
    lines += [f'"$JIT" issue update {full[short]} ' + " ".join(map(shlex.quote, args))
              for short, args in sorted(by_issue.items())]
    return "\n".join(lines) + "\n"


def table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    out += ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]
    return "\n".join(out) if rows else "None."


def membership_conflicts(sat, issues, associations):
    """Rewritten labels whose owner's membership label the labelled issue lacks."""
    by_short = {i[:SHORT]: v for i, v in issues.items()}
    out = []
    for r in sat:
        if not r["replacement"]:
            continue
        owner = r["replacement"].split(":", 1)[1].split("/")[0]
        namespace = associations.get(issue_type(by_short[owner]))
        member = lambda i: {l for l in i.get("labels", []) if l.startswith(f"{namespace}:")}
        mine, theirs = member(by_short[r["issue"]]), member(by_short[owner])
        if namespace and mine and theirs and not mine & theirs:
            out.append([r["issue"], f"`{r['label']}`", owner, ", ".join(sorted(theirs)),
                        ", ".join(sorted(mine)), ", ".join(r["declaring"])])
    return out


def record(sat, cit, conflicts):
    count = lambda rows: collections.Counter(r["kind"] for r in rows)
    s, c = count(sat), count(cit)
    unresolved = [r for r in sat if r["kind"] != "rewritten"]
    return "\n\n".join([
        "## Label counts",
        table(["Namespace", "Labels", "Rewritten", "Ambiguous", "Orphan / unregistered"], [
            ["satisfies", len(sat), s["rewritten"], s["ambiguous"], s["orphan"]],
            ["cites", len(cit), c["rewritten"], 0, c["unregistered"]]]),
        "Owner distance of rewritten `satisfies` labels: " + ", ".join(
            f"{d} edge(s): {n}" for d, n in sorted(collections.Counter(
                r["owner_distance"] for r in sat if r["kind"] == "rewritten").items())) + ".",
        "## Unqualified satisfies labels",
        table(["Issue", "State", "Label", "Kind", "Reason"],
              [[r["issue"], r["state"], f"`{r['label']}`", r["kind"], r["reason"]] for r in unresolved]),
        "## Owners outside the labelled issue's membership",
        "The DAG decides the owner; each row's labelled issue carries a different membership label.",
        table(["Issue", "Label", "Owner", "Owner membership", "Issue membership", "Declaring containers @distance"],
              conflicts),
        "## Unregistered cites labels",
        table(["Issue", "State", "Label"],
              [[r["issue"], r["state"], f"`{r['label']}`"] for r in cit if r["kind"] != "rewritten"]),
    ]) + "\n"


def credit(issues, hard, containers, qualified):
    """Credited (container short, requirement) pairs, each with its crediting issues."""
    out = collections.defaultdict(set)
    for container in issues.values():
        short = container["id"][:SHORT]
        if issue_type(container) not in containers or not hard.get(short):
            continue
        for child in descendants(container, issues):
            for label in child.get("labels", []):
                if not label.startswith("satisfies:"):
                    continue
                value = label[len("satisfies:"):]
                scope, _, sid = value.rpartition("/")
                if (scope == short if qualified else not scope) and sid in hard[short]:
                    out[(short, sid)].add(child["id"][:SHORT])
    return out


def verify(root, migrated, hard, sat, containers):
    issues = load_issues(root)
    before = credit(issues, hard, containers, False)
    after = credit(load_issues(migrated), hard, containers, True)
    hard = {c: ids for c, ids in hard.items()
            if c in {i[:SHORT] for i, v in issues.items() if issue_type(v) in containers}}
    evaluated = {l.split(":", 1)[1] for i in issues.values() for l in i.get("labels", [])
                 if l.startswith("brackets:")}
    evaluated |= {i["id"][:SHORT] for i in issues.values() if "type:epic" in i.get("labels", [])}
    where = {(r["issue"], r["label"]): r for r in sat}
    changes = []
    for pair in sorted(set(before) | set(after)):
        if pair in after and pair in before:
            continue
        short, sid = pair
        resolved = collections.Counter()
        for child in before.get(pair, ()):
            row = where[(child, f"satisfies:{sid}")]
            resolved[row["replacement"].split(":")[1].split("/")[0] if row["replacement"] else row["kind"]] += 1
        changes.append([short, "yes" if short in evaluated else "no", sid,
                        "credited" if pair in before else "uncredited",
                        "credited" if pair in after else "uncredited",
                        ", ".join(f"{k} ×{n}" for k, n in sorted(resolved.items()))])
    covered = lambda credited: {c for c in hard if hard[c] <= {s for (cc, s) in credited if cc == c}}
    cb, ca = covered(before), covered(after)
    verdicts = [[c, "yes" if c in evaluated else "no", "covered" if c in cb else "uncovered",
                 "covered" if c in ca else "uncovered"] for c in sorted(cb ^ ca)]
    required = sum(len(v) for v in hard.values())
    return "\n\n".join([
        "## Coverage before and after",
        table(["Measure", "Before (unqualified)", "After (qualified)"], [
            ["Hard requirements", required, required],
            ["Credited", len(before), len(after)],
            ["Fully covered containers", len(cb), len(ca)]]),
        "Rule-evaluated: an epic (`hard-criteria-covered`) or a container named by a `brackets:` label (`coverage-preview`).",
        "### Container verdict changes",
        table(["Container", "Rule-evaluated", "Before", "After"], verdicts),
        "### Requirement credit changes",
        "Each row lists where the labels that credited the requirement before now resolve (owner short id or unresolved kind, with label count).",
        table(["Container", "Rule-evaluated", "Requirement", "Before", "After", "Former crediting labels"], changes),
    ]) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", default=".", help="repository root, read only")
    parser.add_argument("--jit", default="jit")
    parser.add_argument("--script", help="write the jit issue update script here")
    parser.add_argument("--record", help="write the markdown label record here")
    parser.add_argument("--json", help="write the per-label plan as JSON here")
    parser.add_argument("--verify", metavar="MIGRATED_ROOT")
    args = parser.parse_args()

    issues = load_issues(args.root)
    requirements = items(args.root, args.jit, "requirement")
    declared = collections.defaultdict(set)
    hard = collections.defaultdict(set)
    for item in requirements:
        declared[item["scope"]].add(item["self_id"])
        if re.sub(r"^\[[ xX]\]\s*", "", item["text"]).startswith("[hard]"):
            hard[item["scope"]].add(item["self_id"])
    containers, associations = hierarchy(args.root)
    sat = plan_satisfies(issues, declared, containers)
    if args.verify:
        print(verify(args.root, args.verify, hard, sat, containers), end="")
        return
    cit = plan_cites(issues, {i["self_id"] for i in items(args.root, args.jit, "citation")})
    if args.script:
        with open(args.script, "w") as handle:
            handle.write(script(sat + cit, issues))
        os.chmod(args.script, 0o755)
    if args.json:
        with open(args.json, "w") as handle:
            json.dump(sat + cit, handle, indent=1)
    text = record(sat, cit, membership_conflicts(sat, issues, associations))
    if args.record:
        with open(args.record, "w") as handle:
            handle.write(text)
    else:
        print(text, end="")


if __name__ == "__main__":
    main()
