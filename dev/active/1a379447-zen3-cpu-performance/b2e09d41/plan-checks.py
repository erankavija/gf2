#!/usr/bin/env python3
"""Runs `benchmark-ab-runner check` over every committed campaign plan (jit:b2e09d41).

Writes, beside this script:

- `plan-checks.tsv`: one row per committed plan with the shared check's
  verdict and the verdicts `reference-observations.json` holds for the same
  plan bytes;
- `mutation-checks.tsv`: one row per single-field addendum mutation on which
  two reference executables disagree, with the shared check's verdict.

`reference-observations.json` is an observation record: it holds what the
reference executables named on the command line reported, each identified by
the SHA-256 of its executable and of its source file. A run that names no
reference reads that record and never rewrites it. Plans and mutations are
matched to it by content digest.

A reference is a superseded plan checker, called as `EXECUTABLE PLAN`, or
another edition of the shared runner, called as `EXECUTABLE check PLAN`. The
run fails when the shared check rejects a plan a superseded checker accepts.

Usage (from any directory of the checkout, outside a benchmark window):
  plan-checks.py [--superseded LABEL EXECUTABLE SOURCE]...
                 [--runner-edition LABEL EXECUTABLE SOURCE]...
"""

import argparse
import copy
import hashlib
import json
import pathlib
import subprocess
import sys

PLAN_SCHEMA = "zen3-benchmark-plan-v1"
RUNNER_PACKAGE = "tuning-campaign-support"
RUNNER = "benchmark-ab-runner"
# The plan whose addendum the mutation sweep perturbs.
MUTATION_BASE = "07ca8585-v4-r1-update-single-worker-pilot"
STRING_VALUES = ["", " ", "x"]
OBSERVATIONS_SCHEMA = "plan-check-reference-observations-v1"
SUPERSEDED = "superseded"
RUNNER_EDITION = "runner-edition"

HERE = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path(subprocess.run(
    ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
    check=True, capture_output=True, text=True).stdout.strip())
OBSERVATIONS = HERE / "reference-observations.json"


def helper():
    """The repository-file helper module, found among the files git lists."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", ":(glob)**/repository_files.py"],
        check=True, capture_output=True, text=True).stdout.split()
    (path,) = listing
    sys.path.insert(0, str((ROOT / path).parent))
    import repository_files
    return repository_files


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def committed_plans(files):
    """Root-relative paths of the committed plans, snapshot copies excluded."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-z", "--", "*.json"],
        check=True, capture_output=True, text=True).stdout.split("\0")
    plans = []
    for path in sorted(filter(None, listing)):
        if files.is_snapshot_copy(path):
            continue
        data = (ROOT / path).read_bytes()
        if PLAN_SCHEMA.encode() not in data[:4096]:
            continue
        if json.loads(data).get("schema") == PLAN_SCHEMA:
            plans.append(path)
    return plans


def build_runner():
    """Builds the shared runner and returns its executable path."""
    build = subprocess.run(
        ["./scripts/cargo-budget.sh", "cargo", "+1.95", "build", "--offline", "--release",
         "-p", RUNNER_PACKAGE, "--bin", RUNNER, "--message-format", "json"],
        cwd=ROOT, check=True, capture_output=True, text=True)
    for line in build.stdout.splitlines():
        if not line.startswith("{"):
            continue
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message["target"]["name"] == RUNNER \
                and message.get("executable"):
            return message["executable"]
    raise SystemExit(f"cargo reported no {RUNNER} executable")


def verdict(command):
    """`accept` or `reject` with the first diagnostic line, root-independent."""
    run = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if run.returncode == 0:
        return {"verdict": "accept", "message": ""}
    lines = run.stderr.strip().splitlines()
    message = lines[0] if lines else ""
    return {"verdict": "reject", "message": message.replace(str(ROOT) + "/", "")}


def call(role, executable, plan):
    if role == RUNNER_EDITION:
        return [executable, "check", str(plan)]
    return [executable, str(plan)]


def leaves(node, trail=()):
    """Scalar leaves of a JSON document; the first element stands for a list."""
    if isinstance(node, dict):
        for key, value in node.items():
            yield from leaves(value, trail + (key,))
    elif isinstance(node, list):
        for index, value in enumerate(node[:1]):
            yield from leaves(value, trail + (index,))
    else:
        yield trail, node


def mutations(addendum):
    """Every single-leaf mutation of the sweep as (pointer, replacement)."""
    for trail, value in leaves(addendum):
        if isinstance(value, bool) or value is None:
            continue
        values = STRING_VALUES if isinstance(value, str) else [0, -1, value * 1000]
        for replacement in values:
            if replacement != value:
                yield "/" + "/".join(map(str, trail)), replacement


class MutationStage:
    """Writes one mutated addendum and a plan naming it into a build-output directory."""

    def __init__(self, plans):
        (base,) = [path for path in plans
                   if json.loads((ROOT / path).read_text())["campaign_id"] == MUTATION_BASE]
        self.plan = json.loads((ROOT / base).read_text())
        self.addendum = json.loads((ROOT / self.plan["addendum"]).read_text())
        self.base_sha256 = sha256((ROOT / self.plan["addendum"]).read_bytes())
        target = json.loads(subprocess.run(
            ["cargo", "metadata", "--offline", "--no-deps", "--format-version", "1"],
            cwd=ROOT, check=True, capture_output=True, text=True).stdout)["target_directory"]
        self.directory = pathlib.Path(target) / "b2e09d41-mutations"
        self.directory.mkdir(parents=True, exist_ok=True)

    def write(self, pointer, replacement):
        mutated = copy.deepcopy(self.addendum)
        node = mutated
        *parents, last = pointer.strip("/").split("/")
        for key in parents:
            node = node[int(key)] if isinstance(node, list) else node[key]
        node[int(last) if isinstance(node, list) else last] = replacement
        addendum = self.directory / "addendum.json"
        addendum.write_text(json.dumps(mutated, indent=2) + "\n")
        plan = self.directory / "plan.json"
        plan.write_text(json.dumps(dict(self.plan, addendum=str(addendum)), indent=2) + "\n")
        return plan


def observe(references, plans, stage):
    """Records what each reference executable reports for every plan and mutation."""
    record = {
        "schema": OBSERVATIONS_SCHEMA,
        "references": {
            label: {
                "role": role,
                "executable_sha256": sha256(pathlib.Path(executable).read_bytes()),
                "source_name": pathlib.Path(source).name,
                "source_sha256": sha256(pathlib.Path(source).read_bytes()),
            }
            for role, label, executable, source in references
        },
        "plans": {},
        "mutation_base": {"campaign_id": MUTATION_BASE, "addendum_sha256": stage.base_sha256},
        "mutations": [],
    }
    for path in plans:
        record["plans"][sha256((ROOT / path).read_bytes())] = {
            label: verdict(call(role, executable, path))
            for role, label, executable, _ in references
        }
    for pointer, replacement in mutations(stage.addendum):
        plan = stage.write(pointer, replacement)
        observed = {label: verdict(call(role, executable, plan))
                    for role, label, executable, _ in references}
        if len({entry["verdict"] for entry in observed.values()}) > 1:
            record["mutations"].append(
                {"pointer": pointer, "replacement": replacement, "observed": observed})
    OBSERVATIONS.write_text(json.dumps(record, indent=2) + "\n")


def table(path, header, rows):
    path.write_text("".join("\t".join(row) + "\n" for row in [header, *rows]))


def main():
    parser = argparse.ArgumentParser()
    for role in (SUPERSEDED, RUNNER_EDITION):
        parser.add_argument(f"--{role}", nargs=3, action="append", default=[],
                            metavar=("LABEL", "EXECUTABLE", "SOURCE"))
    args = parser.parse_args()
    references = [(SUPERSEDED, *entry) for entry in args.superseded] \
        + [(RUNNER_EDITION, *entry) for entry in args.runner_edition]
    plans = committed_plans(helper())
    stage = MutationStage(plans)
    if references:
        observe(references, plans, stage)
    observations = json.loads(OBSERVATIONS.read_text())
    labels = sorted(observations["references"])
    superseded = [label for label in labels
                  if observations["references"][label]["role"] == SUPERSEDED]
    runner = build_runner()

    contradictions = []
    rows = []
    for path in plans:
        data = (ROOT / path).read_bytes()
        plan = json.loads(data)
        addendum_path = ROOT / plan["addendum"]
        addendum = json.loads(addendum_path.read_text()) if addendum_path.is_file() else {}
        shared = verdict([runner, "check", path])
        observed = observations["plans"].get(sha256(data), {})
        for label in superseded:
            if observed.get(label, {}).get("verdict") == "accept" and shared["verdict"] != "accept":
                contradictions.append(f"{path}: {label} accepts, the shared check rejects")
        rows.append([
            path, sha256(data), addendum.get("schema", ""),
            addendum.get("family", {}).get("purpose", ""), shared["verdict"],
            *[observed.get(label, {}).get("verdict", "unobserved") for label in labels],
            shared["message"],
        ])
    table(HERE / "plan-checks.tsv",
          ["plan", "plan_sha256", "addendum_schema", "family_purpose", "shared_check",
           *labels, "shared_check_message"], rows)

    if observations["mutation_base"]["addendum_sha256"] != stage.base_sha256:
        raise SystemExit("the mutation base addendum differs from the observed one")
    rows = []
    for mutation in observations["mutations"]:
        shared = verdict([runner, "check",
                          str(stage.write(mutation["pointer"], mutation["replacement"]))])
        rows.append([
            mutation["pointer"], json.dumps(mutation["replacement"]), shared["verdict"],
            *[mutation["observed"][label]["verdict"] for label in labels],
        ])
    table(HERE / "mutation-checks.tsv",
          ["addendum_pointer", "replacement", "shared_check", *labels], rows)

    for line in contradictions:
        print(line, file=sys.stderr)
    return 1 if contradictions else 0


if __name__ == "__main__":
    sys.exit(main())
