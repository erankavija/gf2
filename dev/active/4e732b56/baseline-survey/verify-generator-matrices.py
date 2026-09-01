#!/usr/bin/env python3
"""Compare systematic generator matrices from M4RI and gf2 (jit:4e732b56).

This is a correctness-only check. The M4RI helper runs the timed
``genmatrix-rref`` route — fill in the repository column order, reduce with
``mzd_echelonize_m4ri`` — and dumps its output verbatim; the gf2 helper calls
``GeneratorMatrixAccess::generator_matrix``. Both dumps are row-major bits in
the repository's ``[message | parity]`` order, so the comparison applies no
column normalization. ``normalize`` only validates dimensions and copies rows,
and the receipt records a digest of each side's output.

Usage:
    ./verify-generator-matrices.py [--generators PATH] [--m4ri-bin PATH]
        [--gf2-bin PATH] > generator-matrix-agreement.txt
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import os
import pathlib
import shlex
import subprocess
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parents[4]
ROWS = ("B2", "B3", "T2S")


def parse_dump(path: pathlib.Path) -> dict[str, tuple[int, int, list[str]]]:
    """Read the line-oriented helper output into per-code row strings."""
    result: dict[str, tuple[int, int, list[str]]] = {}
    lines = iter(path.read_text().splitlines())
    for line in lines:
        if not line or line.startswith("#"):
            continue
        fields = line.split()
        if len(fields) != 4 or fields[0] != "code":
            raise ValueError(f"unexpected dump line in {path}: {line!r}")
        name = fields[1]
        n = int(fields[2].split("=", 1)[1])
        k = int(fields[3].split("=", 1)[1])
        matrix = []
        for _ in range(k):
            row = next(lines)
            if len(row) != n or set(row) - {"0", "1"}:
                raise ValueError(f"invalid {name} row in {path}: {row[:80]!r}")
            matrix.append(row)
        end = next(lines, None)
        if end != f"end {name}":
            raise ValueError(f"missing end marker for {name} in {path}")
        result[name] = (n, k, matrix)
    return result


def normalize(n: int, k: int, rows: list[str], *, reverse_columns: bool) -> tuple[str, ...]:
    """Copy row-major bits into the comparison layout.

    M4RI's timed route uses polynomial-coordinate columns, whose coordinate
    order is the reverse of gf2's codeword order. The verifier reverses those
    columns after RREF; gf2's dump is already in repository order.
    """
    if len(rows) != k or any(len(row) != n for row in rows):
        raise ValueError("matrix dimensions do not match its header")
    normalized = tuple("".join(row[j] for j in range(n)) for row in rows)
    return tuple(row[::-1] for row in normalized) if reverse_columns else normalized


def digest(rows: tuple[str, ...]) -> str:
    return hashlib.sha256(("\n".join(rows) + "\n").encode()).hexdigest()


def rref(n: int, rows: tuple[str, ...]) -> tuple[int, tuple[int, ...]]:
    """Canonical GF(2) reduced row echelon form under the given column order.

    Rows are bitstrings with column 0 leftmost. Returns ``(rank, rref_rows)``
    with each row as an integer whose most significant bit is column 0. Two
    full-rank generator matrices span the same code exactly when their
    canonical RREFs under one shared column order are identical.
    """
    pivots: dict[int, int] = {}
    for row in rows:
        vec = int(row, 2)
        for pivot_col, pivot_vec in pivots.items():
            if (vec >> (n - 1 - pivot_col)) & 1:
                vec ^= pivot_vec
        if not vec:
            continue
        col = n - vec.bit_length()
        for pivot_col, pivot_vec in pivots.items():
            if (pivot_vec >> (n - 1 - col)) & 1:
                pivots[pivot_col] = pivot_vec ^ vec
        pivots[col] = vec
    return len(pivots), tuple(pivots[col] for col in sorted(pivots))


def systematic(n: int, k: int, rows: tuple[str, ...]) -> bool:
    return all(row[col] == ("1" if row_idx == col else "0")
               for row_idx, row in enumerate(rows)
               for col in range(k))


def run_dump(command: list[str], env: dict[str, str], path: pathlib.Path) -> None:
    completed = subprocess.run(command, env=env, stdout=path.open("w"), stderr=subprocess.PIPE, text=True)
    if completed.returncode:
        print(completed.stderr, file=sys.stderr, end="")
        raise SystemExit(f"generator dump failed ({completed.returncode}): {shlex.join(command)}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--generators",
        type=pathlib.Path,
        default=REPO / "dev/bench_results/4e732b56/generators.txt",
    )
    ap.add_argument(
        "--m4ri-bin",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parent / "m4ri_genmatrix_bench",
    )
    ap.add_argument(
        "--gf2-bin",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parent / "gf2-side/target/release/survey-gf2-side",
    )
    args = ap.parse_args()

    codes = ",".join(ROWS)
    gf2_rev = subprocess.check_output(["git", "-C", str(REPO), "rev-parse", "HEAD"], text=True).strip()
    env = os.environ.copy()
    env["GF2_SURVEY_CODES"] = codes
    m4ri_command = [str(args.m4ri_bin), str(args.generators), "generator-dump"]
    gf2_command = [str(args.gf2_bin), "generator-dump"]

    with tempfile.TemporaryDirectory(prefix="gf2-generator-agreement-") as temp:
        temp_dir = pathlib.Path(temp)
        m4ri_dump = temp_dir / "m4ri.txt"
        gf2_dump = temp_dir / "gf2.txt"
        run_dump(m4ri_command, env, m4ri_dump)
        gf2_env = dict(env)
        gf2_env["GF2_REV"] = gf2_rev
        run_dump(gf2_command, gf2_env, gf2_dump)
        m4ri = parse_dump(m4ri_dump)
        gf2 = parse_dump(gf2_dump)

    print("# correctness-only systematic generator-matrix agreement")
    print(f"# captured: {datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')}")
    print(f"# rows: {codes}")
    print(f"# M4RI command: {shlex.join(['GF2_SURVEY_CODES=' + codes, *m4ri_command])}")
    print(f"# gf2 command: {shlex.join(['GF2_SURVEY_CODES=' + codes, 'GF2_REV=' + gf2_rev, *gf2_command])}")
    print("# normalization: none — the M4RI route fills and reduces in the repository column order, so both dumps are already row-major [message | parity]")
    print("# same-code criterion: full rank and identical canonical GF(2) RREF of both normalized matrices under the repository column order (row-space equality)")

    failures = 0
    for name in ROWS:
        if name not in m4ri or name not in gf2:
            print(f"{name}: missing from one or both dumps")
            failures += 1
            continue
        m4ri_n, m4ri_k, m4ri_rows = m4ri[name]
        gf2_n, gf2_k, gf2_rows = gf2[name]
        m4ri_raw = normalize(m4ri_n, m4ri_k, m4ri_rows, reverse_columns=False)
        m4ri_normal = m4ri_raw
        gf2_normal = normalize(gf2_n, gf2_k, gf2_rows, reverse_columns=False)
        dimensions_match = (m4ri_n, m4ri_k) == (gf2_n, gf2_k)
        identical = dimensions_match and m4ri_normal == gf2_normal
        m4ri_rank, m4ri_rref = rref(m4ri_n, m4ri_normal)
        gf2_rank, gf2_rref = rref(gf2_n, gf2_normal)
        same_code = (
            dimensions_match
            and m4ri_rank == m4ri_k
            and gf2_rank == gf2_k
            and m4ri_rref == gf2_rref
        )
        print(
            f"{name}: dimensions={m4ri_k}x{m4ri_n} dimensions_match={dimensions_match} "
            f"m4ri_raw_systematic={systematic(m4ri_n, m4ri_k, m4ri_raw)} "
            f"m4ri_normalized_systematic={systematic(m4ri_n, m4ri_k, m4ri_normal)} "
            f"gf2_systematic={systematic(gf2_n, gf2_k, gf2_normal)} "
            f"m4ri_sha256={digest(m4ri_normal)} gf2_sha256={digest(gf2_normal)} "
            f"bit_exact_identical={identical} "
            f"m4ri_rank={m4ri_rank} gf2_rank={gf2_rank} same_code={same_code}"
        )
        if not same_code:
            failures += 1

    if failures:
        print(f"{failures} generator-matrix comparison(s) failed row-space equality", file=sys.stderr)
        return 1
    print(
        f"all {len(ROWS)} rows: full rank and identical canonical RREF under the "
        "repository column order — both routes generate the same code; "
        "bit_exact_identical above reports whether the systematic forms also coincide"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
