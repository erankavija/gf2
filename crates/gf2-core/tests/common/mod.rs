//! Shared tower configurations, schoolbook multipliers and flat-coefficient
//! codecs for the `gfpn_*` and `karatsuba_cross_verify` integration tests. The
//! multipliers use only public accessors and base-field operators, so they
//! share no subroutine with the Karatsuba path under test.

#![allow(dead_code)]
#![deny(unsafe_code)]

use gf2_core::gfp::Fp;
use gf2_core::gfpn::{CubicExt, ExtConfig, QuadraticExt};

/// GF(65537²) = `Fp<65537>[u]/(u² − 3)`.
///
/// 3 is a quadratic non-residue mod 65537.
pub struct Fp65537Ext2;
impl ExtConfig for Fp65537Ext2 {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
pub type Fq2Large = QuadraticExt<Fp65537Ext2>;

/// GF(65537⁴) = `Fq2Large[w]/(w² − u)`.
///
/// `u` is a non-square in `Fq2Large`.
pub struct Fp65537Ext4;
impl ExtConfig for Fp65537Ext4 {
    type BaseField = Fq2Large;
    const NON_RESIDUE: Fq2Large = Fq2Large::new(Fp::<65537>::new(0), Fp::<65537>::new(1));
}
pub type Fq4Large = QuadraticExt<Fp65537Ext4>;

/// GF(7²) = `Fp<7>[u]/(u² − 3)`.
pub struct Fp7Ext2;
impl ExtConfig for Fp7Ext2 {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
}
pub type Fq2Small = QuadraticExt<Fp7Ext2>;

/// GF(7⁶) = `Fq2Small[v]/(v³ − u)`.
///
/// `u` is a cubic non-residue in GF(7²), checked with
/// `@/citation/SageMath2026`.
pub struct Fp7Ext6;
impl ExtConfig for Fp7Ext6 {
    type BaseField = Fq2Small;
    const NON_RESIDUE: Fq2Small = Fq2Small::new(Fp::<7>::new(0), Fp::<7>::new(1));
}
pub type Fq6Small = CubicExt<Fp7Ext6>;

/// GF(7¹²) = `Fq6Small[z]/(z² − (v + 1))`.
///
/// `v + 1` is a quadratic non-residue in GF(7⁶), checked with
/// `@/citation/SageMath2026`.
pub struct Fp7Ext12;
impl ExtConfig for Fp7Ext12 {
    type BaseField = Fq6Small;
    /// `v + 1` in coefficient-tuple form: `c₀ = 1 + 0·u`, `c₁ = 1 + 0·u`,
    /// `c₂ = 0 + 0·u` meaning `(1 + 0·u) + (1 + 0·u)·v + (0 + 0·u)·v²`.
    const NON_RESIDUE: Fq6Small = Fq6Small::new(
        Fq2Small::new(Fp::<7>::new(1), Fp::<7>::new(0)),
        Fq2Small::new(Fp::<7>::new(1), Fp::<7>::new(0)),
        Fq2Small::new(Fp::<7>::new(0), Fp::<7>::new(0)),
    );
}
pub type Fq12Small = QuadraticExt<Fp7Ext12>;

/// GF(7²) with β = 6 ≡ −1 (mod 7). Overridden `mul_by_non_residue` (negation).
pub struct Fq2Fp7NegOneConfig;
impl ExtConfig for Fq2Fp7NegOneConfig {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(6);

    #[inline]
    fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
        -x
    }
}
pub type Fq2Fp7NegOne = QuadraticExt<Fq2Fp7NegOneConfig>;

/// GF(101²) with β = 99 ≡ −2 (mod 101). Default `mul_by_non_residue`.
pub struct Fq2Fp101NegTwoConfig;
impl ExtConfig for Fq2Fp101NegTwoConfig {
    type BaseField = Fp<101>;
    const NON_RESIDUE: Fp<101> = Fp::<101>::new(99);
}
pub type Fq2Fp101NegTwo = QuadraticExt<Fq2Fp101NegTwoConfig>;

/// GF(7³) with β = 3 and overridden `mul_by_non_residue` (`3x = x+x+x`).
pub struct Fq3Fp7Beta3Config;
impl ExtConfig for Fq3Fp7Beta3Config {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);

    #[inline]
    fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
        x + x + x
    }
}
pub type Fq3Fp7Beta3 = CubicExt<Fq3Fp7Beta3Config>;

/// GF(31³) with β = 11. Default `mul_by_non_residue`.
pub struct Fq3Fp31Beta11Config;
impl ExtConfig for Fq3Fp31Beta11Config {
    type BaseField = Fp<31>;
    const NON_RESIDUE: Fp<31> = Fp::<31>::new(11);
}
pub type Fq3Fp31Beta11 = CubicExt<Fq3Fp31Beta11Config>;

/// `Fp<101>[x]/(x³ − 2)`. Default `mul_by_non_residue`.
///
/// gcd(3, 100) = 1, so every element of `Fp<101>` is a cube and x³ − 2 is
/// reducible: the ring is not a field. Karatsuba and schoolbook still compute
/// the same product modulo x³ − β.
pub struct Fq3Fp101Beta2Config;
impl ExtConfig for Fq3Fp101Beta2Config {
    type BaseField = Fp<101>;
    const NON_RESIDUE: Fp<101> = Fp::<101>::new(2);
}
pub type Fq3Fp101Beta2 = CubicExt<Fq3Fp101Beta2Config>;

/// Schoolbook [`QuadraticExt`] multiplication using 4 base-field mults.
///
/// Given `a = a0 + a1·u` and `b = b0 + b1·u` with `u² = β`:
/// ```text
/// a · b = (a0·b0 + β·a1·b1) + (a0·b1 + a1·b0)·u
/// ```
#[inline(never)]
pub fn naive_quadratic_mul<C: ExtConfig>(
    a: QuadraticExt<C>,
    b: QuadraticExt<C>,
) -> QuadraticExt<C> {
    let a0 = a.c0();
    let a1 = a.c1();
    let b0 = b.c0();
    let b1 = b.c1();

    let m00 = a0 * b0;
    let m01 = a0 * b1;
    let m10 = a1 * b0;
    let m11 = a1 * b1;

    let c0 = m00 + C::mul_by_non_residue(m11);
    let c1 = m01 + m10;

    QuadraticExt::new(c0, c1)
}

/// Schoolbook [`CubicExt`] multiplication using 9 base-field mults.
///
/// Given `a = a0 + a1·v + a2·v²` and `b = b0 + b1·v + b2·v²` with `v³ = β`:
/// ```text
/// d0 = a0·b0
/// d1 = a0·b1 + a1·b0
/// d2 = a0·b2 + a1·b1 + a2·b0
/// d3 = a1·b2 + a2·b1
/// d4 = a2·b2
///
/// // Reduce: v³ → β, v⁴ → β·v
/// c0 = d0 + β·d3
/// c1 = d1 + β·d4
/// c2 = d2
/// ```
#[inline(never)]
pub fn naive_cubic_mul<C: ExtConfig>(a: CubicExt<C>, b: CubicExt<C>) -> CubicExt<C> {
    let a0 = a.c0();
    let a1 = a.c1();
    let a2 = a.c2();
    let b0 = b.c0();
    let b1 = b.c1();
    let b2 = b.c2();

    let m00 = a0 * b0;
    let m01 = a0 * b1;
    let m02 = a0 * b2;
    let m10 = a1 * b0;
    let m11 = a1 * b1;
    let m12 = a1 * b2;
    let m20 = a2 * b0;
    let m21 = a2 * b1;
    let m22 = a2 * b2;

    let d0 = m00;
    let d1 = m01 + m10;
    let d2 = m02 + m11 + m20;
    let d3 = m12 + m21;
    let d4 = m22;

    let c0 = d0 + C::mul_by_non_residue(d3);
    let c1 = d1 + C::mul_by_non_residue(d4);
    let c2 = d2;

    CubicExt::new(c0, c1, c2)
}

/// Build an [`Fq4Large`] from `[c00, c01, c10, c11]` where the pairs are the
/// two inner `Fq2Large` coordinates.
pub fn fq4_from_flat(c: [u64; 4]) -> Fq4Large {
    let c0 = Fq2Large::new(Fp::<65537>::new(c[0]), Fp::<65537>::new(c[1]));
    let c1 = Fq2Large::new(Fp::<65537>::new(c[2]), Fp::<65537>::new(c[3]));
    Fq4Large::new(c0, c1)
}

/// Serialise an [`Fq4Large`] back into `[c00, c01, c10, c11]`.
pub fn fq4_to_flat(e: Fq4Large) -> [u64; 4] {
    [
        e.c0().c0().value(),
        e.c0().c1().value(),
        e.c1().c0().value(),
        e.c1().c1().value(),
    ]
}

/// Build an [`Fq6Small`] from six Fp-coefficients, innermost coordinate
/// varying fastest.
pub fn fq6_from_flat(c: [u64; 6]) -> Fq6Small {
    let c0 = Fq2Small::new(Fp::<7>::new(c[0]), Fp::<7>::new(c[1]));
    let c1 = Fq2Small::new(Fp::<7>::new(c[2]), Fp::<7>::new(c[3]));
    let c2 = Fq2Small::new(Fp::<7>::new(c[4]), Fp::<7>::new(c[5]));
    Fq6Small::new(c0, c1, c2)
}

/// Serialise an [`Fq6Small`] back to six Fp-coefficients.
pub fn fq6_to_flat(e: Fq6Small) -> [u64; 6] {
    [
        e.c0().c0().value(),
        e.c0().c1().value(),
        e.c1().c0().value(),
        e.c1().c1().value(),
        e.c2().c0().value(),
        e.c2().c1().value(),
    ]
}

/// Build an [`Fq12Small`] from twelve Fp-coefficients: the first six form
/// the "z⁰" half, the last six the "z¹" half.
pub fn fq12_from_flat(c: [u64; 12]) -> Fq12Small {
    let lo: [u64; 6] = [c[0], c[1], c[2], c[3], c[4], c[5]];
    let hi: [u64; 6] = [c[6], c[7], c[8], c[9], c[10], c[11]];
    Fq12Small::new(fq6_from_flat(lo), fq6_from_flat(hi))
}

/// Serialise an [`Fq12Small`] back to twelve Fp-coefficients.
pub fn fq12_to_flat(e: Fq12Small) -> [u64; 12] {
    let lo = fq6_to_flat(e.c0());
    let hi = fq6_to_flat(e.c1());
    [
        lo[0], lo[1], lo[2], lo[3], lo[4], lo[5], hi[0], hi[1], hi[2], hi[3], hi[4], hi[5],
    ]
}
