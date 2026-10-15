#!/usr/bin/env python3
"""Largest confirmatory cell count a family's next ledger attempt admits under P-20 (jit:a91e7cef).

Creates the family ledger's genesis through the launcher and records the next attempt the shared
evaluator derives from that ledger, `benchmark-acceptance --next-attempt`. The recorder fails when
the declaration names more cells than that attempt admits.

Usage (from the worktree root): record-confirmatory-bound.py DECLARATION RECORD
"""

import importlib.util
import json
import pathlib
import subprocess
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
EVALUATOR = "benchmark-acceptance"


def next_attempt(ledger, family):
    runner, environment = smoke.build_runner(EVALUATOR)
    evaluator = pathlib.Path(runner).with_name(EVALUATOR)
    return json.loads(subprocess.run([evaluator, "--next-attempt", ledger, family], check=True,
                                     env=environment, capture_output=True, text=True).stdout)


def record(declaration_path, out):
    declaration = json.loads(pathlib.Path(declaration_path).read_text(encoding="utf-8"))
    ledger_path = declaration["family_wise"]["ledger_path"]
    ledger = smoke.ROOT / ledger_path
    launcher.genesis(ledger)
    evaluated = next_attempt(ledger, declaration["family"]["id"])
    attempt = evaluated["next_attempt"]
    declared = len(declaration["cells"])
    if declared > attempt["max_cells"]:
        sys.exit(f"the declaration names {declared} cells; the attempt admits "
                 f"{attempt['max_cells']}")
    document = {
        "family": declaration["family"]["id"],
        "declaration": str(generator.repository_path(pathlib.Path(declaration_path).resolve())),
        "ledger": {"path": ledger_path,
                   **{key: attempt[key] for key in ("reservations", "spending_reservations",
                                                    "prior_comparisons")}},
        "read": {
            "family_alpha": {"value": evaluated["family_alpha"],
                             "source": "protocol::SHARED_SETTINGS"},
            "bootstrap_resamples": {"value": evaluated["bootstrap_resamples"],
                                    "source": "protocol::SHARED_SETTINGS"},
            "tail_draws": {"value": evaluated["min_tail_draws"],
                           "source": "receipt::MIN_TAIL_DRAWS"},
        },
        "computed_by": f"{EVALUATOR} --next-attempt, trial_ledger::next_attempt",
        "attempt": {"value": attempt["attempt"], "kind": "derived",
                    "formula": "spending_reservations + 1, this attempt's own reservation"},
        "attempt_alpha": {"value": attempt["attempt_alpha"], "kind": "derived",
                          "formula": "family_alpha / (attempt * (attempt + 1))"},
        "max_confirmatory_cells": {
            "value": attempt["max_cells"], "kind": "derived",
            "formula": "largest m with bootstrap_resamples * (attempt_alpha / "
                       "(prior_comparisons + m)) / 2 >= tail_draws",
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
