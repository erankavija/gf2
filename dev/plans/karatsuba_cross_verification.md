# Karatsuba vs Naive Cross-Verification with Property-Based Tests

**Issue**: 2ce2a757 — Karatsuba vs naive cross-verification with property-based tests
**Parent**: 3f4b946c — Tower extension field architecture for GF(p^n)

## Problem

QuadraticExt and CubicExt use optimized multiplication algorithms (Karatsuba 3M,
Karatsuba-style 6M). While the axiom test harness validates field properties, it
does not directly compare optimized multiplication against a known-correct naive
implementation. Existing tests cover GF(7²) exhaustively (2401 pairs) but:

- Only cover one small field and one non-residue choice
- Don't cover CubicExt at all (until 3f3d6c23 lands)
- Don't use proptest for randomized coverage over larger fields
- Don't verify operation counts / performance bounds

## What already exists

In `quadratic.rs` tests:
- `test_karatsuba_matches_naive_exhaustive` — all 49×49 pairs in GF(7²), β = −1
- `test_quadratic_ext_fp7_field_axioms` — proptest axiom harness (49 elements)

These are good but narrow. This issue extends coverage across multiple field sizes,
non-residue choices, and both extension types.

## Approach

A single integration test file `crates/gf2-core/tests/karatsuba_cross_verify.rs`
containing proptest-based cross-verification. Separate from unit tests to allow
longer runtimes and cross-type coverage.

### Naive reference implementations

Define standalone naive multiplication functions that use schoolbook polynomial
arithmetic, independent of the Karatsuba/Karatsuba-style implementations:

```rust
/// Naive quadratic multiplication: (a0+a1·u)(b0+b1·u) mod (u²−β)
/// = (a0·b0 + β·a1·b1) + (a0·b1 + a1·b0)·u
/// Uses 4 base-field multiplications (no Karatsuba trick).
fn naive_quadratic_mul<C: ExtConfig>(
    a: QuadraticExt<C>,
    b: QuadraticExt<C>,
) -> QuadraticExt<C> {
    let c0 = a.c0() * b.c0() + C::mul_by_non_residue(a.c1() * b.c1());
    let c1 = a.c0() * b.c1() + a.c1() * b.c0();
    QuadraticExt::new(c0, c1)
}

/// Naive cubic multiplication via schoolbook (9M):
/// (a0+a1·v+a2·v²)(b0+b1·v+b2·v²) mod (v³−β)
fn naive_cubic_mul<C: ExtConfig>(
    a: CubicExt<C>,
    b: CubicExt<C>,
) -> CubicExt<C> {
    let (a0, a1, a2) = (a.c0(), a.c1(), a.c2());
    let (b0, b1, b2) = (b.c0(), b.c1(), b.c2());

    // Full schoolbook product (degree 4 polynomial, 5 coefficients):
    // d0 = a0·b0
    // d1 = a0·b1 + a1·b0
    // d2 = a0·b2 + a1·b1 + a2·b0
    // d3 = a1·b2 + a2·b1
    // d4 = a2·b2
    let d0 = a0 * b0;
    let d1 = a0 * b1 + a1 * b0;
    let d2 = a0 * b2 + a1 * b1 + a2 * b0;
    let d3 = a1 * b2 + a2 * b1;
    let d4 = a2 * b2;

    // Reduce mod v³ = β: v³ → β, v⁴ → β·v
    let c0 = d0 + C::mul_by_non_residue(d3);
    let c1 = d1 + C::mul_by_non_residue(d4);
    let c2 = d2;

    CubicExt::new(c0, c1, c2)
}
```

### Test configurations

Multiple field sizes and non-residue choices to catch corner cases:

| Type | Base field | Non-residue β | Order | Notes |
|------|-----------|---------------|-------|-------|
| QuadraticExt | Fp<7> | 6 (= −1) | 49 | Fast path: mul_by_non_residue = negation |
| QuadraticExt | Fp<7> | 3 | 49 | Generic non-residue, default mul_by_non_residue |
| QuadraticExt | Fp<101> | 99 (= −2) | 10201 | Larger prime, wider value range |
| QuadraticExt | Fp<65537> | 3 | ~4.3×10⁹ | Fermat prime, tests wider arithmetic |
| CubicExt | Fp<7> | 3 | 343 | Small field, exhaustive feasible |
| CubicExt | Fp<31> | 11 | 29791 | Medium field |
| CubicExt | Fp<101> | 2 | ~10⁶ | Larger prime |

Non-residue selection rationale:
- For quadratic: β must be a quadratic non-residue mod p (no square root in Fp).
- For cubic: β must be a cubic non-residue mod p (no cube root in Fp).
- Include at least one case with overridden `mul_by_non_residue` and one with default.

### Proptest configuration

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]

    #[test]
    fn test_karatsuba_matches_naive_fp101_quad(
        a0 in 0..101u64, a1 in 0..101u64,
        b0 in 0..101u64, b1 in 0..101u64,
    ) {
        let a = Fq2_101::new(Fp::new(a0), Fp::new(a1));
        let b = Fq2_101::new(Fp::new(b0), Fp::new(b1));
        let karatsuba = a * b;
        let naive = naive_quadratic_mul::<Fq2_101Config>(a, b);
        prop_assert_eq!(karatsuba, naive);
    }
}
```

10000 cases per test × ~7 configurations = 70000 total comparisons.

### Squaring cross-check

The default `FiniteFieldExt::square()` uses `self * self`. Verify this matches
a dedicated naive squaring formula (when one is added later). For now, just verify
`a.square() == naive_mul(a, a)` which confirms consistency.

### Performance bound verification

The issue description mentions verifying "QuadraticExt<Fp<P>> multiplication in
≤10 base-field muls". This is a static property of the algorithm, not a runtime
measurement. Document it as a code-level assertion rather than a benchmark:

- Karatsuba: 3M + 5A + 1B → 3 base muls + 1 `mul_by_non_residue` call.
  If `mul_by_non_residue` uses default (1 mul), total = 4M. ≤10 ✓
- Karatsuba-style cubic: 6M + 13A + 2B → 6 base muls + 2 `mul_by_non_residue`.
  If default, total = 8M. ≤10 for quad (N/A for cubic).

Since this is algorithmic rather than measurable, add a doc-comment on each `mul`
impl stating the operation count, and add a `#[test]` that simply asserts the
comment is accurate by verifying the result matches naive (which this issue does).

No benchmark is needed for this issue — benchmarks belong in `benches/`.

## File layout

```
crates/gf2-core/tests/
└── karatsuba_cross_verify.rs   — integration test (new)
```

All naive reference functions and test configs live in this single file.
No changes to library code.

## Test plan (for this issue's own gates)

1. `cargo test -p gf2-core --test karatsuba_cross_verify` passes.
2. `cargo clippy` clean.
3. `cargo fmt` clean.
4. All proptest cases (10000 per config) pass for both QuadraticExt and CubicExt.

## Dependencies

- 8d2863e6 (QuadraticExt) — in progress, needed for quadratic tests
- 3f3d6c23 (CubicExt) — ready, needed for cubic tests

Both must be done before this issue can be completed. However, the quadratic
portion can be written and tested as soon as 8d2863e6 lands. The cubic portion
is additive.

## Success criteria

- Proptest cross-verification with 10000+ random pairs per configuration.
- Multiple field sizes (small and large primes) and non-residue choices.
- Both QuadraticExt and CubicExt covered.
- Naive implementations are visibly independent of Karatsuba code (schoolbook 4M / 9M).
- Operation count documented on each `mul` impl.
