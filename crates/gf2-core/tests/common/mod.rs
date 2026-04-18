//! Shared helpers for tower-extension integration tests.
//!
//! This module is consumed by multiple integration-test binaries via the
//! standard Rust `tests/common/mod.rs` pattern. The shared items are the
//! canonical definitions — no integration test should re-declare a tower
//! config or a schoolbook multiplier of its own; add new primitives here
//! instead.
//!
//! # Contents
//!
//! * **Tower configurations** for the three nested chains exercised by the
//!   `gfpn_*` integration tests (GF(65537²), GF(65537⁴), GF(7²), GF(7⁶),
//!   GF(7¹²)) plus the four shallow configurations used by
//!   `karatsuba_cross_verify.rs`.
//! * **Naive schoolbook multipliers** for [`QuadraticExt`] (4 base muls) and
//!   [`CubicExt`] (9 base muls). They use only the public accessors of the
//!   tower types and the base field's operators, so a bug in the optimised
//!   Karatsuba path cannot hide behind a shared subroutine.
//! * **Flat `[u64; N]` → tower-element builders** for the nested chains, so
//!   Sage-generated cross-verify vectors can be decoded with a single call.
//!
//! # Rust integration-test module pattern
//!
//! Each integration test binary that wants to pull these helpers in should
//! declare:
//!
//! ```ignore
//! mod common;
//! use common::{Fp65537Ext2, /* ... */};
//! ```
//!
//! The `tests/common/mod.rs` file is the idiomatic way to share code between
//! integration tests (the alternative `#[path = "..."] mod common;` trick is
//! avoided here for readability). Because every symbol in this module is used
//! by at least one consumer, but not every consumer pulls every symbol, each
//! item is tagged `#[allow(dead_code)]` so the unused-symbol lint does not
//! fire per-binary.

#![allow(dead_code)]
#![deny(unsafe_code)]

use gf2_core::gfp::Fp;
use gf2_core::gfpn::{CubicExt, ExtConfig, QuadraticExt};

// ---------------------------------------------------------------------------
// Tower configurations: nested chains (GF(p^4), GF(p^6), GF(p^12))
// ---------------------------------------------------------------------------

/// GF(65537²) = `Fp<65537>[u]/(u² − 3)`.
///
/// 65537 is a Fermat prime and 3 is a quadratic non-residue mod 65537
/// (verified via Legendre symbol). Used both as the base for the GF(65537⁴)
/// tower and as a standalone `QuadraticExt` target.
pub struct Fp65537Ext2;
impl ExtConfig for Fp65537Ext2 {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
pub type Fq2Large = QuadraticExt<Fp65537Ext2>;

/// GF(65537⁴) = `Fq2Large[w]/(w² − u)`.
///
/// `u` is a non-square in `Fq2Large` because the base non-residue 3 is not a
/// square in `Fp<65537>` — this is the canonical GF(p⁴) construction via two
/// quadratic extensions.
pub struct Fp65537Ext4;
impl ExtConfig for Fp65537Ext4 {
    type BaseField = Fq2Large;
    const NON_RESIDUE: Fq2Large = Fq2Large::new(Fp::<65537>::new(0), Fp::<65537>::new(1));
}
pub type Fq4Large = QuadraticExt<Fp65537Ext4>;

/// GF(7²) = `Fp<7>[u]/(u² − 3)`. Base for the GF(7⁶) and GF(7¹²) towers.
pub struct Fp7Ext2;
impl ExtConfig for Fp7Ext2 {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
}
pub type Fq2Small = QuadraticExt<Fp7Ext2>;

/// GF(7⁶) = `Fq2Small[v]/(v³ − u)`.
///
/// `u` is a cubic non-residue in GF(7²) — verified via SageMath
/// (`(t^3 - u).is_irreducible() == True` over GF(7²)).
pub struct Fp7Ext6;
impl ExtConfig for Fp7Ext6 {
    type BaseField = Fq2Small;
    const NON_RESIDUE: Fq2Small = Fq2Small::new(Fp::<7>::new(0), Fp::<7>::new(1));
}
pub type Fq6Small = CubicExt<Fp7Ext6>;

/// GF(7¹²) = `Fq6Small[z]/(z² − (v + 1))`.
///
/// `v + 1` is a quadratic non-residue in GF(7⁶) — verified via SageMath.
/// This is the BLS12-381-shape tower (quad-over-cubic-over-quad), but with
/// p = 7 for speed.
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

// ---------------------------------------------------------------------------
// Tower configurations: shallow (single-level) chains used by
// `karatsuba_cross_verify.rs`.
// ---------------------------------------------------------------------------

/// GF(7²) with β = 6 ≡ −1 (mod 7). Overridden `mul_by_non_residue` (negation).
pub struct Fq2Fp7NegOneConfig;
impl ExtConfig for Fq2Fp7NegOneConfig {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1

    #[inline]
    fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
        -x
    }
}
pub type Fq2Fp7NegOne = QuadraticExt<Fq2Fp7NegOneConfig>;

/// GF(7²) with β = 3 and the default `mul_by_non_residue`.
pub struct Fq2Fp7Beta3Config;
impl ExtConfig for Fq2Fp7Beta3Config {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
}
pub type Fq2Fp7Beta3 = QuadraticExt<Fq2Fp7Beta3Config>;

/// GF(101²) with β = 99 ≡ −2 (mod 101). Default `mul_by_non_residue`.
pub struct Fq2Fp101NegTwoConfig;
impl ExtConfig for Fq2Fp101NegTwoConfig {
    type BaseField = Fp<101>;
    const NON_RESIDUE: Fp<101> = Fp::<101>::new(99);
}
pub type Fq2Fp101NegTwo = QuadraticExt<Fq2Fp101NegTwoConfig>;

/// GF(65537²) with β = 3. Fermat prime, default `mul_by_non_residue`. Shares
/// the same mathematical field as [`Fq2Large`] but with the default
/// non-residue multiplier (kept separate to distinguish the Karatsuba code
/// path from the nested-tower tests).
pub struct Fq2Fp65537Beta3Config;
impl ExtConfig for Fq2Fp65537Beta3Config {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
pub type Fq2Fp65537Beta3 = QuadraticExt<Fq2Fp65537Beta3Config>;

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

/// GF(101³) with β = 2. Default `mul_by_non_residue`.
///
/// Note: gcd(3, 100) = 1, so every element of `Fp<101>` is a cube and x³ − 2
/// is reducible. The Karatsuba-vs-naive cross-check nevertheless holds —
/// both formulas compute the same polynomial product modulo x³ − β
/// regardless of whether the ring is a field.
pub struct Fq3Fp101Beta2Config;
impl ExtConfig for Fq3Fp101Beta2Config {
    type BaseField = Fp<101>;
    const NON_RESIDUE: Fp<101> = Fp::<101>::new(2);
}
pub type Fq3Fp101Beta2 = CubicExt<Fq3Fp101Beta2Config>;

// ---------------------------------------------------------------------------
// Naive schoolbook multipliers (SSOT for all integration tests).
//
// These implementations use ONLY public accessors (`c0`, `c1`, `c2`) and the
// base field's own operators. They deliberately do NOT call the optimised
// `Mul` impl or any of its helpers, so a Karatsuba bug cannot silently hide
// behind a shared subroutine. Each integration test that needs to
// cross-check Karatsuba against schoolbook imports these functions from here
// instead of re-implementing them.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Flat coefficient encoders/decoders for the nested chains.
//
// Coefficients are stored in the canonical "innermost varying fastest" order
// used by the Sage generator, so cross-verify data and Rust construction
// agree byte-for-byte.
// ---------------------------------------------------------------------------

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

/// Build an [`Fq6Small`] from six Fp-coefficients.
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
