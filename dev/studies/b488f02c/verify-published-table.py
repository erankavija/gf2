#!/usr/bin/env python3
"""Verify the published section 4.4 q=3 table cells of feasibility-study.md
against throughput-2026-08-07.csv at each cell's shown precision.

A published cell passes when the raw composite rate, rounded to the number of
significant figures the cell displays, reproduces the cell exactly. Exit 0
when every q=3 cell passes; exit 1 listing each mismatch. Recorded per JIT
issue a2c0db52 (REQ-02).

Run from the repository root:

    python3 dev/studies/b488f02c/verify-published-table.py
"""

import csv
import re
import sys
from pathlib import Path

STUDY_DIR = Path(__file__).resolve().parent
DOC = STUDY_DIR / "feasibility-study.md"
CSV_PATH = STUDY_DIR / "throughput-2026-08-07.csv"

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


def raw_rates():
    rates = {}
    with CSV_PATH.open(encoding="utf-8") as fh:
        rows = [line for line in fh if not line.startswith("#")]
    reader = csv.DictReader(rows)
    for row in reader:
        if row["q"] != "3" or row["outcome"] != "measured":
            continue
        key = (row["n"], row["backend"], row["batch_size"])
        rates[key] = float(row["composite_matrices_per_s"])
    return rates


def sig_figs(token):
    digits = re.sub(r"[^0-9]", "", token).lstrip("0")
    return len(digits)


def round_sig(value, figs):
    if value == 0:
        return "0"
    from decimal import Decimal

    quantised = float(f"%.{figs}g" % value)
    return f"{quantised:g}"


def main():
    rates = raw_rates()
    text = DOC.read_text(encoding="utf-8")
    start = text.index("### 4.4 Measured throughput")
    end = text.index("###", start + 1) if "###" in text[start + 1 :] else len(text)
    section = text[start : text.index("\n###", start + 1)] if "\n###" in text[start + 1 :] else text[start:]
    failures = []
    checked = 0
    row_re = re.compile(r"^\|\s*3\s*\|\s*(\d+)\s*\|(.+)\|\s*$", re.M)
    for match in row_re.finditer(section):
        n = match.group(1)
        cells = [c.strip() for c in match.group(2).split("|")]
        if len(cells) != len(COLUMNS):
            failures.append(f"n={n}: expected {len(COLUMNS)} cells, found {len(cells)}")
            continue
        for (label, backend, batch), cell in zip(COLUMNS, cells):
            token = cell.strip("* ")
            if token in {"—", "-", "censored", ""}:
                continue
            # The table separates digit groups with U+2009 thin spaces.
            shown = re.sub(r"[\s\u2009,]", "", token)
            figs = sig_figs(token)
            candidates = [
                rate
                for (rn, rb, rbatch), rate in rates.items()
                if rn == n and rb == backend and (batch is None or rbatch == batch)
            ]
            if not candidates:
                failures.append(f"n={n} {label}: no measured CSV row")
                continue
            raw = candidates[0]
            expect = round_sig(raw, figs).replace(",", "")
            got = f"{float(shown):g}"
            checked += 1
            if float(expect) != float(got):
                failures.append(
                    f"n={n} {label}: published {token} but CSV {raw} rounds to"
                    f" {expect} at {figs} significant figures"
                )
    print(f"checked {checked} published q=3 cells against {CSV_PATH.name}")
    if failures:
        for failure in failures:
            print(f"MISMATCH: {failure}")
        return 1
    print("all published q=3 cells reproduce at their shown precision")
    return 0


if __name__ == "__main__":
    sys.exit(main())
