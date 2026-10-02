#!/usr/bin/env bash
set -euo pipefail

jit issue create "Factor polynomials over finite fields" \
  --type task --orphan \
  --label component:gf2-core \
  --description "$(cat <<'DESC'
Provide complete factorization of univariate polynomials over GF(2^m) and GF(p) in gf2-core, built on the existing polynomial, GCD and irreducibility machinery.

## Background

gf2-core detects reducibility (distinct-degree factor detection with a factor witness in the irreducibility test) and computes minimal polynomials, but exposes no routine returning the full factorization of a polynomial. Computer algebra systems (NTL, FLINT, Magma, Sage) provide square-free, distinct-degree and equal-degree factorization; the library's mission of competing with them includes this capability.

## Success Criteria

- [hard] REQ-01: A public function returns the complete factorization of a nonzero polynomial over GF(2^m) as irreducible factors with multiplicities and a unit, for field degrees covered by the existing field types.
- [hard] REQ-02: The same function supports polynomials over a prime field GF(p).
- [hard] REQ-03: Multiplying the returned factors with their multiplicities reproduces the input polynomial, and every returned factor passes the library's irreducibility test, for random inputs including repeated factors, constants and degree 0 and 1 polynomials.
- [hard] REQ-04: Seeded randomized steps return identical factorizations for identical inputs and seeds regardless of worker count.
- [hard] REQ-05: Rustdoc states panics and complexity; one teaching example shows factoring a BCH generator polynomial into minimal polynomials.
- [hard] REQ-06: A benchmark protocol and a committed receipt compare factorization time against NTL or FLINT on at least three field degrees.
DESC
)"
