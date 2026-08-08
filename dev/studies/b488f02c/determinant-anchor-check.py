#!/usr/bin/env python3
"""Validate the exact finite-n singular probability used as the campaign's
determinant acceptance anchor (feasibility study section 6).

The claim under test is the closed form

    Pr[det(A) = 0] = 1 - prod_{i=1..n} (1 - q^-i)

for a uniformly random A in F_q^{n x n}, derived from the count of invertible
matrices prod_{k=0..n-1} (q^n - q^k) over the q^(n^2) matrices total.

This script checks it by exhaustive enumeration: for each small (q, n) it walks
every one of the q^(n^2) matrices, decides singularity by Gaussian elimination
mod q, and compares the resulting exact rational against the closed form. There
is no sampling and no RNG, so there is no seed and the output is deterministic:
the same (q, n) set always yields the same table.

It also tabulates how far the n -> infinity limit alpha_q sits from the exact
value at small n, which is the quantity that makes a limit-based acceptance test
unusable there.

Usage:
    python3 determinant-anchor-check.py > determinant-anchor-2026-08-08.txt
"""

from __future__ import annotations

import hashlib
import itertools
import pathlib
import platform
import sys
from fractions import Fraction

# Exhaustively enumerated cases. Each costs q^(n^2) matrices, so the set is
# bounded by what stays under a couple of minutes single-threaded.
ENUMERATED = [(3, 1), (3, 2), (3, 3), (5, 2), (5, 3), (7, 2)]

# Formula-only sizes, quoted by the study to show the limit's error at small n.
LIMIT_GAP_SIZES = {3: [2, 3, 4, 6, 8, 12, 16], 5: [2, 3, 4, 6, 8], 7: [2, 3, 4, 6, 8]}


def exact_singular(q: int, n: int) -> Fraction:
    """1 - prod_{i=1..n} (1 - q^-i), as an exact rational."""
    prod = Fraction(1)
    for i in range(1, n + 1):
        prod *= 1 - Fraction(1, q**i)
    return 1 - prod


def alpha_q(q: int, terms: int = 400) -> float:
    """The n -> infinity limit, 1 - prod_{i>=1} (1 - q^-i)."""
    prod = 1.0
    for i in range(1, terms):
        prod *= 1 - q ** (-i)
    return 1 - prod


def is_singular(flat: tuple[int, ...], q: int, n: int) -> bool:
    """Gaussian elimination mod q; q is prime for every case here."""
    a = [list(flat[r * n : (r + 1) * n]) for r in range(n)]
    for c in range(n):
        piv = next((r for r in range(c, n) if a[r][c] % q), None)
        if piv is None:
            return True
        if piv != c:
            a[c], a[piv] = a[piv], a[c]
        inv = pow(a[c][c], q - 2, q)
        for r in range(c + 1, n):
            f = (a[r][c] * inv) % q
            if f:
                for k in range(c, n):
                    a[r][k] = (a[r][k] - f * a[c][k]) % q
    return False


def brute_force(q: int, n: int) -> Fraction:
    total = singular = 0
    for flat in itertools.product(range(q), repeat=n * n):
        total += 1
        if is_singular(flat, q, n):
            singular += 1
    return Fraction(singular, total)


def main() -> int:
    script = pathlib.Path(__file__).resolve()
    digest = hashlib.sha256(script.read_bytes()).hexdigest()

    print("# Determinant anchor validation for the b488f02c feasibility study.")
    print("# Checks Pr[det = 0] = 1 - prod_{i=1..n}(1 - q^-i) by exhaustive")
    print("# enumeration of all q^(n^2) matrices over F_q.")
    print(f"# script: {script.name}")
    print(f"# script_sha256: {digest}")
    print(f"# invocation: python3 {script.name}")
    print("# inputs: none (exhaustive enumeration; no sampling, no RNG, no seed)")
    print(f"# python: {platform.python_version()} ({platform.python_implementation()})")
    print(f"# platform: {platform.platform()}")
    print()

    print("## Exhaustive enumeration against the closed form")
    print()
    print("q  n  matrices    singular    brute_force      closed_form      agree")
    ok = True
    for q, n in ENUMERATED:
        brute = brute_force(q, n)
        closed = exact_singular(q, n)
        agree = brute == closed
        ok &= agree
        total = q ** (n * n)
        print(
            f"{q}  {n}  {total:<10d}  {brute.numerator * total // brute.denominator:<10d}  "
            f"{str(brute):<15s}  {str(closed):<15s}  {'yes' if agree else 'NO'}"
        )
    print()
    print(f"all_cases_agree: {'yes' if ok else 'NO'}")
    print()

    print("## Why the n -> infinity limit cannot serve as the anchor")
    print()
    print("# alpha_q is the limit; the gap is what a limit-based test would treat")
    print("# as a deviation even from a perfect pipeline.")
    print()
    print("q  alpha_q         n   exact            limit_minus_exact")
    for q, sizes in LIMIT_GAP_SIZES.items():
        a = alpha_q(q)
        for n in sizes:
            e = float(exact_singular(q, n))
            print(f"{q}  {a:.10f}    {n:<3d} {e:.10f}     {a - e:.3e}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
