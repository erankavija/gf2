#!/usr/bin/env python3
"""Verify protocol markers on every rendered anomalous throughput rate.

The study states that each cell repeats until it has both at least five
repetitions and at least five seconds of timed work, with a 120-second cap
(feasibility-study.md:338-341). The pinned harness source is identified by
the CSV's ``# harness_source_sha:`` comment and is read with
``git show <sha>:dev/research/permanent-sampling-feas/src/protocol.rs``. Its
timed loop uses the loop's elapsed wall-clock as the stopping clock and exits
only when ``reps >= MIN_REPS && elapsed >= MIN_TIMED_SECONDS`` or the cap
binds (protocol.rs:168-185 in the current layout; constants at 100-106).

The CSV has no wall-clock column. ``total_s`` is the sum of the four
component spans (generate/evaluate/reduce/store), which undercounts the
timed loop's elapsed wall-clock (protocol.rs:435-455). Therefore
``total_s >= 5`` directly proves the five-second minimum, while a conforming
row may legitimately show slightly less than five; rows below five rely on
the source-pinned exit structure. The five-repetition half is checked
row-by-row from the CSV. A measured row below five repetitions must also have
``total_s >= 120`` as evidence that the cap path, rather than an early exit,
produced it. A successful ``measured`` outcome alone does not prove both
minima, because capped nonconforming rows also have that outcome.

The check maps each section 4.4 table cell to its measured row in
throughput-2026-08-07.csv and requires a dagger on a measured cell whose CSV
row has fewer than the protocol's five repetitions. It also scans the whole
study for every rendered rate token derived from such a row. A dagger on a
conforming cell is also a failure. Exit 0 means the rendered document is
clean; exit 1 lists every failure. Recorded per JIT issue 4fdd781a.

Run from the repository root:

    python3 dev/studies/b488f02c/verify-rendered-stopping-rule.py
"""

from __future__ import annotations

import csv
import math
import re
import subprocess
import sys
from pathlib import Path

STUDY_DIR = Path(__file__).resolve().parent
DOC = STUDY_DIR / "feasibility-study.md"
CSV_PATH = STUDY_DIR / "throughput-2026-08-07.csv"
MIN_REPS = 5
MIN_TIMED_SECONDS = 5.0
MAX_CELL_SECONDS = 120.0
HARNESS_SOURCE_PATH = "dev/research/permanent-sampling-feas/src/protocol.rs"


class CheckFailure(ValueError):
    """A protocol fact required by this check cannot be established."""


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


def section_44(text):
    start_marker = "### 4.4 Measured throughput"
    start = text.index(start_marker)
    next_heading = text.find("\n### ", start + len(start_marker))
    return text[start:] if next_heading == -1 else text[start:next_heading]


def cell_token(cell):
    marked = "†" in cell
    token = cell.replace("†", "").strip().strip("* ")
    return token, marked


def row_is_nonconforming(row):
    # The timed minimum is verified separately: total_s can undercount the
    # enclosing wall-clock loop, while reps is the CSV-visible minimum whose
    # violation makes a measured row nonconforming.
    reps = int(row["reps"])
    timed_work = float(row["total_s"])
    if not math.isfinite(timed_work) or timed_work < 0.0:
        raise CheckFailure("invalid total_s")
    return row["outcome"] == "measured" and reps < MIN_REPS


def _constant_value(source, name):
    literal = r"(?:\d+(?:\.\d*)?|\.\d+)(?:_?f(?:32|64))?"
    match = re.search(
        rf"^\s*(?:pub\s+)?const\s+{name}\s*:[^=;]+="
        rf"\s*({literal})\s*;",
        source,
        re.M,
    )
    if match is None:
        raise CheckFailure(f"pinned harness does not define {name}")
    numeric = re.sub(r"_?f(?:32|64)$", "", match.group(1))
    return float(numeric)


def _verify_harness_source(source, sha):
    try:
        reps = _constant_value(source, "MIN_REPS")
        timed_seconds = _constant_value(source, "MIN_TIMED_SECONDS")
        max_seconds = _constant_value(source, "MAX_CELL_SECONDS")
    except CheckFailure as exc:
        raise CheckFailure(
            f"five-second minimum can no longer be verified: {exc}"
            f" (harness source {sha})"
        ) from exc

    if reps != MIN_REPS or timed_seconds != MIN_TIMED_SECONDS or max_seconds != MAX_CELL_SECONDS:
        raise CheckFailure(
            "five-second minimum can no longer be verified: pinned harness "
            f"constants are not MIN_REPS={MIN_REPS}, "
            f"MIN_TIMED_SECONDS={MIN_TIMED_SECONDS}, "
            f"MAX_CELL_SECONDS={MAX_CELL_SECONDS} (harness source {sha})"
        )

    minimums = (
        r"(?:\blet\s+minimums_met\s*=\s*)?\(?\s*"
        r"(?:\bresult\.)?reps\s*>=\s*MIN_REPS\s*"
        r"&&\s*elapsed\s*>=\s*MIN_TIMED_SECONDS\s*\)?"
    )
    if re.search(minimums, source) is None:
        raise CheckFailure(
            "five-second minimum can no longer be verified: pinned harness "
            f"is missing the minimums_met conjunction (harness source {sha})"
        )
    if re.search(r"(?:\blet\s+capped\s*=\s*)?elapsed\s*>=\s*MAX_CELL_SECONDS", source) is None:
        raise CheckFailure(
            "five-second minimum can no longer be verified: pinned harness "
            f"is missing the cap predicate (harness source {sha})"
        )
    exit_predicate = (
        r"\bif\s+(?:minimums_met\s*\|\|\s*capped|"
        rf"\(?\s*(?:\bresult\.)?reps\s*>=\s*MIN_REPS\s*&&\s*"
        rf"elapsed\s*>=\s*MIN_TIMED_SECONDS\s*\)?\s*\|\|\s*"
        rf"elapsed\s*>=\s*MAX_CELL_SECONDS)\s*\{{"
    )
    if re.search(exit_predicate, source) is None:
        raise CheckFailure(
            "five-second minimum can no longer be verified: pinned harness "
            f"is missing the stopping exit predicate (harness source {sha})"
        )
    return True


def _pinned_harness_proof():
    sha = None
    with CSV_PATH.open(encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("# harness_source_sha:"):
                sha = line.partition(":")[2].strip()
                break
    if not sha:
        raise CheckFailure(
            "five-second minimum can no longer be verified: CSV has no "
            "# harness_source_sha: comment"
        )
    try:
        result = subprocess.run(
            ["git", "show", f"{sha}:{HARNESS_SOURCE_PATH}"],
            check=True,
            capture_output=True,
            encoding="utf-8",
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        detail = getattr(exc, "stderr", "") or str(exc)
        detail = detail.strip()
        raise CheckFailure(
            "five-second minimum can no longer be verified: could not extract "
            f"{HARNESS_SOURCE_PATH} from harness source {sha}"
            + (f": {detail}" if detail else "")
        ) from exc
    return _verify_harness_source(result.stdout, sha)


def validate_measured_rows(rows):
    """Verify both stopping minima and cap evidence for every measured row."""
    failures = []
    harness_proof = None
    for row in rows:
        if row["outcome"] != "measured":
            continue
        try:
            reps = int(row["reps"])
            timed_work = float(row["total_s"])
        except (KeyError, TypeError, ValueError) as exc:
            failures.append(
                f"q={row.get('q', '?')} n={row.get('n', '?')} {row.get('backend', '?')}: "
                f"invalid measured stopping fields ({exc})"
            )
            continue
        if not math.isfinite(timed_work) or timed_work < 0.0:
            failures.append(
                f"q={row['q']} n={row['n']} {row['backend']}: invalid total_s={row['total_s']}"
            )
            continue
        if timed_work < MIN_TIMED_SECONDS and harness_proof is None:
            try:
                harness_proof = _pinned_harness_proof()
            except CheckFailure as exc:
                failures.append(str(exc))
                break
        if reps < MIN_REPS and timed_work < MAX_CELL_SECONDS:
            failures.append(
                f"q={row['q']} n={row['n']} {row['backend']} M={row['batch_size']}: "
                "protocol contradiction: cap-path evidence failure; measured row has "
                f"reps={reps} but total_s={row['total_s']} < {MAX_CELL_SECONDS:g}; "
                "the cap path cannot be verified"
            )
    return failures


def rendered_rate_tokens(row):
    """Return normal publication precisions for a measured CSV rate.

    The study renders rates at a small number of significant figures. Keeping
    these tokens derived from the CSV lets the document-wide check follow the
    measured rows without a section-specific list of rates or locations.
    """
    rate = float(row["composite_matrices_per_s"])
    return {format(rate, f".{figures}g") for figures in range(3, 7)}


def line_number(text, offset):
    return text.count("\n", 0, offset) + 1


def has_dagger_after_rate(text, match):
    """Whether the rendered rate has the study's adjacent dagger marker."""
    line_end = text.find("\n", match.end())
    if line_end == -1:
        line_end = len(text)
    suffix = text[match.end() : line_end]
    return "†" in suffix[:8]


def document_rate_failures(text, rows):
    anomalous = [row for row in rows if row_is_nonconforming(row)]
    token_rows = {
        token: row
        for row in anomalous
        for token in rendered_rate_tokens(row)
    }
    if not token_rows:
        return []
    token_pattern = re.compile(
        r"(?<![0-9])(?:"
        + "|".join(re.escape(token) for token in sorted(token_rows, key=len, reverse=True))
        + r")(?![0-9])"
    )
    failures = []
    for match in token_pattern.finditer(text):
        row = token_rows[match.group(0)]
        if has_dagger_after_rate(text, match):
            continue
        batch = f" M={row['batch_size']}" if row["backend"] == "gpu_hip" else ""
        failures.append(
            f"line {line_number(text, match.start())}: rendered nonconforming rate"
            f" {match.group(0)} (q={row['q']} n={row['n']}"
            f" {row['backend']}{batch}): missing †"
        )
    return failures


def main():
    rows = csv_rows()
    by_key = {}
    for row in rows:
        key = (row["q"], row["n"], row["backend"], row["batch_size"])
        by_key[key] = row

    text = DOC.read_text(encoding="utf-8")
    section = section_44(text)
    row_re = re.compile(r"^\|\s*([357])\s*\|\s*(\d+)\s*\|(.+)\|\s*$", re.M)
    failures = validate_measured_rows(rows)
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

    failures.extend(document_rate_failures(text, rows))
    print(f"checked {checked} rendered measured section 4.4 cells against {CSV_PATH.name}")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("all rendered measured cells have correct stopping-rule markers")
    return 0


if __name__ == "__main__":
    sys.exit(main())
