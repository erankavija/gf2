#!/usr/bin/env python3
"""Write the campaign declaration of the decoder calibration (jit:0a77ecfc).

The cell count is the committed grid listing's; the imports are the owner
envelopes `IMPORTS` names.

Usage: make-campaign-declaration.py [--check]
"""

import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # a check writes nothing, however invoked

from locate import CALIBRATION, HERE, ISSUE, ROOT, emit, package, sha256, tracked

OUTPUT = HERE / "campaign-declaration.json"
GRID = HERE / "producer" / "coding-list-grid.json"
PROFILES = "data/tuning-profiles"
SECTIONS = {"core": "gf2-core/selectors", "algebra": "gf2-algebra/permanent"}
# The owners the executed campaign imports; its receipt pins the declaration's
# bytes, so later calibrated owners do not move them.
IMPORTS = {"core": "gf2-dbd8787d-20261001t230000z-2601601.json",
           "algebra": "gf2-a83583e0-20260930t230000z-2728298.json"}


def pinned(path):
    return {"path": path, "sha256": sha256(path)}


def imported(name, envelope):
    """The declaration entry importing the owner envelope at root-relative `envelope`."""
    owner = f"gf2-{name}"
    run = Path(envelope).name
    complete = [
        path for path in tracked(f"**/tuning-profiles/{run}")
        if set(SECTIONS.values()) <= set(json.loads((ROOT / path).read_text())["sections"])
    ]
    if len(complete) != 1:
        raise SystemExit(f"{len(complete)} complete envelopes of {run}; exactly one must exist")
    return {"owner": owner, "name": name, "section": SECTIONS[name],
            "envelope": pinned(envelope), "complete": pinned(complete[0])}


def main():
    cells = json.loads(json.loads(GRID.read_text())["evidence"])["cells"]
    declaration = {
        "schema": "tuning-campaign-declaration-v1",
        "issue": ISSUE,
        "protocol": f"{CALIBRATION}/premeasurement-protocol.md",
        "producing_manifest": f"{CALIBRATION}/producing-build-inputs.json",
        "measured_owners": [{"owner": "gf2-coding", "name": "coding", "cells": cells}],
        "imported_owners": [
            imported(name, f"{package(f'gf2-{name}')}/{PROFILES}/{envelope}")
            for name, envelope in IMPORTS.items()
        ],
        "evidence_index": True,
    }
    emit(OUTPUT, json.dumps(declaration, indent=2) + "\n", sys.argv[1:] == ["--check"],
         f"{CALIBRATION}/make-campaign-declaration.py")


if __name__ == "__main__":
    main()
