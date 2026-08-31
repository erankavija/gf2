#!/usr/bin/env python3
"""Check that the external baseline encodes the same code as gf2 (jit:4e732b56).

The survey compares gf2 against AFF3CT on the two DVB-T2 rows. That comparison
is only like-for-like if both sides use the same generator polynomial, so this
check recomputes the DVB-T2 generator from the minimal-polynomial table in the
gf2 tree and compares it, coefficient by coefficient, against the polynomial the
AFF3CT harness emitted with `aff3ct_bch_bench gdump`.

Both inputs are read at run time: the exponent table is parsed out of the Rust
source, and the AFF3CT coefficients out of the run's committed `generators.txt`.
Neither is transcribed here.

Usage:
    ./verify-generators.py <run-dir>/generators.txt

Exit status is 0 when every checked row agrees and 1 otherwise.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parents[4]
GENERATORS_RS = REPO / "crates/gf2-coding/src/bch/dvb_t2/generators.rs"

# Contract row -> (constant in generators.rs, t)
ROWS = {"T2S": ("SHORT_GENERATORS", 12), "T2N": ("NORMAL_GENERATORS", 12)}


def parse_table(source: str, const: str) -> list[list[int]]:
    """Extract the exponent lists of one `pub const <const>: &[&[usize]]` table."""
    start = source.index(f"pub const {const}")
    body = source[start : source.index("];", start)]
    return [
        [int(x) for x in re.findall(r"\d+", row)]
        for row in re.findall(r"&\[([0-9,\s]+)\]", body)
    ]


def poly_from_exponents(exponents: list[int]) -> int:
    value = 0
    for e in exponents:
        value |= 1 << e
    return value


def gf2_mul(a: int, b: int) -> int:
    result = 0
    while b:
        low = b & -b
        result ^= a << (low.bit_length() - 1)
        b ^= low
    return result


def product(table: list[list[int]], t: int) -> int:
    """g(x) is the product of the first t minimal polynomials."""
    g = 1
    for exponents in table[:t]:
        g = gf2_mul(g, poly_from_exponents(exponents))
    return g


def parse_gdump(path: pathlib.Path) -> dict[str, int]:
    """`<name> <n> <k> <deg> <coeff_0><coeff_1>...`, coefficient i of x^i."""
    out: dict[str, int] = {}
    for line in path.read_text().splitlines():
        fields = line.split()
        if len(fields) != 5:
            continue
        value = 0
        for i, ch in enumerate(fields[4]):
            if ch == "1":
                value |= 1 << i
        out[fields[0]] = value
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("gdump", type=pathlib.Path)
    args = ap.parse_args()

    source = GENERATORS_RS.read_text()
    emitted = parse_gdump(args.gdump)

    failures = 0
    for row, (const, t) in ROWS.items():
        if row not in emitted:
            print(f"{row}: MISSING from {args.gdump}")
            failures += 1
            continue
        expected = product(parse_table(source, const), t)
        actual = emitted[row]
        agree = expected == actual
        print(
            f"{row}: gf2 tree deg={expected.bit_length() - 1} "
            f"aff3ct deg={actual.bit_length() - 1} identical={agree}"
        )
        if not agree:
            failures += 1

    if failures:
        print(f"{failures} row(s) disagree; the comparison is not like-for-like", file=sys.stderr)
        return 1
    print(f"all {len(ROWS)} DVB-T2 rows agree; the encoding comparison is like-for-like")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
