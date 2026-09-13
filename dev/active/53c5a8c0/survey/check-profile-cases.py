#!/usr/bin/env python3
"""Check every profile row's case against the frozen cell grid (jit:53c5a8c0).

Usage: check-profile-cases.py <profile-cases.tsv>

A profile row names a cell and carries the case JSON it drives. The case must be
the workload and seed `cells.py` freezes for that cell, so a profile row cannot
drift into a size or a seed no addendum declares. The check compares the row's
decoded case against the cell's own case dictionary and reports every mismatch
before any run starts.
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import cells  # noqa: E402

GRID = {
    cell["cell_id"]: cell["case"]
    for group in (cells.CROSSOVER_CELLS, cells.POLYNOMIAL_CELLS, cells.HOLDOUT_CELLS)
    for cell in group
}


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    problems = []
    rows = 0
    for number, line in enumerate(pathlib.Path(sys.argv[1]).read_text().splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 6:
            problems.append(f"line {number}: {len(fields)} columns, expected 6")
            continue
        label, cell, _build, _arm, _path, case = fields
        rows += 1
        frozen = GRID.get(cell)
        if frozen is None:
            problems.append(f"{label}: cell {cell!r} is in no frozen family or holdout")
            continue
        try:
            driven = json.loads(case)
        except json.JSONDecodeError as error:
            problems.append(f"{label}: case does not decode: {error}")
            continue
        if driven != frozen:
            problems.append(
                f"{label}: case {json.dumps(driven, sort_keys=True)} is not the "
                f"frozen case of {cell}, {json.dumps(frozen, sort_keys=True)}"
            )
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        raise SystemExit(1)
    print(f"{rows} profile rows carry the frozen case of the cell they name")


if __name__ == "__main__":
    main()
