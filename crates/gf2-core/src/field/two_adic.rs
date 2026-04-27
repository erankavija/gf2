//! Two-adic finite fields: primitive 2^k-th roots of unity.
//!
//! A *two-adic* finite field is one whose multiplicative group `F^*` contains
//! a power-of-two subgroup large enough to support radix-2 Fast Fourier
//! Transform / Number Theoretic Transform (NTT) butterflies. Concretely, for
//! a prime field `GF(P)` with `P − 1 = m · 2^k` and `k ≥ 1`, the field admits
//! a primitive `2^k`-th root of unity — the NTT transform length is then
//! capped at `2^k`.
//!
//! This module introduces the [`TwoAdicField`] trait which surfaces two
//! pieces of data per supported field:
//!
//! - [`TwoAdicField::TWO_ADICITY`] — the exponent `k` (largest power of two
//!   dividing the multiplicative-group order).
//! - [`TwoAdicField::two_adic_generator`] — a fixed primitive `2^TWO_ADICITY`-th
//!   root of unity. All smaller `2^j`-th primitive roots (`j ≤ TWO_ADICITY`)
//!   are obtained by repeated squaring via [`TwoAdicField::two_adic_root_of_unity`].
//!
//! # Supported primes
//!
//! Concrete impls are provided for the Proth primes that matter for the
//! NTT code paths in this workspace:
//!
//! | Prime        | Value                      | `TWO_ADICITY` | Generator (value form) |
//! |--------------|----------------------------|---------------|------------------------|
//! | `Fp<65537>`  | `2^16 + 1` (Fermat prime)  | `16`          | `3`                    |
//! | `Fp<BABYBEAR_P>` | `15 · 2^27 + 1 = 2_013_265_921` | `27` | `440_564_289` (`0x1a42_7a41`) |
//! | `Fp<KOALABEAR_P>` | `127 · 2^24 + 1 = 2_130_706_433` | `24` | `1_791_270_792` (`0x6ac4_9f88`) |
//!
//! Generators are standard constants from the Plonky3 family of zk-friendly
//! prime-field implementations; each is verified by a unit test in this
//! module asserting both the `g^(2^TWO_ADICITY) = 1` identity *and* the
//! primitivity condition `g^(2^(TWO_ADICITY − 1)) ≠ 1`.
//!
//! ## Why not `Fp<P>` for all `P`?
//!
//! The trait constant `TWO_ADICITY` must be compile-time, which (on stable
//! Rust without nightly features) cannot be derived from the const-generic
//! `P` inside the trait impl itself. We therefore provide concrete impls
//! only for the Proth primes currently used in the workspace; see the
//! *workaround* note below for the per-prime helper that extracts
//! `TWO_ADICITY` from [`classify`](crate::gfp::specialized::classify).
//!
//! ## Not implemented for `Gf2mElement`
//!
//! For a binary extension field `GF(2^m)` the multiplicative group has
//! *odd* order `2^m − 1`, so the only power of two dividing `|F^*|` is
//! `2^0 = 1`. A `TwoAdicField` impl with `TWO_ADICITY = 0` would be
//! useless for NTT purposes — the only `2^0`-th root of unity is `1`
//! itself — so we deliberately do not implement [`TwoAdicField`] for
//! `Gf2mElement`. Callers needing an FFT over `GF(2^m)` should use the
//! additive (Gao–Mateer / Lin–Chung–Han) FFT instead, which is outside
//! the scope of this trait.
//!
//! # Extracting `TWO_ADICITY` at compile time
//!
//! The Proth classification lives in
//! [`crate::gfp::specialized::classify`] and is already a `const fn`. We
//! expose a thin [`ProthTwoAdicity<P>`] helper whose associated
//! `TWO_ADICITY` constant re-exports the exponent `n` from
//! `PrimeShape::Proth { n, .. }`. Concrete [`TwoAdicField`] impls then
//! delegate to this helper; the indirection keeps the math inside the
//! existing specialised-prime classifier.
//!
//! # Examples
//!
//! ```
//! use gf2_core::field::{FiniteField, FiniteFieldExt, TwoAdicField};
//! use gf2_core::gfp::Fp;
//!
//! // A primitive 2^4-th root of unity in Fp<65537>.
//! let w = <Fp<65537> as TwoAdicField>::two_adic_root_of_unity(4);
//! // Its 16th power is 1.
//! assert!(w.pow(16).is_one());
//! // Its 8th power is −1 (primitivity check).
//! assert_eq!(w.pow(8), -Fp::<65537>::new(1));
//! ```

use crate::field::{FiniteField, FiniteFieldExt};
use crate::gfp::specialized::{classify, PrimeShape};
use crate::gfp::Fp;

/// Marker trait for finite fields whose multiplicative group contains a large
/// power-of-two subgroup, enabling radix-2 NTT / FFT butterflies.
///
/// Implementors expose:
///
/// - The largest `k` such that `2^k | |F^*|` (the *two-adicity*).
/// - A fixed generator of the 2^k-th roots-of-unity subgroup.
/// - A derived accessor for every smaller `2^j`-th primitive root
///   (`j ≤ k`) via repeated squaring.
///
/// This is the field-side API a radix-2 NTT implementation needs; it does
/// **not** imply any specific fast-multiplication algorithm and does not
/// prescribe a transform length.
///
/// # Implementors
///
/// This trait is currently implemented for the Proth primes used in the
/// workspace's NTT code paths:
///
/// - [`Fp<65537>`] — the Fermat prime `2^16 + 1`, with `TWO_ADICITY = 16`.
/// - [`Fp<BABYBEAR_P>`] — Plonky3's 31-bit BabyBear prime `15·2^27 + 1`,
///   with `TWO_ADICITY = 27`.
/// - [`Fp<KOALABEAR_P>`] — Plonky3's 31-bit KoalaBear prime `127·2^24 + 1`,
///   with `TWO_ADICITY = 24`.
///
/// Additional Proth primes can be added by providing a concrete impl that
/// forwards `TWO_ADICITY` through [`ProthTwoAdicity`] and supplies a
/// verified generator value.
///
/// # Not implemented for `Gf2mElement`
///
/// Binary extension fields `GF(2^m)` have multiplicative group of *odd*
/// order `2^m − 1`, so the only power of two dividing `|F^*|` is
/// `2^0 = 1`. A `TwoAdicField` impl with `TWO_ADICITY = 0` would be
/// useless for NTT purposes — the only `2^0`-th root of unity is `1` —
/// so we deliberately do not implement [`TwoAdicField`] for
/// [`Gf2mElement`](crate::gf2m::Gf2mElement). Callers needing an FFT
/// over `GF(2^m)` should use the additive (Gao–Mateer / Lin–Chung–Han)
/// FFT instead, which is outside the scope of this trait.
///
/// # Arguments
///
/// (Trait-level — per-method arguments are documented on
/// [`two_adic_generator`](Self::two_adic_generator) and
/// [`two_adic_root_of_unity`](Self::two_adic_root_of_unity).)
///
/// # Examples
///
/// ```
/// use gf2_core::field::{FiniteField, FiniteFieldExt, TwoAdicField};
/// use gf2_core::gfp::Fp;
///
/// let w = <Fp<65537> as TwoAdicField>::two_adic_root_of_unity(2);
/// // w is a primitive 4th root of unity, so w^4 == 1 but w^2 != 1.
/// assert!(w.pow(4).is_one());
/// assert!(!w.pow(2).is_one());
/// ```
///
/// # Panics
///
/// No methods of this trait panic on valid inputs. The provided
/// [`two_adic_root_of_unity`](Self::two_adic_root_of_unity) default
/// panics when `k > TWO_ADICITY` (out of the supported range); see
/// its method doc for details.
///
/// # Complexity
///
/// The trait itself adds no runtime cost. See
/// [`two_adic_generator`](Self::two_adic_generator) and
/// [`two_adic_root_of_unity`](Self::two_adic_root_of_unity) for the
/// per-method complexity of the concrete accessors.
pub trait TwoAdicField: FiniteField {
    /// The largest `k` such that `2^k` divides `|F^*|`.
    ///
    /// For a prime field `GF(P)` this is the exponent `k` in the
    /// factorisation `P − 1 = m · 2^k` with `m` odd.
    ///
    /// # Arguments
    ///
    /// Not applicable — this is an associated constant with no operands.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::TwoAdicField;
    /// use gf2_core::gfp::Fp;
    ///
    /// assert_eq!(<Fp<65537> as TwoAdicField>::TWO_ADICITY, 16);
    /// ```
    ///
    /// # Panics
    ///
    /// Not applicable — evaluated at compile time, no runtime execution.
    ///
    /// # Complexity
    ///
    /// Not applicable — a compile-time constant, zero runtime cost.
    const TWO_ADICITY: u32;

    /// A fixed generator of the `2^TWO_ADICITY`-th roots-of-unity subgroup.
    ///
    /// All smaller `2^j`-th primitive roots are obtained by squaring
    /// this generator `TWO_ADICITY − j` times — see
    /// [`two_adic_root_of_unity`](Self::two_adic_root_of_unity).
    ///
    /// # Arguments
    ///
    /// No runtime arguments — the type parameter `Self` is implicit and
    /// determines which field's generator is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt, TwoAdicField};
    /// use gf2_core::gfp::Fp;
    ///
    /// let g = <Fp<65537> as TwoAdicField>::two_adic_generator();
    /// // g is a primitive 2^16-th root of unity: g^(2^16) = 1.
    /// assert!(g.pow(1u64 << 16).is_one());
    /// // Primitivity: g^(2^15) != 1.
    /// assert!(!g.pow(1u64 << 15).is_one());
    /// ```
    ///
    /// # Panics
    ///
    /// Does not panic — all supplied impls return a hard-coded constant
    /// value. Implementors that cannot produce a valid generator should
    /// decline to implement the trait rather than panic.
    ///
    /// # Complexity
    ///
    /// O(1) — the generator is returned directly as a compile-time
    /// constant. No field arithmetic is performed.
    fn two_adic_generator() -> Self;

    /// A primitive `2^k`-th root of unity, for any `k` in `[0, TWO_ADICITY]`.
    ///
    /// By convention the 2^0-th root of unity is 1, the 2^1-th is −1,
    /// and the 2^TWO_ADICITY-th is [`two_adic_generator`](Self::two_adic_generator).
    ///
    /// # Arguments
    ///
    /// * `k` - Target exponent; must satisfy `k ≤ TWO_ADICITY`.
    ///
    /// # Panics
    ///
    /// Panics if `k > TWO_ADICITY` — a larger power-of-two root of unity
    /// does not exist in the field.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt, TwoAdicField};
    /// use gf2_core::gfp::Fp;
    ///
    /// // 2^0-th root is 1.
    /// assert!(<Fp<65537> as TwoAdicField>::two_adic_root_of_unity(0).is_one());
    /// // 2^1-th root is −1: its square is 1, itself is not.
    /// let w1 = <Fp<65537> as TwoAdicField>::two_adic_root_of_unity(1);
    /// assert!(w1.pow(2).is_one());
    /// assert!(!w1.is_one());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(log(2^(TWO_ADICITY − k))) = O(TWO_ADICITY − k) field multiplications
    /// via square-and-multiply.
    fn two_adic_root_of_unity(k: u32) -> Self {
        assert!(
            k <= Self::TWO_ADICITY,
            "requested 2^{k}-th root of unity exceeds field two-adicity {}",
            Self::TWO_ADICITY
        );
        Self::two_adic_generator().pow(1u64 << (Self::TWO_ADICITY - k))
    }
}

// ---------------------------------------------------------------------------
// Const-generic helper: compile-time TWO_ADICITY for Proth primes.
// ---------------------------------------------------------------------------

/// Compile-time extractor for the two-adicity of a Proth prime.
///
/// This helper exists because Rust's stable const-eval rules do not allow
/// us to call [`classify`] directly inside the body of
/// `TwoAdicField::TWO_ADICITY` when `P` is const-generic on the impl.
/// By routing through a zero-sized helper whose associated constant is
/// itself `const`-evaluated at monomorphisation time, we recover full
/// compile-time derivation from the const-generic parameter `P`.
///
/// For a Proth prime `P = k · 2^n + 1`, `TWO_ADICITY` equals `n`.
/// For any other prime shape the constant is `0` (and the corresponding
/// [`TwoAdicField`] impl is deliberately not provided — Proth is the only
/// shape the trait currently supports).
///
/// # Arguments
///
/// * `P` — const-generic `u64` modulus. Must be an odd prime ≤ 2^63 (the
///   same bound enforced by [`Fp`]) and should be of Proth shape
///   (`P = k · 2^n + 1` with odd `k`) for the extracted `TWO_ADICITY` to
///   be meaningful. When `P` is not Proth, the classifier returns
///   `PrimeShape::Other` and this helper reports `TWO_ADICITY = 0`; in
///   that case there is no matching [`TwoAdicField`] impl, so the helper
///   cannot be composed with the trait-level API for that `P`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::two_adic::ProthTwoAdicity;
///
/// // Fp<65537> is the Fermat prime 2^16 + 1 = 1 · 2^16 + 1.
/// assert_eq!(ProthTwoAdicity::<65537>::TWO_ADICITY, 16);
/// ```
///
/// # Panics
///
/// None. The associated [`TWO_ADICITY`](Self::TWO_ADICITY) constant is a
/// total `const fn` match on [`PrimeShape`] and cannot fail at
/// monomorphisation time.
///
/// # Complexity
///
/// O(1) at runtime — every read is a compile-time constant.
/// [`classify`] itself is `O(log P)` but runs exclusively at
/// monomorphisation, contributing nothing to runtime cost.
///
/// # References
///
/// The Proth prime shape `k · 2^n + 1` is the classical form introduced
/// by François Proth (1878); see e.g. Crandall & Pomerance, *Prime
/// Numbers: A Computational Perspective*, §4.2.2. The Plonky3 zk-STARK
/// framework (<https://github.com/Plonky3/Plonky3>) popularised this
/// shape for fast NTTs over 31-bit fields.
pub struct ProthTwoAdicity<const P: u64>;

impl<const P: u64> ProthTwoAdicity<P> {
    /// Two-adicity of `P` — the largest `n` with `2^n | (P − 1)`, assuming
    /// `P` has Proth shape. Zero otherwise.
    ///
    /// # Arguments
    ///
    /// Not applicable — the modulus `P` is supplied as a const-generic
    /// parameter on the enclosing [`ProthTwoAdicity`] struct.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::two_adic::{BABYBEAR_P, KOALABEAR_P, ProthTwoAdicity};
    ///
    /// assert_eq!(ProthTwoAdicity::<65537>::TWO_ADICITY, 16);
    /// assert_eq!(ProthTwoAdicity::<BABYBEAR_P>::TWO_ADICITY, 27);
    /// assert_eq!(ProthTwoAdicity::<KOALABEAR_P>::TWO_ADICITY, 24);
    /// // Non-Proth primes report 0.
    /// assert_eq!(ProthTwoAdicity::<7>::TWO_ADICITY, 0);
    /// ```
    ///
    /// # Panics
    ///
    /// Not applicable — a total `const fn` match on [`PrimeShape`].
    ///
    /// # Complexity
    ///
    /// Not applicable — a compile-time constant, zero runtime cost.
    pub const TWO_ADICITY: u32 = match classify(P) {
        PrimeShape::Proth { n, .. } => n,
        _ => 0,
    };
}

// ---------------------------------------------------------------------------
// Concrete TwoAdicField impls for supported Proth primes.
// ---------------------------------------------------------------------------
//
// Each generator below is verified by the unit tests at the bottom of the
// file; the reference values come from the Plonky3 canonical constants
// (https://github.com/Plonky3/Plonky3), cross-checked with a hand
// computation: pick a multiplicative generator `g_mult` of F^* and raise
// it to the cofactor `m = (P − 1) / 2^TWO_ADICITY`.

/// `Fp<65537>` — the Fermat prime `2^16 + 1`.
///
/// `P − 1 = 2^16`, so `TWO_ADICITY = 16`. The multiplicative group is
/// cyclic of order `2^16` and `3` is a primitive root mod 65537 (a
/// well-known classical fact, see e.g. Hardy & Wright §7.3). Since the
/// cofactor `m = 1`, `3` itself is the 2^16-th primitive root of unity.
impl TwoAdicField for Fp<65537> {
    const TWO_ADICITY: u32 = ProthTwoAdicity::<65537>::TWO_ADICITY;

    fn two_adic_generator() -> Self {
        Fp::<65537>::new(3)
    }
}

/// `Fp<BABYBEAR_P>` where `BABYBEAR_P = 15 · 2^27 + 1 = 2_013_265_921`.
///
/// `TWO_ADICITY = 27`. The Plonky3 canonical generator `g_mult = 31` is
/// a multiplicative generator of `F^*`; the 2^27-th primitive root of
/// unity is then `31^15 mod P = 440_564_289 = 0x1a42_7a41`.
impl TwoAdicField for Fp<{ BABYBEAR_P }> {
    const TWO_ADICITY: u32 = ProthTwoAdicity::<{ BABYBEAR_P }>::TWO_ADICITY;

    fn two_adic_generator() -> Self {
        // 31^15 mod P — verified in `tests::babybear_generator_is_primitive`.
        Fp::<{ BABYBEAR_P }>::new(0x1a42_7a41)
    }
}

/// `Fp<KOALABEAR_P>` where `KOALABEAR_P = 127 · 2^24 + 1 = 2_130_706_433`.
///
/// `TWO_ADICITY = 24`. The Plonky3 canonical generator `g_mult = 3` is
/// a multiplicative generator of `F^*`; the 2^24-th primitive root of
/// unity is then `3^127 mod P = 1_791_270_792 = 0x6ac4_9f88`.
impl TwoAdicField for Fp<{ KOALABEAR_P }> {
    const TWO_ADICITY: u32 = ProthTwoAdicity::<{ KOALABEAR_P }>::TWO_ADICITY;

    fn two_adic_generator() -> Self {
        // 3^127 mod P — verified in `tests::koalabear_generator_is_primitive`.
        Fp::<{ KOALABEAR_P }>::new(0x6ac4_9f88)
    }
}

/// BabyBear Proth prime: `15 · 2^27 + 1 = 2_013_265_921`.
///
/// Exact value — a 31-bit Proth prime with two-adicity 27, introduced as
/// the canonical zk-STARK field by the Plonky3 framework
/// (<https://github.com/Plonky3/Plonky3>) and subsequently adopted by
/// RISC Zero (`risc0-zkp`) and the AIR/plonkish zk ecosystem at large.
/// Composes with [`TwoAdicField`] via the [`Fp<{ BABYBEAR_P }>`] impl in
/// this module, which fixes the 2^27-th primitive root of unity to the
/// Plonky3 canonical constant `0x1a42_7a41`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::two_adic::BABYBEAR_P;
///
/// assert_eq!(BABYBEAR_P, 2_013_265_921);
/// assert_eq!(BABYBEAR_P, 15 * (1u64 << 27) + 1);
/// ```
///
/// # Panics
///
/// None — this is a compile-time constant.
///
/// # Complexity
///
/// O(1).
pub const BABYBEAR_P: u64 = 15 * (1u64 << 27) + 1;

/// KoalaBear Proth prime: `127 · 2^24 + 1 = 2_130_706_433`, equivalently
/// `2^31 − 2^24 + 1`.
///
/// Exact value — a 31-bit Proth prime with two-adicity 24, also
/// introduced by Plonky3 (<https://github.com/Plonky3/Plonky3>) as a
/// companion to BabyBear: the multiplicative group has slightly larger
/// cofactor (`127` instead of `15`) at the cost of a smaller
/// two-adicity. Composes with [`TwoAdicField`] via the
/// [`Fp<{ KOALABEAR_P }>`] impl in this module, which fixes the 2^24-th
/// primitive root of unity to the Plonky3 canonical constant
/// `0x6ac4_9f88`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::two_adic::KOALABEAR_P;
///
/// assert_eq!(KOALABEAR_P, 2_130_706_433);
/// assert_eq!(KOALABEAR_P, 127 * (1u64 << 24) + 1);
/// assert_eq!(KOALABEAR_P, (1u64 << 31) - (1u64 << 24) + 1);
/// ```
///
/// # Panics
///
/// None — this is a compile-time constant.
///
/// # Complexity
///
/// O(1).
pub const KOALABEAR_P: u64 = 127 * (1u64 << 24) + 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FiniteFieldExt;

    // --- Two-adicity compile-time constants ---

    #[test]
    fn test_two_adicity_constants_match_classify() {
        assert_eq!(<Fp<65537> as TwoAdicField>::TWO_ADICITY, 16);
        assert_eq!(<Fp<{ BABYBEAR_P }> as TwoAdicField>::TWO_ADICITY, 27);
        assert_eq!(<Fp<{ KOALABEAR_P }> as TwoAdicField>::TWO_ADICITY, 24);
    }

    #[test]
    fn test_babybear_p_value_matches_plonky3_constant() {
        assert_eq!(BABYBEAR_P, 2_013_265_921);
    }

    #[test]
    fn test_koalabear_p_value_matches_plonky3_constant() {
        assert_eq!(KOALABEAR_P, 2_130_706_433);
        assert_eq!(KOALABEAR_P, (1u64 << 31) - (1u64 << 24) + 1);
    }

    // --- Generator primitivity: g^(2^TWO_ADICITY) == 1 and
    //     g^(2^(TWO_ADICITY − 1)) != 1 for every supported prime. ---

    fn assert_generator_primitive<F: TwoAdicField + Copy>() {
        let g = F::two_adic_generator();
        let one = g.one_like();
        // g^(2^k) must be one.
        assert!(
            g.pow(1u64 << F::TWO_ADICITY) == one,
            "two_adic_generator^(2^TWO_ADICITY) must equal 1",
        );
        // g^(2^(k−1)) must NOT be one (primitivity of order 2^k).
        assert!(F::TWO_ADICITY >= 1);
        assert!(
            g.pow(1u64 << (F::TWO_ADICITY - 1)) != one,
            "two_adic_generator^(2^(TWO_ADICITY − 1)) must not equal 1",
        );
    }

    #[test]
    fn test_generator_is_primitive_fp_65537() {
        assert_generator_primitive::<Fp<65537>>();
    }

    #[test]
    fn test_generator_is_primitive_babybear() {
        assert_generator_primitive::<Fp<{ BABYBEAR_P }>>();
    }

    #[test]
    fn test_generator_is_primitive_koalabear() {
        assert_generator_primitive::<Fp<{ KOALABEAR_P }>>();
    }

    // --- two_adic_root_of_unity edge cases ---

    fn assert_small_roots<F: TwoAdicField + Copy>() {
        let one = F::two_adic_generator().one_like();

        // k = 0: the only 2^0-th root of unity is 1.
        let w0 = F::two_adic_root_of_unity(0);
        assert_eq!(w0, one, "2^0-th root of unity must be 1");

        // k = 1: primitive square root of 1 is −1 in any field of
        // characteristic != 2. Equivalently: w^2 = 1 but w != 1.
        let w1 = F::two_adic_root_of_unity(1);
        assert!(w1.pow(2).is_one(), "2^1-th root squared must be 1");
        assert!(w1 != one, "2^1-th root must be primitive (not 1)");

        // k = TWO_ADICITY: matches the canonical generator.
        let w_max = F::two_adic_root_of_unity(F::TWO_ADICITY);
        assert_eq!(
            w_max,
            F::two_adic_generator(),
            "2^TWO_ADICITY-th root must be the canonical generator",
        );

        // For every k ∈ [0, TWO_ADICITY], squaring w_k gives w_{k−1}.
        let mut w = F::two_adic_generator();
        for k in (1..=F::TWO_ADICITY).rev() {
            let expected = F::two_adic_root_of_unity(k);
            assert_eq!(
                w, expected,
                "ladder mismatch: iterated squaring disagrees with root-of-unity accessor at k={k}",
            );
            w = w * w;
        }
        assert_eq!(w, one, "final squaring from the 2^1-th root must give 1",);
    }

    #[test]
    fn test_small_roots_fp_65537() {
        assert_small_roots::<Fp<65537>>();
    }

    #[test]
    fn test_small_roots_babybear() {
        assert_small_roots::<Fp<{ BABYBEAR_P }>>();
    }

    #[test]
    fn test_small_roots_koalabear() {
        assert_small_roots::<Fp<{ KOALABEAR_P }>>();
    }

    // --- Panic tests: k > TWO_ADICITY ---

    #[test]
    #[should_panic(expected = "exceeds field two-adicity")]
    fn test_root_of_unity_panics_when_k_exceeds_two_adicity_fp_65537() {
        let _ = <Fp<65537> as TwoAdicField>::two_adic_root_of_unity(17);
    }

    #[test]
    #[should_panic(expected = "exceeds field two-adicity")]
    fn test_root_of_unity_panics_when_k_exceeds_two_adicity_babybear() {
        let _ = <Fp<{ BABYBEAR_P }> as TwoAdicField>::two_adic_root_of_unity(28);
    }

    #[test]
    #[should_panic(expected = "exceeds field two-adicity")]
    fn test_root_of_unity_panics_when_k_exceeds_two_adicity_koalabear() {
        let _ = <Fp<{ KOALABEAR_P }> as TwoAdicField>::two_adic_root_of_unity(25);
    }

    // --- Concrete generator values (regression guard against accidental
    //     changes to the hard-coded constants). ---

    #[test]
    fn test_generator_value_is_3_fp_65537() {
        assert_eq!(<Fp<65537> as TwoAdicField>::two_adic_generator().value(), 3,);
    }

    #[test]
    fn test_generator_value_matches_plonky3_babybear() {
        assert_eq!(
            <Fp<{ BABYBEAR_P }> as TwoAdicField>::two_adic_generator().value(),
            0x1a42_7a41,
        );
    }

    #[test]
    fn test_generator_value_matches_plonky3_koalabear() {
        assert_eq!(
            <Fp<{ KOALABEAR_P }> as TwoAdicField>::two_adic_generator().value(),
            0x6ac4_9f88,
        );
    }

    // --- Cross-check: reproduce the generator by exponentiating a
    //     multiplicative generator of F^* by the cofactor m = (P−1)/2^k. ---

    #[test]
    fn test_generator_matches_cofactor_exponentiation_fp_65537() {
        // 3 is a primitive root mod 65537 (Hardy & Wright §7.3).
        // Cofactor m = (P-1)/2^16 = 1, so the two-adic generator is 3^1 = 3.
        let g_mult = Fp::<65537>::new(3);
        let cofactor: u64 = (65537u64 - 1) >> 16; // = 1
        let reconstructed = g_mult.pow(cofactor);
        assert_eq!(
            reconstructed,
            <Fp<65537> as TwoAdicField>::two_adic_generator(),
        );
    }

    #[test]
    fn test_generator_matches_cofactor_exponentiation_babybear() {
        // Plonky3 uses g_mult = 31 as a multiplicative generator for BabyBear.
        // Cofactor m = (P-1)/2^27 = 15.
        let g_mult = Fp::<{ BABYBEAR_P }>::new(31);
        let cofactor: u64 = (BABYBEAR_P - 1) >> 27; // = 15
        let reconstructed = g_mult.pow(cofactor);
        assert_eq!(
            reconstructed,
            <Fp<{ BABYBEAR_P }> as TwoAdicField>::two_adic_generator(),
        );
    }

    #[test]
    fn test_generator_matches_cofactor_exponentiation_koalabear() {
        // Plonky3 uses g_mult = 3 as a multiplicative generator for KoalaBear.
        // Cofactor m = (P-1)/2^24 = 127.
        let g_mult = Fp::<{ KOALABEAR_P }>::new(3);
        let cofactor: u64 = (KOALABEAR_P - 1) >> 24; // = 127
        let reconstructed = g_mult.pow(cofactor);
        assert_eq!(
            reconstructed,
            <Fp<{ KOALABEAR_P }> as TwoAdicField>::two_adic_generator(),
        );
    }
}
