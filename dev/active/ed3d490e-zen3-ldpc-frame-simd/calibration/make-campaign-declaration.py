#!/usr/bin/env python3
"""Write the campaign declaration of the decoder calibration (jit:0a77ecfc).

The cell count is the committed grid listing's; the imports are the core owner
the baked constants pin and the one calibrated algebra owner.

Usage: make-campaign-declaration.py [--check]
"""

import json
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # a check writes nothing, however invoked

from locate import CALIBRATION, HERE, ISSUE, ROOT, emit, package, sha256, tracked

OUTPUT = HERE / "campaign-declaration.json"
GRID = HERE / "producer" / "coding-list-grid.json"
PROFILES = "data/tuning-profiles"
SECTIONS = {"core": "gf2-core/selectors", "algebra": "gf2-algebra/permanent"}


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
    core = package("gf2-core")
    baked = re.search(rf"{PROFILES}/(gf2-[a-z0-9-]+\.json)",
                      (ROOT / core / "src/tuning/baked.rs").read_text())
    if not baked:
        raise SystemExit("the baked core constants pin no measured owner")
    algebra = tracked(f"{package('gf2-algebra')}/{PROFILES}/gf2-*.json")
    if len(algebra) != 1:
        raise SystemExit(f"{len(algebra)} calibrated algebra owners; exactly one must exist")
    declaration = {
        "schema": "tuning-campaign-declaration-v1",
        "issue": ISSUE,
        "protocol": f"{CALIBRATION}/premeasurement-protocol.md",
        "producing_manifest": f"{CALIBRATION}/producing-build-inputs.json",
        "measured_owners": [{"owner": "gf2-coding", "name": "coding", "cells": cells}],
        "imported_owners": [
            imported("core", f"{core}/{PROFILES}/{baked.group(1)}"),
            imported("algebra", algebra[0]),
        ],
        "evidence_index": True,
    }
    emit(OUTPUT, json.dumps(declaration, indent=2) + "\n", sys.argv[1:] == ["--check"],
         f"{CALIBRATION}/make-campaign-declaration.py")


if __name__ == "__main__":
    main()
