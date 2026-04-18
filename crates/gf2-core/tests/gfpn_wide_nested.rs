//! Integration tests for the `Wide` accumulator type on nested tower
//! extensions (issue d11b769a).
//!
//! Verifies that `QuadraticExt<QuadraticExt<Fp<P>>>` (i.e., GF(p⁴)) propagates
//! the wide type correctly — the outer `Wide` is
//! `QuadraticExtWide<QuadraticExtWide<u128>>` — and that all field axioms hold
//! including the wide roundtrip and `mul_to_wide` consistency.

use gf2_core::field::{ConstField, FiniteField};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{ExtConfig, QuadraticExt, QuadraticExtWide};

// GF(65537²) = Fp<65537>[u]/(u² − β). 65537 is prime and 3 is a quadratic
// non-residue mod 65537 (it's the smallest such value for that Fermat prime).
struct Fp65537Ext2;
impl ExtConfig for Fp65537Ext2 {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
type Fq2 = QuadraticExt<Fp65537Ext2>;

// GF(65537⁴) = Fq2[w]/(w² − γ). The inner β = 3 is NOT a square in Fq2
// (verified by: 3 is a QNR in Fp<65537>, so c₀ + 0·u = 3 has no square root).
// For simplicity we pick γ = u (i.e., γ = 0 + 1·u in Fq2), which is a known
// non-square in any quadratic extension field whose base non-residue is not a
// square (it is the "canonical" construction of GF(p^4) via two quadratic
// extensions).
struct Fp65537Ext4;
impl ExtConfig for Fp65537Ext4 {
    type BaseField = Fq2;
    const NON_RESIDUE: Fq2 = Fq2::new(Fp::<65537>::new(0), Fp::<65537>::new(1));
}
type Fq4 = QuadraticExt<Fp65537Ext4>;

/// The outer `Wide` propagates through the tower:
/// `QuadraticExt<QuadraticExt<Fp<P>>>::Wide == QuadraticExtWide<QuadraticExtWide<u128>>`.
/// This test compiles only if the types match (enforced by the type checker).
#[test]
fn nested_wide_type_propagation() {
    // Construct by type inference from a concrete element path.
    let a = Fq4::new(
        Fq2::new(Fp::new(1), Fp::new(2)),
        Fq2::new(Fp::new(3), Fp::new(4)),
    );
    let w: <Fq4 as FiniteField>::Wide = a.to_wide();
    // Statically coerce to the fully-expanded nested type — this fails to
    // compile if the propagation is wrong.
    let _: QuadraticExtWide<QuadraticExtWide<u128>> = w;
}

#[test]
fn nested_wide_roundtrip() {
    // Pick a few non-trivial elements and verify reduce_wide ∘ to_wide = id.
    for c00 in [0u64, 1, 42, 65535] {
        for c01 in [0u64, 7, 1000] {
            for c10 in [0u64, 3, 2023] {
                for c11 in [0u64, 11, 65000] {
                    let a = Fq4::new(
                        Fq2::new(Fp::new(c00), Fp::new(c01)),
                        Fq2::new(Fp::new(c10), Fp::new(c11)),
                    );
                    let wide = a.to_wide();
                    let back = <Fq4 as FiniteField>::reduce_wide(&wide);
                    assert_eq!(back, a, "roundtrip at ({c00},{c01},{c10},{c11})");
                }
            }
        }
    }
}

#[test]
fn nested_mul_to_wide_consistency() {
    // Pick several pairs and check mul_to_wide ∘ reduce_wide matches direct
    // multiplication.
    let samples = [
        (0u64, 1, 0, 0),
        (1, 0, 0, 0),
        (2, 3, 4, 5),
        (123, 456, 789, 101),
        (65535, 1, 2, 65536),
        (7, 7, 7, 7),
    ];
    for (a00, a01, a10, a11) in samples {
        for (b00, b01, b10, b11) in samples {
            let a = Fq4::new(
                Fq2::new(Fp::new(a00), Fp::new(a01)),
                Fq2::new(Fp::new(a10), Fp::new(a11)),
            );
            let b = Fq4::new(
                Fq2::new(Fp::new(b00), Fp::new(b01)),
                Fq2::new(Fp::new(b10), Fp::new(b11)),
            );
            let wide = a.mul_to_wide(&b);
            let reduced = <Fq4 as FiniteField>::reduce_wide(&wide);
            assert_eq!(reduced, a * b);
        }
    }
}

#[test]
fn nested_dot_product_accumulation() {
    // Accumulate 16 random-looking products in the nested wide, reduce once,
    // and compare to the element-wise multiply-and-add path.
    let coeffs: &[(u64, u64, u64, u64)] = &[
        (1, 2, 3, 4),
        (5, 6, 7, 8),
        (9, 10, 11, 12),
        (13, 14, 15, 16),
        (17, 18, 19, 20),
        (21, 22, 23, 24),
        (25, 26, 27, 28),
        (29, 30, 31, 32),
        (33, 34, 35, 36),
        (37, 38, 39, 40),
        (41, 42, 43, 44),
        (45, 46, 47, 48),
        (49, 50, 51, 52),
        (53, 54, 55, 56),
        (57, 58, 59, 60),
        (61, 62, 63, 64),
    ];
    let mut acc: <Fq4 as FiniteField>::Wide = <Fq4 as FiniteField>::Wide::default();
    let mut expected = Fq4::zero();
    for (i, (x0, x1, x2, x3)) in coeffs.iter().enumerate() {
        let (y0, y1, y2, y3) = coeffs[(i + 7) % coeffs.len()];
        let a = Fq4::new(
            Fq2::new(Fp::new(*x0), Fp::new(*x1)),
            Fq2::new(Fp::new(*x2), Fp::new(*x3)),
        );
        let b = Fq4::new(
            Fq2::new(Fp::new(y0), Fp::new(y1)),
            Fq2::new(Fp::new(y2), Fp::new(y3)),
        );
        acc += a.mul_to_wide(&b);
        expected += a * b;
    }
    let got = <Fq4 as FiniteField>::reduce_wide(&acc);
    assert_eq!(got, expected);
}

/// The nested tower must still cap accumulation at the base prime's budget.
#[test]
fn nested_max_unreduced_additions_matches_base() {
    // For P=65537 the bound `u128::MAX / (P-1)²` exceeds `usize::MAX` and
    // saturates, so this assertion is only meaningful for the *equality*:
    // the tower must pass through whatever the base field reports.
    let k = <Fq4 as FiniteField>::max_unreduced_additions();
    let base = <Fp<65537> as FiniteField>::max_unreduced_additions();
    assert_eq!(k, base);
}

// ---------------------------------------------------------------------------
// Larger-prime nested tower: GF(Mersenne61²) composed once more ⇒ GF(P⁴).
// Mersenne61 makes `max_unreduced_additions` finite, proving the tower no
// longer returns the `usize::MAX` sentinel at the top level.
// ---------------------------------------------------------------------------

const M61: u64 = 2305843009213693951;

struct M61Ext2;
impl ExtConfig for M61Ext2 {
    type BaseField = Fp<M61>;
    const NON_RESIDUE: Fp<M61> = Fp::<M61>::new(2);
}
type M61Fq2 = QuadraticExt<M61Ext2>;

struct M61Ext4;
impl ExtConfig for M61Ext4 {
    type BaseField = M61Fq2;
    const NON_RESIDUE: M61Fq2 = M61Fq2::new(Fp::<M61>::new(0), Fp::<M61>::new(1));
}
type M61Fq4 = QuadraticExt<M61Ext4>;

#[test]
fn nested_max_unreduced_additions_finite_for_large_prime() {
    // Mersenne61 gives `k = u128::MAX / (P-1)² ≈ 64` — far below usize::MAX.
    let k = <M61Fq4 as FiniteField>::max_unreduced_additions();
    let base = <Fp<M61> as FiniteField>::max_unreduced_additions();
    assert_eq!(k, base);
    assert_ne!(k, usize::MAX);
    assert!(k >= 1);
    // Sanity: the Fq2 intermediate level also equals the base bound.
    let intermediate = <M61Fq2 as FiniteField>::max_unreduced_additions();
    assert_eq!(intermediate, base);
}

#[test]
fn nested_wide_type_propagates_through_two_levels() {
    let a = M61Fq4::new(
        M61Fq2::new(Fp::new(1), Fp::new(2)),
        M61Fq2::new(Fp::new(3), Fp::new(4)),
    );
    let w: <M61Fq4 as FiniteField>::Wide = a.to_wide();
    // Type coercion: the wide must be exactly the doubly-nested shape.
    let _: QuadraticExtWide<QuadraticExtWide<u128>> = w;
}

/// Field axioms smoke test: one element and its inverse satisfy the identity.
#[test]
fn nested_axioms_smoke() {
    let a = Fq4::new(
        Fq2::new(Fp::new(3), Fp::new(5)),
        Fq2::new(Fp::new(7), Fp::new(11)),
    );
    assert!(!a.is_zero());
    let inv = a.inv().expect("non-zero has inverse in GF(65537^4)");
    assert!((a * inv).is_one());
    assert_eq!(a + Fq4::zero(), a);
    assert_eq!(a * Fq4::one(), a);
    assert_eq!(Fq4::order(), (65537u128).pow(4));
    assert_eq!(a.extension_degree(), 4);
}
