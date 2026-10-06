#!/usr/bin/env python3
"""Write the committed holdout declaration of the crossover family.

Usage: make-holdout.py [output]   (default survey/holdout-cells.json)

The declaration is generated from `cells.py` in two shapes: the plan shape the
runner needs and the addendum shape the confirmation freezer appends. Both come
from one definition, so a holdout cell cannot be declared to the runner and to
the protocol with different sizes, seeds, arms or objectives.

The file is committed before the pilot runs. Its sizes appear in no pilot cell
and its arm directions are fixed here, so the holdout tests a threshold instead
of helping to choose one.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cells as grid  # noqa: E402

RATIONALE = (
    "Sixteen and five hundred and twelve elements appear in no pilot cell and "
    "their fixtures use seeds no pilot cell uses, so no sample of either cell "
    "took part in selecting a length threshold. The short cell's arms are "
    "swapped and its objective is non-regression: it asks whether the "
    "per-element dot product is not worse than the dispatched one where a gate "
    "would choose it. The long cell keeps the grid's direction and objective: "
    "it asks whether the dispatched dot product is materially faster where the "
    "established path is retained."
)


def main():
    output = (
        sys.argv[1]
        if len(sys.argv) > 1
        else os.path.join(os.path.dirname(os.path.abspath(__file__)), "holdout-cells.json")
    )
    declaration = {
        "schema": "53c5a8c0-holdout-declaration-v1",
        "family": grid.CROSSOVER_FAMILY,
        "rationale": RATIONALE,
        "cells": grid.HOLDOUT_CELLS,
        "addendum_cells": [
            grid.addendum_cell(cell, role="holdout") for cell in grid.HOLDOUT_CELLS
        ],
    }
    with open(output, "w") as handle:
        json.dump(declaration, handle, indent=2)
        handle.write("\n")
    print(f"{len(grid.HOLDOUT_CELLS)} holdout cells -> {output}", file=sys.stderr)


if __name__ == "__main__":
    main()
