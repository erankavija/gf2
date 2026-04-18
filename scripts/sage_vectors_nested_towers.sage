#!/usr/bin/env sage
# scripts/sage_vectors_nested_towers.sage
#
# Regenerates the `SAGE_FQ4`, `SAGE_FQ6`, and `SAGE_FQ12` cross-verification
# vectors used by `crates/gf2-core/tests/gfpn_nested_towers.rs`.
#
# The Rust integration test hard-codes the tuples (a, b, a·b, a+b, a⁻¹) for
# three nested tower chains; this script is the single source of truth that
# produced them. If a tower construction ever changes (e.g., a different
# non-residue), rerun this script and paste the output into the test file.
#
# Tower chains (must match `tests/common/mod.rs` exactly):
#
#   GF(65537⁴) = Fp<65537>[u]/(u²−3)[w]/(w²−u)
#   GF(7⁶)    = Fp<7>[u]/(u²−3)[v]/(v³−u)
#   GF(7¹²)   = Fp<7>[u]/(u²−3)[v]/(v³−u)[z]/(z²−(v+1))
#
# Coefficient tuple order: "innermost Fp component varies fastest", matching
# the `fq{4,6,12}_from_flat` helpers in `tests/common/mod.rs`. Concretely:
#
#   Fq4  flat[0..4]  = (u_coeff_of_w0, 1_coeff_of_w0, u_coeff_of_w1, 1_coeff_of_w1)
#                      i.e., (c00, c01, c10, c11) in the Rust helper.
#   Fq6  flat[0..6]  = (v⁰_u, v⁰_1, v¹_u, v¹_1, v²_u, v²_1)
#   Fq12 flat[0..12] = (z⁰-side Fq6 flat, z¹-side Fq6 flat)
#
# Seed: set_random_seed(42) — deterministic across Sage releases for the
# random-element generator used here.
#
# Usage:
#   sage scripts/sage_vectors_nested_towers.sage
#
# Sage version used when generating the committed vectors: 10.8.

set_random_seed(42)

# ---------------------------------------------------------------------------
# Fq4 = GF(65537^4) via u²=3, w²=u
# ---------------------------------------------------------------------------
p = 65537
Fp = GF(p)
R.<x> = PolynomialRing(Fp)
Fq2.<u> = Fp.extension(x^2 - 3)
assert (x^2 - 3).is_irreducible(), "u^2 - 3 must be irreducible over Fp"
R2.<y> = PolynomialRing(Fq2)
Fq4.<w> = Fq2.extension(y^2 - u)
assert (y^2 - u).is_irreducible(), "y^2 - u must be irreducible over Fq2"

def fq4_flat(e):
    # e = (c0_u * u + c0_1) + (c1_u * u + c1_1) * w
    c0, c1 = e.list()
    c00, c01 = c0.list()
    c10, c11 = c1.list()
    return [int(c00), int(c01), int(c10), int(c11)]

print("// GF(65537^4) = Fp[u]/(u^2-3)[w]/(w^2-u)")
for _ in range(10):
    while True:
        a = Fq4.random_element()
        if a != 0:
            break
    b = Fq4.random_element()
    prod = a * b
    s = a + b
    inv = a^-1
    print("Fq4Vec {")
    print(f"    a: {fq4_flat(a)},")
    print(f"    b: {fq4_flat(b)},")
    print(f"    prod: {fq4_flat(prod)},")
    print(f"    sum: {fq4_flat(s)},")
    print(f"    inv_a: {fq4_flat(inv)},")
    print("},")

# ---------------------------------------------------------------------------
# Fq6 = GF(7^6) via u²=3, v³=u
# ---------------------------------------------------------------------------
p = 7
Fp = GF(p)
R.<x> = PolynomialRing(Fp)
Fq2_small.<u> = Fp.extension(x^2 - 3)
assert (x^2 - 3).is_irreducible(), "u^2 - 3 must be irreducible over Fp"
R2.<y> = PolynomialRing(Fq2_small)
Fq6.<v> = Fq2_small.extension(y^3 - u)
assert (y^3 - u).is_irreducible(), "y^3 - u must be irreducible over Fq2 (u a cubic non-residue)"

def fq6_flat(e):
    # e = c0 + c1*v + c2*v^2,  each ci = ci_u*u + ci_1
    cs = e.list()
    out = []
    for ci in cs:
        cu, c1 = ci.list()
        out.append(int(cu))
        out.append(int(c1))
    while len(out) < 6:
        out.append(0)
    return out

print("\n// GF(7^6) = Fp[u]/(u^2-3)[v]/(v^3-u)")
for _ in range(10):
    while True:
        a = Fq6.random_element()
        if a != 0:
            break
    b = Fq6.random_element()
    prod = a * b
    s = a + b
    inv = a^-1
    print("Fq6Vec {")
    print(f"    a: {fq6_flat(a)},")
    print(f"    b: {fq6_flat(b)},")
    print(f"    prod: {fq6_flat(prod)},")
    print(f"    sum: {fq6_flat(s)},")
    print(f"    inv_a: {fq6_flat(inv)},")
    print("},")

# ---------------------------------------------------------------------------
# Fq12 = GF(7^12) via u²=3, v³=u, z²=v+1
# ---------------------------------------------------------------------------
R3.<t> = PolynomialRing(Fq6)
Fq12.<z> = Fq6.extension(t^2 - (v + 1))
assert (t^2 - (v + 1)).is_irreducible(), "t^2 - (v+1) must be irreducible over Fq6"

def fq12_flat(e):
    # e = lo + hi*z, each of lo,hi in Fq6
    cs = e.list()
    lo = cs[0] if len(cs) >= 1 else Fq6(0)
    hi = cs[1] if len(cs) >= 2 else Fq6(0)
    return fq6_flat(lo) + fq6_flat(hi)

print("\n// GF(7^12) = Fp[u]/(u^2-3)[v]/(v^3-u)[z]/(z^2-(v+1))")
for _ in range(10):
    while True:
        a = Fq12.random_element()
        if a != 0:
            break
    b = Fq12.random_element()
    prod = a * b
    s = a + b
    inv = a^-1
    print("Fq12Vec {")
    print(f"    a: {fq12_flat(a)},")
    print(f"    b: {fq12_flat(b)},")
    print(f"    prod: {fq12_flat(prod)},")
    print(f"    sum: {fq12_flat(s)},")
    print(f"    inv_a: {fq12_flat(inv)},")
    print("},")
