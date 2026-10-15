#!/usr/bin/env python3
"""Largest confirmatory cell count a family's next ledger attempt admits under P-20 (jit:a91e7cef).

Creates the family ledger's genesis through the launcher, then evaluates the attempt budget of
`trial_ledger::attempt_alpha` and the tail-support test of `receipt.rs` at the shared settings of
the protocol document. The record quotes each source line it evaluates, and the recorder fails
when one is absent or the declaration names more cells than the bound.

Usage (from the worktree root): record-confirmatory-bound.py DECLARATION RECORD
"""

import importlib.util
import json
import pathlib
import re
import sys

sys.dont_write_bytecode = True


def load(name):
    path = pathlib.Path(__file__).resolve().with_name(name)
    module = importlib.util.module_from_spec(
        importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path))
    module.__spec__.loader.exec_module(module)
    return module


generator, launcher = load("make-family.py"), load("run-family-campaign.py")
smoke = generator.smoke
# The source lines the bound evaluates, by file; `floor` captures the tail-draw floor.
CITED = {
    "trial_ledger.rs": (
        r"let attempts = entries\.iter\(\)\.filter\(\|e\| e\.comparisons > 0\)\.count\(\)\.max\(1\) as f64;",
        r"Ok\(family\.family_wise\.alpha / \(attempts \* \(attempts \+ 1\.0\)\)\)",
    ),
    "receipt.rs": (
        r"corrected_alpha: attempt_alpha / f64::from\(comparisons\),",
        r"\|\| f64::from\(settings\.bootstrap_resamples\) \* corrected_alpha / 2\.0 < (?P<floor>[0-9.]+);",
    ),
}


def cited_lines():
    """Each cited line with its file, and the tail-draw floor the P-20 line compares with."""
    quoted, floor = [], None
    for name, patterns in CITED.items():
        path = smoke.located("file", name)
        text = (smoke.ROOT / path).read_text(encoding="utf-8")
        for pattern in patterns:
            match = re.search(pattern, text)
            if match is None:
                sys.exit(f"{path} no longer holds the line {pattern!r} the bound evaluates")
            quoted.append({"path": str(path), "verbatim": match[0]})
            floor = float(match["floor"]) if "floor" in match.groupdict() else floor
    return quoted, floor


def bound(alpha, resamples, floor, attempt):
    """Largest m whose corrected tails hold `floor` draws, in the evaluator's f64 order."""
    attempt_alpha = alpha / (attempt * (attempt + 1.0))
    m = 0
    while not float(resamples) * (attempt_alpha / (m + 1)) / 2.0 < floor:
        m += 1
    return attempt_alpha, m


def record(declaration_path, out):
    declaration = json.loads(pathlib.Path(declaration_path).read_text(encoding="utf-8"))
    ledger_path = declaration["family_wise"]["ledger_path"]
    ledger = smoke.ROOT / ledger_path
    launcher.genesis(ledger)
    reservations = [json.loads(line) for line in ledger.read_text(encoding="utf-8").splitlines()]
    spending = sum(1 for reservation in reservations if reservation["comparisons"] > 0)
    addendum_schema = generator.schema()
    settings = generator.shared_settings(addendum_schema)
    protocol = f"{generator.protocol_document(addendum_schema)}, {generator.SHARED_SECTION[3:]}"
    quoted, floor = cited_lines()
    attempt = spending + 1
    attempt_alpha, admitted = bound(settings["family_alpha"], settings["bootstrap_resamples"],
                                    floor, attempt)
    declared = len(declaration["cells"])
    if declared > admitted:
        sys.exit(f"the declaration names {declared} cells; the attempt admits {admitted}")
    document = {
        "family": declaration["family"]["id"],
        "declaration": str(generator.repository_path(pathlib.Path(declaration_path).resolve())),
        "ledger": {"path": ledger_path, "reservations": len(reservations),
                   "spending_reservations": spending},
        "read": {
            "family_alpha": {"value": settings["family_alpha"], "source": protocol},
            "bootstrap_resamples": {"value": settings["bootstrap_resamples"], "source": protocol},
            "tail_draws": {"value": floor, "source": quoted[-1]["path"]},
        },
        "cited": quoted,
        "attempt": {"value": attempt, "kind": "derived",
                    "formula": "spending_reservations + 1, this attempt's own reservation"},
        "attempt_alpha": {"value": attempt_alpha, "kind": "derived",
                          "formula": "family_alpha / (attempt * (attempt + 1))"},
        "max_confirmatory_cells": {
            "value": admitted, "kind": "derived",
            "formula": "largest m with bootstrap_resamples * (attempt_alpha / m) / 2 >= tail_draws",
        },
        "declared_cells": {"value": declared, "kind": "descriptive",
                           "source": "cells of the declaration"},
    }
    generator.write(pathlib.Path(out), document)
    return out


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    print(record(*sys.argv[1:]))


if __name__ == "__main__":
    main()
