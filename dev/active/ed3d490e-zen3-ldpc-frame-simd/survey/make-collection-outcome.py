#!/usr/bin/env python3
"""Per-cell outcome table of one confirmation receipt (jit:32bfe9f8).

Reads the receipt's acceptance summary, its plan and the addendum snapshot the receipt pins, and
writes one row per summary cell: role and objective from the addendum, arms from the plan,
decision and outcome as the evaluator records them, and the interpretation INTERPRETATION
declares for the cell's objective and outcome. When the addendum declares holdout cells, a
second table names each cell's baseline and candidate builds from the plan's arms and states for
each holdout cell whether the calibrated profile holds on the held-out inputs, which it does
exactly when the recorded outcome is `pass`. The receipt directory is the one named by
CAMPAIGN_ID among the repository's receipts.

Usage (from the worktree root): make-collection-outcome.py CAMPAIGN_ID OUT
                                make-collection-outcome.py --self-test
"""

import functools
import importlib.util
import json
import os
import pathlib
import sys
import unittest

sys.dont_write_bytecode = True


def load(path):
    module = importlib.util.module_from_spec(importlib.util.spec_from_file_location(
        path.stem.replace("-", "_"), path))
    module.__spec__.loader.exec_module(module)
    return module


SURVEY = pathlib.Path(__file__).resolve().parent
generator = load(SURVEY / "make-family.py")
smoke = generator.smoke
TITLE = "# Outcome per cell of `{}`"
# What each recorded outcome says about the plan's candidate arm against its baseline arm, by the
# cell objective (protocol, *Decision rule*): a comparator-gap cell's `improved` is a material gap
# in the candidate's favour.
INTERPRETATION = {
    ("improvement", "pass"): "candidate meets the improvement margin",
    ("improvement", "not-material"): "candidate not worse, below the improvement margin",
    ("improvement", "fail"): "candidate regressed",
    ("improvement", "inconclusive"): "interval spans a margin",
    ("non-regression", "pass"): "candidate not regressed",
    ("non-regression", "fail"): "candidate regressed",
    ("non-regression", "inconclusive"): "interval spans a margin",
    ("comparator-gap", "pass"): "candidate faster by a material gap",
    ("comparator-gap", "not-material"): "no material gap established",
    ("comparator-gap", "fail"): "baseline faster beyond the equivalence margin",
    ("comparator-gap", "inconclusive"): "interval spans a margin",
    ("improvement", "pilot"): "exploratory cell, no decision",
    ("non-regression", "pilot"): "exploratory cell, no decision",
    ("comparator-gap", "pilot"): "exploratory cell, no decision",
}
HOLDOUT = "holdout"


def read(path):
    return json.loads(path.read_bytes())


@functools.cache
def receipt_directories():
    files = generator.repository_files()
    return tuple(smoke.ROOT / pathlib.Path(path).parent
                 for path in files.tracked_files(smoke.ROOT, "receipt.json"))


def receipt_directory(campaign):
    found = [directory for directory in receipt_directories() if directory.name == campaign]
    if len(found) != 1:
        sys.exit(f"{len(found)} receipt directories are called {campaign}; exactly one must be")
    return found[0]


def link(place, target):
    return pathlib.Path(os.path.relpath(target, place.parent)).as_posix()


def outcomes(directory):
    """The summary, the plan and, per summary cell in summary order, the cell's addendum
    declaration, plan entry, summary entry and interpretation."""
    summary = read(directory / "acceptance-summary.json")
    pinned = read(directory / "receipt.json")["addendum"]
    declared = {cell["cell_id"]: cell for cell in read(directory / pinned["snapshot"])["cells"]}
    plan = read(directory / "plan.json")
    planned = {cell["cell_id"]: cell for cell in plan["cells"]}
    cells = []
    for cell in summary["cells"]:
        identity = cell["cell_id"]
        objective = declared[identity]["objective"]
        if (objective, cell["outcome"]) not in INTERPRETATION:
            raise KeyError(f"{identity}: no interpretation of outcome {cell['outcome']} "
                           f"for objective {objective}")
        cells.append({"declared": declared[identity], "planned": planned[identity],
                      "summary": cell,
                      "interpretation": INTERPRETATION[(objective, cell["outcome"])]})
    return summary, plan, cells


def render(directory, place):
    """The outcome table of the receipt in `directory`, written to be read at `place`."""
    summary, plan, cells = outcomes(directory)
    pinned = read(directory / "receipt.json")["addendum"]
    declared = {cell["summary"]["cell_id"]: cell["declared"] for cell in cells}
    planned = {cell["summary"]["cell_id"]: cell["planned"] for cell in cells}
    rows = [[
        f"`{cell['summary']['cell_id']}`", cell["declared"]["role"],
        cell["declared"]["objective"], cell["summary"]["core_arm"],
        f"`{cell['planned']['baseline_arm']}`", f"`{cell['planned']['candidate_arm']}`",
        f"`{cell['summary']['decision']}`", f"`{cell['summary']['outcome']}`",
        cell["interpretation"],
    ] for cell in cells]
    header = ["Cell", "Role", "Objective", "Core arm", "Baseline arm", "Candidate arm",
              "Decision", "Outcome", "Interpretation"]
    lines = [
        TITLE.format(summary["campaign_id"]), "",
        "> **Diátaxis Type:** Reference", "",
        f"Generated by [`{pathlib.Path(__file__).name}`]({link(place, pathlib.Path(__file__))}) "
        f"from the [acceptance summary]({link(place, directory / 'acceptance-summary.json')}), "
        f"the [plan]({link(place, directory / 'plan.json')}) and the "
        f"[addendum snapshot]({link(place, directory / pinned['snapshot'])}) "
        "the receipt pins; from the worktree root, "
        f"`{pathlib.Path(__file__).name} {summary['campaign_id']} "
        f"{generator.repository_path(place)}` regenerates it.", "",
        f"Source: verdict `{summary['verdict']}`, qualifies `{str(summary['qualifies']).lower()}`, "
        f"{len(summary['findings'])} findings.", "",
        *table(header, rows),
    ]
    if any(declared[cell["cell_id"]]["role"] == HOLDOUT for cell in summary["cells"]):
        builds = []
        for cell in summary["cells"]:
            identity, role = cell["cell_id"], declared[cell["cell_id"]]["role"]
            arms = (plan["arms"][planned[identity][side]]["build"]
                    for side in ("baseline_arm", "candidate_arm"))
            holds = ("holds" if cell["outcome"] == "pass" else "does not hold") \
                if role == HOLDOUT else "not a holdout cell"
            builds.append([f"`{identity}`", role, *(f"`{build}`" for build in arms),
                           f"`{cell['outcome']}`", holds])
        lines += ["", "## Builds and holdout", "", *table(
            ["Cell", "Role", "Baseline build", "Candidate build", "Outcome",
             "Profile on held-out inputs"], builds)]
    return "\n".join(lines) + "\n"


def table(header, rows):
    return ["| " + " | ".join(header) + " |", "|" + "---|" * len(header),
            *("| " + " | ".join(row) + " |" for row in rows)]


def write(path, text):
    partial = path.with_name(f".{path.name}.partial")
    partial.write_text(text, encoding="utf-8")
    partial.replace(path)


def main():
    if sys.argv[1:] == ["--self-test"]:
        tests = load(SURVEY / "test_make_collection_outcome.py")
        result = unittest.TextTestRunner().run(
            unittest.defaultTestLoader.loadTestsFromModule(tests))
        sys.exit(0 if result.wasSuccessful() else 1)
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    out = pathlib.Path(sys.argv[2]).resolve()
    write(out, render(receipt_directory(sys.argv[1]), out))
    print(generator.repository_path(out))


if __name__ == "__main__":
    main()
