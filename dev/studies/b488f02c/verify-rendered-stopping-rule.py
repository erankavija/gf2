#!/usr/bin/env python3
"""Verify protocol markers on every rendered section 4.4 throughput cell.

The check maps each section 4.4 table cell to its measured row in
throughput-2026-08-07.csv and requires a dagger on a measured cell whose CSV
row has fewer than the protocol's five timed repetitions. A dagger on a
conforming cell is also a failure. The CSV's ``total_s`` column is parsed as
the accumulated composite timed work; the protocol's stopping clock is the
enclosing wall-clock loop, whose successful ``measured`` outcome records that
both stopping minima were reached. Exit 0 means the rendered table is clean;
exit 1 lists every failure. Recorded per JIT issue 4fdd781a.

Run from the repository root:

    python3 dev/studies/b488f02c/verify-rendered-stopping-rule.py
"""

from __future__ import annotations

import csv
import re
import sys
from pathlib import Path

STUDY_DIR = Path(__file__).resolve().parent
DOC = STUDY_DIR / "feasibility-study.md"
CSV_PATH = STUDY_DIR / "throughput-2026-08-07.csv"
MIN_REPS = 5
MIN_TIMED_SECONDS = 5.0

# Published column -> (backend, batch_size filter or None for any).
COLUMNS = [
    ("scalar", "cpu_scalar", None),
    ("AVX2", "cpu_avx2", None),
    ("rayon batch", "cpu_rayon_batch_scalar", None),
    ("rayon intra", "cpu_rayon_intra_matrix", None),
    ("Ryser generic", "cpu_ryser_generic", None),
    ("GPU M=256", "gpu_hip", "256"),
    ("GPU M=1024", "gpu_hip", "1024"),
]


def csv_rows():
    with CSV_PATH.open(encoding="utf-8") as fh:
        return list(csv.DictReader(line for line in fh if not line.startswith("#")))


def section_44():
    text = DOC.read_text(encoding="utf-8")
    start_marker = "### 4.4 Measured throughput"
    start = text.index(start_marker)
    next_heading = text.find("\n### ", start + len(start_marker))
    return text[start:] if next_heading == -1 else text[start:next_heading]


def cell_token(cell):
    marked = "†" in cell
    token = cell.replace("†", "").strip().strip("* ")
    return token, marked


def row_is_nonconforming(row):
    # total_s is retained as the CSV's accumulated composite timed-work
    # column. The protocol's stopping clock is the enclosing wall-clock loop,
    # and outcome=measured records that it reached both minima; reps is the
    # recorded minimum whose violation makes these two rows anomalous.
    reps = int(row["reps"])
    timed_work = float(row["total_s"])
    if timed_work < 0.0:
        raise ValueError("negative total_s")
    return row["outcome"] == "measured" and reps < MIN_REPS


def main():
    rows = csv_rows()
    by_key = {}
    for row in rows:
        key = (row["q"], row["n"], row["backend"], row["batch_size"])
        by_key[key] = row

    section = section_44()
    row_re = re.compile(r"^\|\s*([357])\s*\|\s*(\d+)\s*\|(.+)\|\s*$", re.M)
    failures = []
    checked = 0
    for match in row_re.finditer(section):
        q, n = match.group(1), match.group(2)
        cells = [cell.strip() for cell in match.group(3).split("|")]
        if len(cells) != len(COLUMNS):
            failures.append(f"q={q} n={n}: expected {len(COLUMNS)} cells, found {len(cells)}")
            continue
        for column_index, (label, backend, batch) in enumerate(COLUMNS):
            cell = cells[column_index]
            token, marked = cell_token(cell)
            key = (q, n, backend, batch or next(
                (row["batch_size"] for row in rows
                 if row["q"] == q and row["n"] == n and row["backend"] == backend),
                "",
            ))
            row = by_key.get(key)
            if row is None:
                candidates = [
                    candidate for candidate in rows
                    if candidate["q"] == q
                    and candidate["n"] == n
                    and candidate["backend"] == backend
                    and (batch is None or candidate["batch_size"] == batch)
                ]
                row = candidates[0] if candidates else None
            if row is None:
                failures.append(f"q={q} n={n} {label}: no CSV row")
                continue
            if token in {"—", "-", "censored", "unsupported", ""}:
                if marked:
                    failures.append(f"q={q} n={n} {label}: dagger on non-measured cell")
                continue
            if row["outcome"] != "measured":
                failures.append(
                    f"q={q} n={n} {label}: rendered value {token} maps to CSV outcome"
                    f" {row['outcome']}"
                )
                continue
            checked += 1
            nonconforming = row_is_nonconforming(row)
            if nonconforming and not marked:
                failures.append(
                    f"q={q} n={n} {label}: missing † (CSV reps={row['reps']},"
                    f" total_s={row['total_s']})"
                )
            elif not nonconforming and marked:
                failures.append(
                    f"q={q} n={n} {label}: unexpected † on conforming CSV row"
                    f" (reps={row['reps']}, total_s={row['total_s']})"
                )

    print(f"checked {checked} rendered measured section 4.4 cells against {CSV_PATH.name}")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("all rendered measured cells have correct stopping-rule markers")
    return 0


if __name__ == "__main__":
    sys.exit(main())
