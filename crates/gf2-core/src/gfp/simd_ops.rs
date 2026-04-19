//! SIMD element-wise vector-op hooks for `Fp<P>`.
//!
//! This module exposes [`SimdVecOps`], the single source of truth for
//! element-wise SIMD dispatch used by [`crate::field::FieldVec`] and
//! [`crate::gfpn::BatchExtField`]. The trait carries default-`None`
//! implementations that make every `FiniteField` compile unchanged;
//! specific primes override the hooks to route through AVX2 kernels in
//! `gf2-kernels-simd`.
//!
//! The only currently specialised prime is `Fp<65537>` (the fifth
//! Fermat prime), which uses the Montgomery-canonical coincidence
//! `R = 2^64 ≡ 1 (mod 65537)` to read and write canonical `u32` values
//! without REDC round-trips. The shared packing and unpacking helpers
//! live here so that `FieldVec` and `BatchExtField` share the same
//! implementation.
//!
//! When the `simd` feature is disabled or AVX2 is unavailable at
//! runtime, every `try_*` hook returns `None` and callers fall back to
//! the scalar element-wise path.

use super::Fp;

// ---------------------------------------------------------------------------
// SimdVecOps trait
// ---------------------------------------------------------------------------

/// Element-wise SIMD-dispatch hook for batched base-field arithmetic.
///
/// `FieldVec::mul_vec` / `add_vec` / `sub_vec` consult the three
/// `try_simd_*` methods before falling back to scalar loops. Every
/// implementing type may return `None` (the default — scalar behaviour)
/// or override a hook to route through a kernel in `gf2-kernels-simd`.
///
/// This trait is sealed in effect: external users should treat it as
/// an implementation detail — the default `None` methods are the only
/// stable contract they see. The blanket impl for `Fp<P>` covers every
/// prime instantiation and is what satisfies the
/// `F: FiniteField + SimdVecOps` bound on `FieldVec`'s element-wise
/// ops.
///
/// # Examples
///
/// ```
/// use gf2_core::field::FieldVec;
/// use gf2_core::gfp::{Fp, SimdVecOps};
///
/// // Trait satisfied by every `Fp<P>`; callers rarely reference the
/// // method directly — `FieldVec::mul_vec` dispatches through it.
/// let xs: Vec<Fp<65537>> = (0..4u64).map(Fp::<65537>::new).collect();
/// let ys: Vec<Fp<65537>> = (0..4u64).map(|i| Fp::<65537>::new(i + 1)).collect();
/// let maybe = <Fp<65537> as SimdVecOps>::try_simd_mul_vec(&xs, &ys);
/// // `maybe` is `Some` on AVX2 hosts with the `simd` feature, `None` elsewhere.
/// let _ = maybe;
/// let _ = FieldVec::from(xs).mul_vec(&FieldVec::from(ys));
/// ```
pub trait SimdVecOps: Sized {
    /// Attempts a SIMD batch multiply; returns `None` to defer to the
    /// scalar element-wise path.
    ///
    /// # Arguments
    ///
    /// * `a`, `b` — same-length element slices.
    ///
    /// # Complexity
    ///
    /// `O(n)` base-field multiplies, with an 8-lane vectorisation
    /// factor on AVX2-capable CPUs for the specialised primes.
    #[inline]
    fn try_simd_mul_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }

    /// Attempts a SIMD batch add; returns `None` to defer to the
    /// scalar element-wise path.
    ///
    /// # Arguments
    ///
    /// * `a`, `b` — same-length element slices.
    ///
    /// # Complexity
    ///
    /// `O(n)`, lane-parallel when specialised.
    #[inline]
    fn try_simd_add_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }

    /// Attempts a SIMD batch subtract; returns `None` to defer to the
    /// scalar element-wise path.
    ///
    /// # Arguments
    ///
    /// * `a`, `b` — same-length element slices.
    ///
    /// # Complexity
    ///
    /// `O(n)`, lane-parallel when specialised.
    #[inline]
    fn try_simd_sub_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }
}

// ---------------------------------------------------------------------------
// Scalar-fallback impls for other base-field types exposed by the crate.
// Each one inherits the default `None` from the trait, so `FieldVec`'s
// element-wise ops use their scalar zip/map loop for these types.
// ---------------------------------------------------------------------------

impl<V: crate::gf2m::UintExt> SimdVecOps for crate::gf2m::Gf2mElement_<V> {}

// ---------------------------------------------------------------------------
// Blanket impl for Fp<P>: compile-time P == 65537 routes into the AVX2 path.
// ---------------------------------------------------------------------------

impl<const P: u64> SimdVecOps for Fp<P> {
    #[inline]
    fn try_simd_mul_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_mul_vec::<P>(a, b);
        }
        None
    }

    #[inline]
    fn try_simd_add_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_add_vec::<P>(a, b);
        }
        None
    }

    #[inline]
    fn try_simd_sub_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_sub_vec::<P>(a, b);
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Fp<65537> SIMD helpers — shared with BatchExtField::batch_mul_quadratic.
// ---------------------------------------------------------------------------
//
// All four helpers below (`fp65537_pack`, `fp65537_unpack`, and the three
// `fp65537_try_*_vec` functions) exist as crate-private single sources of
// truth: `FieldVec` element-wise ops and `BatchExtField::batch_mul_quadratic`
// both reach them through this module.

/// Packs a slice of `Fp<P>` where `P == 65537` into canonical `Vec<u32>`.
///
/// For `P = 65537`, Montgomery storage equals the canonical value because
/// `R = 2^64 ≡ 1 (mod P)`. We therefore use `raw_storage()` directly and
/// avoid the REDC round-trip of `.value()`.
///
/// # Arguments
///
/// * `xs` — slice of `Fp<P>` with `P = 65537`.
///
/// # Complexity
///
/// `O(n)`; one pointer chase and a `u64→u32` truncation per element.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp65537_pack<const P: u64>(xs: &[Fp<P>]) -> Vec<u32> {
    debug_assert_eq!(P, 65537, "fp65537_pack: P must be 65537");
    xs.iter().map(|x| x.raw_storage() as u32).collect()
}

/// Unpacks a slice of canonical `u32` values into `Vec<Fp<P>>` where
/// `P == 65537`. The inverse of [`fp65537_pack`].
///
/// # Arguments
///
/// * `xs` — slice of canonical values, all `< 65537`.
///
/// # Complexity
///
/// `O(n)`.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp65537_unpack<const P: u64>(xs: &[u32]) -> Vec<Fp<P>> {
    debug_assert_eq!(P, 65537, "fp65537_unpack: P must be 65537");
    xs.iter()
        .map(|&x| Fp::<P>::from_raw_storage(x as u64))
        .collect()
}

#[cfg(feature = "simd")]
fn fp65537_try_mul_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    let fns = crate::simd::maybe_fp65537()?;
    let n = a.len();
    let a_u32 = fp65537_pack::<P>(a);
    let b_u32 = fp65537_pack::<P>(b);
    let mut out = vec![0u32; n];
    (fns.batch_mul_fn)(&a_u32, &b_u32, &mut out);
    Some(fp65537_unpack::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp65537_try_add_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    let fns = crate::simd::maybe_fp65537()?;
    let n = a.len();
    let a_u32 = fp65537_pack::<P>(a);
    let b_u32 = fp65537_pack::<P>(b);
    let mut out = vec![0u32; n];
    (fns.batch_add_fn)(&a_u32, &b_u32, &mut out);
    Some(fp65537_unpack::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp65537_try_sub_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    let fns = crate::simd::maybe_fp65537()?;
    let n = a.len();
    let a_u32 = fp65537_pack::<P>(a);
    let b_u32 = fp65537_pack::<P>(b);
    let mut out = vec![0u32; n];
    (fns.batch_sub_fn)(&a_u32, &b_u32, &mut out);
    Some(fp65537_unpack::<P>(&out))
}

// No-SIMD stubs when the `simd` feature is off.

#[cfg(not(feature = "simd"))]
#[inline]
fn fp65537_try_mul_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp65537_try_add_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp65537_try_sub_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}
