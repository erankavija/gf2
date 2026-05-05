//! SIMD element-wise vector-op hooks for `Fp<P>`.
//!
//! This module exposes [`SimdVecOps`], the single source of truth for
//! element-wise SIMD dispatch used by [`crate::field::FieldVec`] and
//! [`crate::gfpn::BatchExtField`]. The trait carries default-`None`
//! implementations that make every `FiniteField` compile unchanged;
//! specific primes override the hooks to route through AVX2 kernels in
//! `gf2-kernels-simd`.
//!
//! Specialised primes are routed before the generic Montgomery path:
//! `Fp<65537>` uses the Fermat-prime AVX2 kernels, `Fp<2^31 - 1>` uses
//! the existing Mersenne multiply kernel, and eligible non-special
//! Montgomery primes use the generic `u64` AVX2 kernels. The shared
//! packing and unpacking helpers live here so that `FieldVec` and
//! `BatchExtField` share the same implementation.
//!
//! When the `simd` feature is disabled or AVX2 is unavailable at
//! runtime, every `try_*` hook returns `None` and callers fall back to
//! the scalar element-wise path.

use super::Fp;
#[cfg(feature = "simd")]
use super::{montgomery::MontConsts, use_specialized_storage};

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
// Blanket impl for Fp<P>: exact specialisations win, then generic Montgomery.
// ---------------------------------------------------------------------------

// IMPORTANT — dispatch ordering invariant (Issue 3d06224c, regression guard).
//
// The `try_simd_*_vec` methods below dispatch most-specific first to preserve
// the Mersenne31 (`P = 2^31 − 1`) and Fermat-prime (`P = 65537`) fast paths
// over the generic Montgomery AVX2 lane. The `if P == M31` and
// `if P == 65537` exact tests MUST remain ABOVE the generic
// `fp_generic_try_*` fallback (which itself is internally guarded by
// `fp_generic_enabled` so it never claims either specialised prime).
//
// New per-prime dispatch branches (e.g. small-prime packed kernel for
// `p ≤ 251`, u16-packed kernel for `p < 65536`) MUST be inserted BELOW the
// existing exact-prime tests but ABOVE the generic fallback, and MUST also
// be excluded by `fp_generic_enabled` so the dispatch lattice stays sound.
// Re-ordering or removing the existing exact tests would silently route
// Mersenne31 / Fermat traffic through the generic Montgomery kernel and
// regress the `WITHIN_1.5X` family verdict in
// `dev/bench_results/2026-05-04-609855d9-gfp-by-family.md`.
//
// The regression test `m31_simd_mul_matches_scalar_across_boundary_lens`
// and the dispatch-classification test
// `specialized_primes_do_not_use_generic_montgomery_path` in this file's
// `tests` module guard this invariant at the unit level; the criterion
// benchmark `mersenne_gemm_regression` (`benches/mersenne_gemm_regression.rs`)
// guards it at the throughput level.
impl<const P: u64> SimdVecOps for Fp<P> {
    #[inline]
    fn try_simd_mul_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_mul_vec::<P>(a, b);
        }
        if P == M31 {
            return fpm31_try_mul_vec::<P>(a, b);
        }
        fp_generic_try_mul_vec::<P>(a, b)
    }

    #[inline]
    fn try_simd_add_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_add_vec::<P>(a, b);
        }
        fp_generic_try_add_vec::<P>(a, b)
    }

    #[inline]
    fn try_simd_sub_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_sub_vec::<P>(a, b);
        }
        fp_generic_try_sub_vec::<P>(a, b)
    }
}

// ---------------------------------------------------------------------------
// Fp<2^31 - 1> SIMD helpers.
// ---------------------------------------------------------------------------

const M31: u64 = (1u64 << 31) - 1;

#[cfg(feature = "simd")]
#[inline]
fn fpm31_pack<const P: u64>(xs: &[Fp<P>]) -> Vec<u32> {
    debug_assert_eq!(P, M31, "fpm31_pack: P must be 2^31 - 1");
    xs.iter().map(|x| x.raw_storage() as u32).collect()
}

#[cfg(feature = "simd")]
#[inline]
fn fpm31_unpack<const P: u64>(xs: &[u32]) -> Vec<Fp<P>> {
    debug_assert_eq!(P, M31, "fpm31_unpack: P must be 2^31 - 1");
    xs.iter()
        .map(|&x| Fp::<P>::from_raw_storage(x as u64))
        .collect()
}

#[cfg(feature = "simd")]
fn fpm31_try_mul_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    debug_assert_eq!(P, M31, "fpm31_try_mul_vec: P must be 2^31 - 1");
    let fns = crate::simd::maybe_mersenne()?;
    let n = a.len();
    let a_u32 = fpm31_pack::<P>(a);
    let b_u32 = fpm31_pack::<P>(b);
    let mut out = vec![0u32; n];
    (fns.m31_batch_mul_fn)(&a_u32, &b_u32, &mut out);
    Some(fpm31_unpack::<P>(&out))
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fpm31_try_mul_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

// ---------------------------------------------------------------------------
// Generic Montgomery SIMD helpers.
// ---------------------------------------------------------------------------

#[cfg(feature = "simd")]
#[inline]
fn fp_generic_enabled<const P: u64>() -> bool {
    P > 2 && P <= (1u64 << 63) && P != 65537 && !use_specialized_storage(P)
}

#[cfg(feature = "simd")]
#[inline]
fn fp_generic_pack<const P: u64>(xs: &[Fp<P>]) -> Vec<u64> {
    xs.iter().map(|x| x.raw_storage()).collect()
}

#[cfg(feature = "simd")]
#[inline]
fn fp_generic_unpack<const P: u64>(xs: &[u64]) -> Vec<Fp<P>> {
    xs.iter().map(|&x| Fp::<P>::from_raw_storage(x)).collect()
}

#[cfg(feature = "simd")]
fn fp_generic_try_mul_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_generic_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_generic()?;
    let n = a.len();
    let a_u64 = fp_generic_pack::<P>(a);
    let b_u64 = fp_generic_pack::<P>(b);
    let mut out = vec![0u64; n];
    (fns.batch_mul_fn)(&a_u64, &b_u64, P, MontConsts::<P>::P_INV, &mut out);
    Some(fp_generic_unpack::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp_generic_try_add_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_generic_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_generic()?;
    let n = a.len();
    let a_u64 = fp_generic_pack::<P>(a);
    let b_u64 = fp_generic_pack::<P>(b);
    let mut out = vec![0u64; n];
    (fns.batch_add_fn)(&a_u64, &b_u64, P, &mut out);
    Some(fp_generic_unpack::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp_generic_try_sub_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_generic_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_generic()?;
    let n = a.len();
    let a_u64 = fp_generic_pack::<P>(a);
    let b_u64 = fp_generic_pack::<P>(b);
    let mut out = vec![0u64; n];
    (fns.batch_sub_fn)(&a_u64, &b_u64, P, &mut out);
    Some(fp_generic_unpack::<P>(&out))
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_generic_try_mul_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_generic_try_add_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_generic_try_sub_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    const WORD_BOUNDARY_LENS: &[usize] = &[0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257];

    fn check_generic_prime<const P: u64>() {
        #[cfg(not(feature = "simd"))]
        {
            return;
        }
        #[cfg(feature = "simd")]
        {
            if crate::simd::maybe_fp_generic().is_none() {
                return;
            }

            for &len in WORD_BOUNDARY_LENS {
                let a: Vec<Fp<P>> = (0..len as u64)
                    .map(|i| Fp::<P>::new(i.wrapping_mul(1_000_003).wrapping_add(17)))
                    .collect();
                let b: Vec<Fp<P>> = (0..len as u64)
                    .map(|i| Fp::<P>::new(i.wrapping_mul(2_000_033).wrapping_add(23)))
                    .collect();

                let got_add =
                    <Fp<P> as SimdVecOps>::try_simd_add_vec(&a, &b).expect("generic SIMD add");
                let got_sub =
                    <Fp<P> as SimdVecOps>::try_simd_sub_vec(&a, &b).expect("generic SIMD sub");
                let got_mul =
                    <Fp<P> as SimdVecOps>::try_simd_mul_vec(&a, &b).expect("generic SIMD mul");

                for i in 0..len {
                    assert_eq!(got_add[i], a[i] + b[i], "add P={P}, len={len}, i={i}");
                    assert_eq!(got_sub[i], a[i] - b[i], "sub P={P}, len={len}, i={i}");
                    assert_eq!(got_mul[i], a[i] * b[i], "mul P={P}, len={len}, i={i}");
                }
            }
        }
    }

    #[test]
    fn generic_simd_matches_scalar_for_proof_suite_primes() {
        check_generic_prime::<3>();
        check_generic_prime::<5>();
        check_generic_prime::<7>();
        check_generic_prime::<11>();
        check_generic_prime::<13>();
        check_generic_prime::<17>();
        check_generic_prime::<2_147_483_629>();
        check_generic_prime::<2_305_843_009_213_693_907>();
        check_generic_prime::<9_223_372_036_854_775_783>();
    }

    #[test]
    #[cfg(feature = "simd")]
    fn generic_montgomery_guard_excludes_unsupported_moduli() {
        // `Fp<P>` itself rejects `P > 2^63`, but keep the SIMD guard equally
        // strict so this private dispatch layer can never reach the AVX2
        // Montgomery kernel's `modulus <= 2^63` assertion for out-of-range
        // const parameters.
        assert!(!fp_generic_enabled::<2>());
        assert!(!fp_generic_enabled::<{ (1u64 << 63) + 25 }>());
    }

    /// Regression guard for issue `3d06224c` (story `cc5de315`, "Protect
    /// Mersenne fast path"). Asserts that the `if P == M31` dispatch
    /// branch in `<Fp<P> as SimdVecOps>::try_simd_mul_vec` is reachable on
    /// AVX2 hosts and that, on every word-boundary-relevant length, the
    /// SIMD-batched Mersenne31 multiply matches the scalar element-wise
    /// product bit-exactly. Sibling issues `662f7a15` and `9e12659b`
    /// concurrently extend the dispatch ladder; this test fails if either
    /// of them accidentally re-orders the ladder so Mersenne31 traffic is
    /// routed through the generic Montgomery AVX2 kernel (which would
    /// regress the `WITHIN_1.5X` family verdict per
    /// `dev/bench_results/2026-05-04-609855d9-gfp-by-family.md`).
    #[test]
    #[cfg(feature = "simd")]
    fn m31_simd_mul_matches_scalar_across_boundary_lens() {
        if crate::simd::maybe_mersenne().is_none() {
            // Non-AVX2 host; the fast path is genuinely unreachable here
            // and the dispatch ordering is therefore moot at runtime.
            return;
        }

        const P: u64 = M31;

        for &len in WORD_BOUNDARY_LENS {
            let a: Vec<Fp<P>> = (0..len as u64)
                .map(|i| Fp::<P>::new(i.wrapping_mul(2_654_435_761).wrapping_add(11)))
                .collect();
            let b: Vec<Fp<P>> = (0..len as u64)
                .map(|i| Fp::<P>::new(i.wrapping_mul(40_503).wrapping_add(7)))
                .collect();

            // Dispatch must reach the M31-specialised SIMD path, never
            // the generic Montgomery AVX2 lane.
            let got = <Fp<P> as SimdVecOps>::try_simd_mul_vec(&a, &b)
                .expect("M31 dispatch must yield Some on AVX2 host");
            assert_eq!(
                got.len(),
                len,
                "len mismatch on M31 SIMD multiply, len={len}",
            );

            for i in 0..len {
                let expected = a[i] * b[i];
                assert_eq!(
                    got[i], expected,
                    "M31 SIMD multiply diverges from scalar at len={len}, i={i}",
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn specialized_primes_do_not_use_generic_montgomery_path() {
        assert!(!fp_generic_enabled::<65537>());
        assert!(!fp_generic_enabled::<{ (1u64 << 31) - 1 }>());
        assert!(!fp_generic_enabled::<{ (1u64 << 61) - 1 }>());

        let a65537 = [Fp::<65537>::new(3), Fp::<65537>::new(5)];
        let b65537 = [Fp::<65537>::new(7), Fp::<65537>::new(11)];
        if crate::simd::maybe_fp65537().is_some() {
            assert!(<Fp<65537> as SimdVecOps>::try_simd_mul_vec(&a65537, &b65537).is_some());
        }

        let m31_a = [Fp::<{ (1u64 << 31) - 1 }>::new(3)];
        let m31_b = [Fp::<{ (1u64 << 31) - 1 }>::new(7)];
        if crate::simd::maybe_mersenne().is_some() {
            let got = <Fp<{ (1u64 << 31) - 1 }> as SimdVecOps>::try_simd_mul_vec(&m31_a, &m31_b)
                .expect("M31 SIMD multiply");
            assert_eq!(got, vec![m31_a[0] * m31_b[0]]);
        }

        let m61_a = [Fp::<{ (1u64 << 61) - 1 }>::new(3)];
        let m61_b = [Fp::<{ (1u64 << 61) - 1 }>::new(7)];
        assert!(
            <Fp<{ (1u64 << 61) - 1 }> as SimdVecOps>::try_simd_mul_vec(&m61_a, &m61_b).is_none()
        );
    }
}
