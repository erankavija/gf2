#!/usr/bin/env python3
"""Record the refused freeze of the sparse product check-cost confirmation (jit:bfc2ceab).

Usage: make-spmm-freeze-refusal.py, with no arguments

Runs the family launcher's `freeze` action and writes, in the directory above
this script, how the action ends beside the digests of what it reads. Fails,
before writing, when the action freezes an addendum.
"""

import json
import subprocess
import sys

import review
import spmm_family
from locate import HERE, ROOT, repository_files

LAUNCHER = "run-spmm-check.sh"
# The canonical freezer, by its name and opening among the same-named scripts.
FREEZER = ("freeze-confirmation.py",
           b'#!/usr/bin/env python3\n"""Freeze a confirmation addendum from its accepted pilot.')


def pinned(relative):
    return {"path": relative, "sha256": spmm_family.sha256(ROOT / relative)}


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    family = spmm_family.FAMILY
    confirmation = family / review.GRAIN_FAMILY_CONFIRMATION
    if confirmation.exists():
        raise SystemExit(f"{confirmation.name} exists: the confirmation is frozen")
    launcher = repository_files.live_file(ROOT, LAUNCHER)
    command = [launcher, "freeze"]
    run = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if run.returncode == 0 or confirmation.exists():
        raise SystemExit(f"`{' '.join(command)}` froze {confirmation.name}; nothing is refused")
    addendum = spmm_family.pilot_addendum()
    path, entries = spmm_family.ledger()
    stages = spmm_family.stages()
    admitted = addendum["search_budget"]["max_pilot_trials_per_cell"]
    trials = [(label, directory) for label, directory, receipt in stages
              if receipt["label"] == "pilot"]
    record = {
        "schema": "spmm-freeze-refusal-v1",
        "generator": str((HERE / "make-spmm-freeze-refusal.py").relative_to(ROOT)),
        "family": addendum["family"]["id"],
        "command": command,
        "exit_status": run.returncode,
        "standard_output": run.stdout.splitlines(),
        "standard_error": run.stderr.splitlines(),
        "confirmation_addendum_written": False,
        "launcher": pinned(launcher),
        "freezer": pinned(repository_files.document(ROOT, *FREEZER)),
        "pilot_addendum": pinned(str((family / review.GRAIN_FAMILY_ADDENDUM).relative_to(ROOT))),
        "ledger": {**pinned(str(path.relative_to(ROOT))), "entries": len(entries)},
        "pilot_trials": [
            {"stage": label,
             **pinned(str((directory / "receipt.json").relative_to(ROOT)))}
            for label, directory in trials
        ],
        "pilot_trials_admitted_per_cell": admitted,
        "pilot_trials_left": admitted - len(trials),
    }
    output = family / review.GRAIN_FAMILY_REFUSAL
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"{output.relative_to(ROOT)}: exit {run.returncode}, {run.stderr.strip()}")


if __name__ == "__main__":
    main()
