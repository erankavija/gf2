//! SIMD dispatch hooks for `Fp<P>`.
//!
//! [`SimdVecOps`] is the element-wise SIMD dispatch used by
//! [`crate::field::FieldVec`] and [`crate::gfpn::BatchExtField`]; `Fp<P>`
//! overrides its default-`None` hooks to route through AVX2 kernels in
//! `gf2-kernels-simd`. When the `simd` feature is disabled or AVX2 is
//! unavailable at runtime, every `try_*` hook declines and callers fall back
//! to their scalar path.

use super::Fp;
#[cfg(feature = "simd")]
use super::{montgomery::MontConsts, use_specialized_storage};
#[cfg(feature = "simd")]
use crate::field::FiniteField;
use crate::field::PlePanelLane;

// ---------------------------------------------------------------------------
// SimdVecOps trait
// ---------------------------------------------------------------------------

/// Element-wise SIMD-dispatch hook for batched base-field arithmetic.
///
/// `FieldVec::mul_vec` / `add_vec` / `sub_vec` consult the `try_simd_*`
/// methods before their scalar loops; the default `None` selects the scalar
/// loop.
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
    /// Attempts a SIMD batch multiply of same-length slices; returns `None`
    /// to defer to the scalar element-wise path.
    #[inline]
    fn try_simd_mul_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }

    /// Attempts a SIMD batch add of same-length slices; returns `None` to
    /// defer to the scalar element-wise path.
    #[inline]
    fn try_simd_add_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }

    /// Attempts a SIMD batch subtract of same-length slices; returns `None`
    /// to defer to the scalar element-wise path.
    #[inline]
    fn try_simd_sub_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }

    /// Attempts a SIMD dot product `∑ a[i] · b[i]` of same-length slices,
    /// equal to the scalar `dot_product_slices` result; returns `None` to
    /// defer to the chunked `mul_product_sum_wide` loop in
    /// `crate::field::vec::dot_product_slices`.
    #[inline]
    fn try_simd_dot_vec(_a: &[Self], _b: &[Self]) -> Option<Self> {
        None
    }
}

// ---------------------------------------------------------------------------
// Scalar-fallback impls for other base-field types exposed by the crate.
// Each one inherits the default `None` from the trait, so `FieldVec`'s
// element-wise ops use their scalar zip/map loop for these types.
// ---------------------------------------------------------------------------

impl<V: crate::gf2m::UintExt> SimdVecOps for crate::gf2m::Gf2mElement_<V> {}

// `Gf2mWide` participates in `FieldMatrix::gemm` paths but has no
// dedicated SIMD batch hooks (its multiply is already vectorised via
// `gf2_kernels_simd::gf2m_wide`).
impl<const N: usize, Cfg: crate::gf2m::Gf2mWideConfig<N>> SimdVecOps
    for crate::gf2m::Gf2mWide<N, Cfg>
{
}

// Goldilocks uses the dedicated `GoldilocksFp` scalar reducer; no
// byte/word-lane SIMD batch path applies because the storage is a full
// `u64` Goldilocks residue.
impl SimdVecOps for crate::gfp::specialized::GoldilocksFp {}

// Tower extensions over `Fp<P>` route through `BatchExtField`'s SoA
// kernels rather than the element-wise `SimdVecOps` hooks; the trait is
// still implemented (with the default `None` returns) so the
// `dot_product_slices` bound is satisfied for `FieldMatrix::<QuadraticExt<C>>`
// and `FieldMatrix::<CubicExt<C>>`.
impl<C: crate::gfpn::ExtConfig> SimdVecOps for crate::gfpn::QuadraticExt<C> {}
impl<C: crate::gfpn::ExtConfig> SimdVecOps for crate::gfpn::CubicExt<C> {}

// ---------------------------------------------------------------------------
// Blanket impl for Fp<P>: exact specialisations win, then generic Montgomery.
// ---------------------------------------------------------------------------

// Dispatch-order invariant: the exact-prime tests (`P == 65537`, `P == M31`)
// precede the range tests, which precede the generic Montgomery fallback,
// and `fp_generic_enabled` excludes every prime an earlier branch owns. The
// tests `m31_simd_mul_matches_scalar_across_boundary_lens` and
// `specialized_primes_do_not_use_generic_montgomery_path` guard it.
impl<const P: u64> SimdVecOps for Fp<P> {
    #[inline]
    fn try_simd_mul_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_mul_vec::<P>(a, b);
        }
        if P == M31 {
            return fpm31_try_mul_vec::<P>(a, b);
        }
        if P <= 251 {
            return fp_small_try_mul_vec::<P>(a, b);
        }
        if P >= 252 && P < 65536 {
            return fp_medium_try_mul_vec::<P>(a, b);
        }
        fp_generic_try_mul_vec::<P>(a, b)
    }

    #[inline]
    fn try_simd_add_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_add_vec::<P>(a, b);
        }
        if P <= 251 {
            return fp_small_try_add_vec::<P>(a, b);
        }
        if P >= 252 && P < 65536 {
            return fp_medium_try_add_vec::<P>(a, b);
        }
        fp_generic_try_add_vec::<P>(a, b)
    }

    #[inline]
    fn try_simd_sub_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        if P == 65537 {
            return fp65537_try_sub_vec::<P>(a, b);
        }
        if P <= 251 {
            return fp_small_try_sub_vec::<P>(a, b);
        }
        if P >= 252 && P < 65536 {
            return fp_medium_try_sub_vec::<P>(a, b);
        }
        fp_generic_try_sub_vec::<P>(a, b)
    }

    #[inline]
    fn try_simd_dot_vec(a: &[Self], b: &[Self]) -> Option<Self> {
        if P <= 251 && P >= 3 {
            return fp_small_try_dot_vec::<P>(a, b);
        }
        None
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

/// Whole-GEMM fast path for `Fp<2^31 - 1>` (Mersenne31) using the
/// AVX2 `m31_batch_dot_fn` kernel.
///
/// Packs `a` (`m × k` row-major) and `b_t` (`n × k` row-major, already
/// transposed by the caller) once into `u32` buffers; `Fp<M31>` storage is
/// canonical, so the pack is a truncation. Runs one batch dot per cell of
/// `out` (`m × n` row-major).
///
/// Returns `true` when the kernel populated `out`; `false` when the
/// field is not M31, a dimension is zero, the `simd` feature is disabled,
/// or AVX2 is unavailable at runtime.
#[cfg(feature = "simd")]
pub(crate) fn fp_m31_try_gemm_classical<const P: u64>(
    a: &[Fp<P>],
    b_t: &[Fp<P>],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [Fp<P>],
) -> bool {
    if P != M31 {
        return false;
    }
    if m == 0 || k == 0 || n == 0 {
        return false;
    }
    debug_assert_eq!(a.len(), m * k, "fp_m31_try_gemm_classical: a shape");
    debug_assert_eq!(b_t.len(), n * k, "fp_m31_try_gemm_classical: b_t shape");
    debug_assert_eq!(out.len(), m * n, "fp_m31_try_gemm_classical: out shape");

    let Some(fns) = crate::simd::maybe_mersenne() else {
        return false;
    };

    GEMM_M31_A_SCRATCH.with_borrow_mut(|a_u32| {
        GEMM_M31_BT_SCRATCH.with_borrow_mut(|bt_u32| {
            a_u32.resize(m * k, 0u32);
            bt_u32.resize(n * k, 0u32);
            for (dst, src) in a_u32.iter_mut().zip(a.iter()) {
                *dst = src.raw_storage() as u32;
            }
            for (dst, src) in bt_u32.iter_mut().zip(b_t.iter()) {
                *dst = src.raw_storage() as u32;
            }

            for i in 0..m {
                let a_row = &a_u32[i * k..(i + 1) * k];
                for j in 0..n {
                    let bt_row = &bt_u32[j * k..(j + 1) * k];
                    let dot = (fns.m31_batch_dot_fn)(a_row, bt_row);
                    out[i * n + j] = Fp::<P>::from_raw_storage(dot as u64);
                }
            }
        })
    });
    true
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_m31_try_gemm_classical<const P: u64>(
    _a: &[Fp<P>],
    _b_t: &[Fp<P>],
    _m: usize,
    _k: usize,
    _n: usize,
    _out: &mut [Fp<P>],
) -> bool {
    false
}

/// Non-allocating availability probe for `fp_m31_try_gemm_classical`.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_m31_gemm_classical_available<const P: u64>() -> bool {
    P == M31 && crate::simd::maybe_mersenne().is_some()
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_m31_gemm_classical_available<const P: u64>() -> bool {
    false
}

// ---------------------------------------------------------------------------
// Small-prime (P <= 251) SIMD helpers.
//
// The kernels take canonical bytes in `[0, P)`; `P <= 251` is
// Montgomery-stored, so pack and unpack convert through `value()` /
// `Fp::new`.
// ---------------------------------------------------------------------------

/// Whether the small-prime byte-lane dispatch handles `P`. `P = 2` is
/// excluded because the byte-lane Barrett constant assumes `p ≥ 3`.
#[cfg(feature = "simd")]
#[inline]
fn fp_small_enabled<const P: u64>() -> bool {
    P >= 3 && P <= 251
}

#[cfg(feature = "simd")]
#[inline]
fn fp_small_pack<const P: u64>(xs: &[Fp<P>]) -> Vec<u8> {
    debug_assert!(fp_small_enabled::<P>());
    xs.iter().map(|x| x.value() as u8).collect()
}

#[cfg(feature = "simd")]
#[inline]
fn fp_small_unpack<const P: u64>(xs: &[u8]) -> Vec<Fp<P>> {
    debug_assert!(fp_small_enabled::<P>());
    xs.iter().map(|&x| Fp::<P>::new(x as u64)).collect()
}

#[cfg(feature = "simd")]
fn fp_small_try_mul_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_small_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_small()?;
    let n = a.len();
    let a_u8 = fp_small_pack::<P>(a);
    let b_u8 = fp_small_pack::<P>(b);
    let mut out = vec![0u8; n];
    (fns.batch_mul_fn)(&a_u8, &b_u8, P as u8, &mut out);
    Some(fp_small_unpack::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp_small_try_add_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_small_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_small()?;
    let n = a.len();
    let a_u8 = fp_small_pack::<P>(a);
    let b_u8 = fp_small_pack::<P>(b);
    let mut out = vec![0u8; n];
    (fns.batch_add_fn)(&a_u8, &b_u8, P as u8, &mut out);
    Some(fp_small_unpack::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp_small_try_sub_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_small_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_small()?;
    let n = a.len();
    let a_u8 = fp_small_pack::<P>(a);
    let b_u8 = fp_small_pack::<P>(b);
    let mut out = vec![0u8; n];
    (fns.batch_sub_fn)(&a_u8, &b_u8, P as u8, &mut out);
    Some(fp_small_unpack::<P>(&out))
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_small_try_mul_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_small_try_add_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_small_try_sub_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(feature = "simd")]
fn fp_small_try_dot_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Fp<P>> {
    if !fp_small_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_small()?;
    if a.is_empty() {
        return Some(Fp::<P>::new(0));
    }
    let a_u8 = fp_small_pack::<P>(a);
    let b_u8 = fp_small_pack::<P>(b);
    let canonical = (fns.batch_dot_fn)(&a_u8, &b_u8, P as u8);
    Some(Fp::<P>::new(canonical as u64))
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_small_try_dot_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Fp<P>> {
    None
}

/// Conservative default for the tuning profile's `prime_route.f32_min_prime`
/// field: the minimum prime for which Candidate F (f32-FMA cascade) is
/// preferred over Candidate C (AVX2 16-bit Barrett). With the default
/// column threshold [`F32_MIN_COLS`], it admits only the cell
/// `P == 251 && n >= 512`.
pub(crate) const N_THRESH_PRIME: u64 = 251;

/// Conservative default for the tuning profile's `prime_route.f32_min_cols`
/// field: the minimum output width, in columns, at which `select_f32_path`
/// prefers the f32-FMA cascade.
pub(crate) const F32_MIN_COLS: usize = 512;

/// The `prime_route.f32_min_prime` value [`select_f32_path`] compares `P`
/// against.
///
/// The field is baked rather than resolved through `crate::tuning::active()`
/// because the predicate is a `const fn` over a const-generic prime, so the
/// whole comparison folds at monomorphisation. The default build resolves it
/// to [`N_THRESH_PRIME`]; `RUSTFLAGS="--cfg gf2_tuning_baked"` resolves it to
/// `crate::tuning::baked::N_THRESH_PRIME`. Installing a runtime profile does
/// not move this boundary.
#[cfg(all(feature = "simd", gf2_tuning_baked))]
const F32_MIN_PRIME_SELECTED: u64 = crate::tuning::baked::N_THRESH_PRIME;

/// The `prime_route.f32_min_prime` value [`select_f32_path`] compares `P`
/// against; see the baked arm for the mechanism.
#[cfg(all(feature = "simd", not(gf2_tuning_baked)))]
const F32_MIN_PRIME_SELECTED: u64 = N_THRESH_PRIME;

/// The `prime_route.f32_min_cols` value [`select_f32_path`] compares `n`
/// against; the mechanism is [`F32_MIN_PRIME_SELECTED`]'s.
#[cfg(all(feature = "simd", gf2_tuning_baked))]
const F32_MIN_COLS_SELECTED: usize = crate::tuning::baked::F32_MIN_COLS;

/// The `prime_route.f32_min_cols` value [`select_f32_path`] compares `n`
/// against; the mechanism is [`F32_MIN_PRIME_SELECTED`]'s.
#[cfg(all(feature = "simd", not(gf2_tuning_baked)))]
const F32_MIN_COLS_SELECTED: usize = F32_MIN_COLS;

/// Per-(P, m, k, n) Candidate-F / route-A selector.
///
/// Returns `true` when the f32 cascade is the default arm for this
/// (prime, size) cell. The two bounds come from the tuning profile's
/// `prime_route.f32_min_prime` and `prime_route.f32_min_cols` fields through
/// [`F32_MIN_PRIME_SELECTED`] and [`F32_MIN_COLS_SELECTED`]; at their
/// conservative defaults (251 and 512) only the cell `P == 251 && n >= 512`
/// qualifies.
#[cfg(feature = "simd")]
#[inline]
const fn select_f32_path<const P: u64>(_m: usize, _k: usize, n: usize) -> bool {
    P >= F32_MIN_PRIME_SELECTED && P <= 251 && n >= F32_MIN_COLS_SELECTED
}

// ---------------------------------------------------------------------------
// Route-A dispatch toggle
// ---------------------------------------------------------------------------

#[cfg(all(feature = "simd", any(test, feature = "test-support")))]
use std::sync::atomic::AtomicUsize;
#[cfg(feature = "simd")]
use std::sync::atomic::{AtomicBool, Ordering};

/// Process-wide debug switch forcing GF(251) GEMM onto the route-A f32/FMA
/// cascade; set through [`set_route_a_gf251_enabled`].
#[cfg(feature = "simd")]
static ROUTE_A_GF251_ENABLED: AtomicBool = AtomicBool::new(false);

/// Sets the runtime debug switch that forces every GF(251) GEMM call onto
/// route A (the f32-FMA cascade with lookup-table pack / unpack and
/// vectorized AVX2 Barrett output reduction).
///
/// With the switch `false` (default), GF(251) still takes route A in the
/// cells `select_f32_path` admits (`n ≥ 512` at the default thresholds).
/// The flag is a process-wide `AtomicBool`: restore `false` after use to
/// avoid cross-test interference. Primes other than 251 ignore it.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "simd")]
/// # {
/// use gf2_core::gfp::simd_ops::set_route_a_gf251_enabled;
/// set_route_a_gf251_enabled(true);
/// // ... run GF(251) GEMM via route A ...
/// set_route_a_gf251_enabled(false);
/// # }
/// ```
#[cfg(feature = "simd")]
pub fn set_route_a_gf251_enabled(enabled: bool) {
    ROUTE_A_GF251_ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(feature = "simd")]
#[inline]
fn route_a_gf251_enabled<const P: u64>() -> bool {
    if P != 251 {
        return false;
    }
    ROUTE_A_GF251_ENABLED.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Route-C dispatch toggle
// ---------------------------------------------------------------------------

/// Process-wide debug switch for the route-C GF(251) pure-integer
/// Goto/BLIS-style panelized micro-kernel; set through
/// [`set_route_c_gf251_enabled`]. If both switches are on for `P == 251`,
/// route A wins (the dispatch checks route A first).
#[cfg(feature = "simd")]
static ROUTE_C_GF251_ENABLED: AtomicBool = AtomicBool::new(false);

/// Sets the runtime debug switch that opts GF(251) GEMM calls into the
/// route-C pure-integer Goto/BLIS-style panelized micro-kernel
/// (`crate::simd::maybe_fp_small_panel`).
///
/// The switch applies to the `P == 251` cells that route A's guard in
/// `prime_gemm_select` leaves. The flag is a process-wide `AtomicBool`:
/// restore `false` after use to avoid cross-test interference.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "simd")]
/// # {
/// use gf2_core::gfp::simd_ops::set_route_c_gf251_enabled;
/// set_route_c_gf251_enabled(true);
/// // ... run GF(251) GEMM via route C ...
/// set_route_c_gf251_enabled(false);
/// # }
/// ```
#[cfg(feature = "simd")]
pub fn set_route_c_gf251_enabled(enabled: bool) {
    ROUTE_C_GF251_ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(feature = "simd")]
#[inline]
fn route_c_gf251_enabled<const P: u64>() -> bool {
    if P != 251 {
        return false;
    }
    ROUTE_C_GF251_ENABLED.load(Ordering::Relaxed)
}

/// Whole-gemm fast path for `P ≤ 251`. Packs `a` (`m × k` row-major) and
/// `b_t` (`n × k` row-major, already transposed by the caller), runs the arm
/// [`prime_gemm_select`] names, and unpacks into `out` (`m × n` row-major).
///
/// Every arm except `F32CascadeDirect` packs and unpacks through the
/// per-prime `from_mont` / `to_mont` tables of `build_small_prime_tables`
/// into thread-local scratch; `F32CascadeDirect` converts each element and
/// allocates per call.
///
/// Returns `true` when one of the fast paths executed; `false` to
/// defer to the caller's scalar `dot_product_slices` loop.
#[cfg(feature = "simd")]
pub(crate) fn fp_small_try_gemm_classical<const P: u64>(
    a: &[Fp<P>],
    b_t: &[Fp<P>],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [Fp<P>],
) -> bool {
    debug_assert_eq!(a.len(), m * k, "fp_small_try_gemm_classical: a shape");
    debug_assert_eq!(b_t.len(), n * k, "fp_small_try_gemm_classical: b_t shape");
    debug_assert_eq!(out.len(), m * n, "fp_small_try_gemm_classical: out shape");

    let p_u8 = P as u8;

    // Arm selection belongs to `prime_gemm_select` alone. Each arm here only
    // runs the kernel it names, so the reporter and this dispatcher cannot
    // disagree.
    match prime_gemm_select::<P>(m, k, n) {
        PrimeGemmRoute::F32CascadeTabled => {
            #[cfg(any(test, feature = "test-support"))]
            record_executed_prime_gemm_route(PrimeGemmRoute::F32CascadeTabled);
            let Some(fns_f32) = crate::simd::maybe_fp_small_f32() else {
                return false;
            };
            let tables = build_small_prime_tables::<P>();
            let from_mont_f32 = tables.from_mont_f32.as_slice();
            let to_mont = tables.to_mont.as_slice();
            GEMM_SMALL_F32_A_SCRATCH.with_borrow_mut(|a_f32_scratch| {
                GEMM_SMALL_F32_BT_SCRATCH.with_borrow_mut(|bt_f32_scratch| {
                    GEMM_SMALL_OUT_SCRATCH.with_borrow_mut(|out_u8_scratch| {
                        a_f32_scratch.resize(m * k, 0.0);
                        bt_f32_scratch.resize(n * k, 0.0);
                        out_u8_scratch.resize(m * n, 0u8);

                        for (dst, src) in a_f32_scratch.iter_mut().zip(a.iter()) {
                            let raw = src.raw_storage() as usize;
                            debug_assert!(raw < from_mont_f32.len());
                            *dst = from_mont_f32[raw];
                        }
                        for (dst, src) in bt_f32_scratch.iter_mut().zip(b_t.iter()) {
                            let raw = src.raw_storage() as usize;
                            debug_assert!(raw < from_mont_f32.len());
                            *dst = from_mont_f32[raw];
                        }

                        (fns_f32.batch_gemm_route_a_fn)(
                            a_f32_scratch,
                            bt_f32_scratch,
                            m,
                            k,
                            n,
                            p_u8,
                            out_u8_scratch,
                        );

                        for (slot, &byte) in out.iter_mut().zip(out_u8_scratch.iter()) {
                            let canon = byte as usize;
                            debug_assert!(canon < to_mont.len());
                            *slot = Fp::<P>::from_raw_storage(to_mont[canon]);
                        }
                        true
                    })
                })
            })
        }

        PrimeGemmRoute::U8Panel => {
            #[cfg(any(test, feature = "test-support"))]
            record_executed_prime_gemm_route(PrimeGemmRoute::U8Panel);
            let Some(fns_panel) = crate::simd::maybe_fp_small_panel() else {
                return false;
            };
            let tables = build_small_prime_tables::<P>();
            let from_mont = tables.from_mont.as_slice();
            let to_mont = tables.to_mont.as_slice();
            GEMM_SMALL_A_SCRATCH.with_borrow_mut(|a_u8| {
                GEMM_SMALL_BT_SCRATCH.with_borrow_mut(|bt_u8| {
                    GEMM_SMALL_OUT_SCRATCH.with_borrow_mut(|out_u8| {
                        a_u8.resize(m * k, 0u8);
                        bt_u8.resize(n * k, 0u8);
                        out_u8.resize(m * n, 0u8);

                        for (dst, src) in a_u8.iter_mut().zip(a.iter()) {
                            let raw = src.raw_storage() as usize;
                            debug_assert!(raw < from_mont.len());
                            *dst = from_mont[raw];
                        }
                        for (dst, src) in bt_u8.iter_mut().zip(b_t.iter()) {
                            let raw = src.raw_storage() as usize;
                            debug_assert!(raw < from_mont.len());
                            *dst = from_mont[raw];
                        }

                        (fns_panel.batch_gemm_fn)(a_u8, bt_u8, m, k, n, p_u8, out_u8);

                        for (slot, &byte) in out.iter_mut().zip(out_u8.iter()) {
                            let canon = byte as usize;
                            debug_assert!(canon < to_mont.len());
                            *slot = Fp::<P>::from_raw_storage(to_mont[canon]);
                        }
                        true
                    })
                })
            })
        }

        PrimeGemmRoute::F32CascadeDirect => {
            #[cfg(any(test, feature = "test-support"))]
            record_executed_prime_gemm_route(PrimeGemmRoute::F32CascadeDirect);
            let Some(fns_f32) = crate::simd::maybe_fp_small_f32() else {
                return false;
            };
            let mut out_u8 = vec![0u8; m * n];
            let a_f32: Vec<f32> = a.iter().map(|x| x.value() as f32).collect();
            let bt_f32: Vec<f32> = b_t.iter().map(|x| x.value() as f32).collect();
            (fns_f32.batch_gemm_fn)(&a_f32, &bt_f32, m, k, n, p_u8, &mut out_u8);
            for (slot, &byte) in out.iter_mut().zip(out_u8.iter()) {
                *slot = Fp::<P>::new(byte as u64);
            }
            true
        }

        PrimeGemmRoute::ByteLaneBaseline => {
            #[cfg(any(test, feature = "test-support"))]
            record_executed_prime_gemm_route(PrimeGemmRoute::ByteLaneBaseline);
            let Some(fns) = crate::simd::maybe_fp_small() else {
                return false;
            };
            let tables = build_small_prime_tables::<P>();

            GEMM_SMALL_A_SCRATCH.with_borrow_mut(|a_u8| {
                GEMM_SMALL_BT_SCRATCH.with_borrow_mut(|bt_u8| {
                    GEMM_SMALL_OUT_SCRATCH.with_borrow_mut(|out_u8| {
                        let a_len = m * k;
                        let bt_len = n * k;
                        let out_len = m * n;
                        a_u8.resize(a_len, 0u8);
                        bt_u8.resize(bt_len, 0u8);
                        out_u8.resize(out_len, 0u8);

                        let from_mont = tables.from_mont.as_slice();
                        for (dst, src) in a_u8.iter_mut().zip(a.iter()) {
                            let raw = src.raw_storage() as usize;
                            debug_assert!(raw < from_mont.len());
                            *dst = from_mont[raw];
                        }
                        for (dst, src) in bt_u8.iter_mut().zip(b_t.iter()) {
                            let raw = src.raw_storage() as usize;
                            debug_assert!(raw < from_mont.len());
                            *dst = from_mont[raw];
                        }

                        for i in 0..m {
                            let a_row = &a_u8[i * k..(i + 1) * k];
                            let out_row = &mut out_u8[i * n..(i + 1) * n];
                            (fns.gemm_row_panel_fn)(a_row, bt_u8, k, n, p_u8, out_row);
                        }

                        let to_mont = tables.to_mont.as_slice();
                        for (slot, &byte) in out.iter_mut().zip(out_u8.iter()) {
                            let canon = byte as usize;
                            debug_assert!(canon < to_mont.len());
                            *slot = Fp::<P>::from_raw_storage(to_mont[canon]);
                        }
                    });
                });
            });
            true
        }

        PrimeGemmRoute::F64Cascade | PrimeGemmRoute::U16Panel | PrimeGemmRoute::Deferred => false,
    }
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_small_try_gemm_classical<const P: u64>(
    _a: &[Fp<P>],
    _b_t: &[Fp<P>],
    _m: usize,
    _k: usize,
    _n: usize,
    _out: &mut [Fp<P>],
) -> bool {
    false
}

/// Sparse-times-dense whole-matmat dispatcher for `Fp<P>`.
///
/// Packs `b` once into a canonical-byte (`P ≤ 251`) or canonical-u16
/// (`P ∈ (251, 65535]`) buffer and sweeps every row of the sparse left
/// matrix through the AVX2 SpMM kernel. Returns `false` when `P` is outside
/// the supported range, when the `simd` feature is disabled, or when AVX2
/// is unavailable; the caller then falls back to the generic
/// Wide-accumulator scatter path in `SparseFieldMatrix::matmat`.
#[cfg(feature = "simd")]
pub(crate) fn fp_try_spmm<const P: u64>(
    a_row_ptr: &[usize],
    a_col_idx: &[usize],
    a_values: &[Fp<P>],
    b: &[Fp<P>],
    b_rows: usize,
    n: usize,
    out: &mut [Fp<P>],
) -> bool {
    let m = a_row_ptr.len().saturating_sub(1);
    debug_assert_eq!(a_col_idx.len(), a_values.len());
    debug_assert_eq!(b.len(), b_rows * n);
    debug_assert_eq!(out.len(), m * n);

    if m == 0 || n == 0 {
        return true;
    }

    if fp_small_enabled::<P>() {
        let Some(fns) = crate::simd::maybe_fp_small() else {
            return false;
        };
        let b_u8: Vec<u8> = b.iter().map(|x| x.value() as u8).collect();
        let a_vals_u8: Vec<u8> = a_values.iter().map(|x| x.value() as u8).collect();
        let mut out_u8 = vec![0u8; n];
        for r in 0..m {
            let start = a_row_ptr[r];
            let end = a_row_ptr[r + 1];
            for slot in out_u8.iter_mut() {
                *slot = 0;
            }
            if start != end {
                (fns.spmm_row_fn)(
                    &a_vals_u8[start..end],
                    &a_col_idx[start..end],
                    &b_u8,
                    n,
                    n,
                    P as u8,
                    &mut out_u8,
                );
            }
            let out_row = &mut out[r * n..(r + 1) * n];
            for (slot, &byte) in out_row.iter_mut().zip(out_u8.iter()) {
                *slot = Fp::<P>::new(byte as u64);
            }
        }
        return true;
    }

    if fp_medium_eligible::<P>() {
        let Some(fns) = crate::simd::maybe_fp_medium() else {
            return false;
        };
        let b_u16: Vec<u16> = b.iter().map(|x| x.value() as u16).collect();
        let a_vals_u16: Vec<u16> = a_values.iter().map(|x| x.value() as u16).collect();
        let mut out_u16 = vec![0u16; n];
        for r in 0..m {
            let start = a_row_ptr[r];
            let end = a_row_ptr[r + 1];
            for slot in out_u16.iter_mut() {
                *slot = 0;
            }
            if start != end {
                (fns.spmm_row_fn)(
                    &a_vals_u16[start..end],
                    &a_col_idx[start..end],
                    &b_u16,
                    n,
                    n,
                    P as u16,
                    &mut out_u16,
                );
            }
            let out_row = &mut out[r * n..(r + 1) * n];
            for (slot, &word) in out_row.iter_mut().zip(out_u16.iter()) {
                *slot = Fp::<P>::new(word as u64);
            }
        }
        return true;
    }

    false
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_try_spmm<const P: u64>(
    _a_row_ptr: &[usize],
    _a_col_idx: &[usize],
    _a_values: &[Fp<P>],
    _b: &[Fp<P>],
    _b_rows: usize,
    _n: usize,
    _out: &mut [Fp<P>],
) -> bool {
    false
}

// ---------------------------------------------------------------------------
// Generic Montgomery SIMD helpers.
// ---------------------------------------------------------------------------

#[cfg(feature = "simd")]
#[inline]
fn fp_generic_enabled<const P: u64>() -> bool {
    // Generic Montgomery covers all eligible primes EXCEPT the ones owned
    // by specialised kernels:
    //   * `P = 65537` → Fp65537 Fermat-prime kernel (`fp65537_*_vec`).
    //   * `P <= 251` → small-prime byte-lane Barrett kernel
    //     (`fp_small_*_vec`).
    //   * `P ∈ (251, 65536)` → medium-prime u16 Barrett kernel
    //     (`fp_medium_*_vec`).
    //   * specialised-storage primes (Mersenne `n ≥ 31`, Proth `n ≥ 24`)
    //     keep canonical storage and bypass the Montgomery layer entirely.
    P > 2
        && P <= (1u64 << 63)
        && P != 65537
        && P > 251
        && !(P >= 252 && P < 65536)
        && !use_specialized_storage(P)
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

/// Packs a slice of `Fp<P>` where `P == 65537` into canonical `Vec<u32>`.
///
/// For `P = 65537`, Montgomery storage equals the canonical value because
/// `R = 2^64 ≡ 1 (mod P)`, so `raw_storage()` is used directly.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp65537_pack<const P: u64>(xs: &[Fp<P>]) -> Vec<u32> {
    debug_assert_eq!(P, 65537, "fp65537_pack: P must be 65537");
    xs.iter().map(|x| x.raw_storage() as u32).collect()
}

/// The inverse of [`fp65537_pack`]; every value in `xs` is `< 65537`.
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

// ---------------------------------------------------------------------------
// Fp<P> medium-prime SIMD helpers — `P ∈ (251, 65536)` (`word-fits-in-u16`).
// ---------------------------------------------------------------------------
//
// The kernel operates on **canonical** u16 values. Add/sub are linear in the
// Montgomery storage form (`aR + bR = (a+b)R`), so for those we pack/unpack
// via raw_storage and avoid the REDC round-trip. Multiplication is not
// linear in storage form, so we round-trip through `value()` / `Fp::new` to
// expose canonical residues to the Barrett kernel.

#[cfg(feature = "simd")]
#[inline]
const fn fp_medium_eligible<const P: u64>() -> bool {
    P >= 252 && P < 65536
}

#[cfg(feature = "simd")]
#[inline]
fn fp_medium_pack_canonical<const P: u64>(xs: &[Fp<P>]) -> Vec<u16> {
    debug_assert!(
        fp_medium_eligible::<P>(),
        "fp_medium_pack_canonical: P out of range"
    );
    xs.iter().map(|x| x.value() as u16).collect()
}

#[cfg(feature = "simd")]
#[inline]
fn fp_medium_unpack_canonical<const P: u64>(xs: &[u16]) -> Vec<Fp<P>> {
    debug_assert!(
        fp_medium_eligible::<P>(),
        "fp_medium_unpack_canonical: P out of range"
    );
    xs.iter().map(|&x| Fp::<P>::new(x as u64)).collect()
}

#[cfg(feature = "simd")]
#[inline]
fn fp_medium_pack_raw<const P: u64>(xs: &[Fp<P>]) -> Vec<u16> {
    // Storage-domain pack: Montgomery residues are in `[0, P) ⊆ [0, 2^16)`,
    // so a `u64 → u16` truncation is exact.
    debug_assert!(
        fp_medium_eligible::<P>(),
        "fp_medium_pack_raw: P out of range"
    );
    xs.iter().map(|x| x.raw_storage() as u16).collect()
}

#[cfg(feature = "simd")]
#[inline]
fn fp_medium_unpack_raw<const P: u64>(xs: &[u16]) -> Vec<Fp<P>> {
    debug_assert!(
        fp_medium_eligible::<P>(),
        "fp_medium_unpack_raw: P out of range"
    );
    xs.iter()
        .map(|&x| Fp::<P>::from_raw_storage(x as u64))
        .collect()
}

#[cfg(feature = "simd")]
fn fp_medium_try_mul_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_medium_eligible::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_medium()?;
    let n = a.len();
    let a_u16 = fp_medium_pack_canonical::<P>(a);
    let b_u16 = fp_medium_pack_canonical::<P>(b);
    let mut out = vec![0u16; n];
    let p16 = P as u16;
    let m32 = gf2_kernels_simd::fp_medium::barrett_m32(p16);
    (fns.batch_mul_fn)(&a_u16, &b_u16, p16, m32, &mut out);
    Some(fp_medium_unpack_canonical::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp_medium_try_add_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_medium_eligible::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_medium()?;
    let n = a.len();
    let a_u16 = fp_medium_pack_raw::<P>(a);
    let b_u16 = fp_medium_pack_raw::<P>(b);
    let mut out = vec![0u16; n];
    (fns.batch_add_fn)(&a_u16, &b_u16, P as u16, &mut out);
    Some(fp_medium_unpack_raw::<P>(&out))
}

#[cfg(feature = "simd")]
fn fp_medium_try_sub_vec<const P: u64>(a: &[Fp<P>], b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    if !fp_medium_eligible::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_medium()?;
    let n = a.len();
    let a_u16 = fp_medium_pack_raw::<P>(a);
    let b_u16 = fp_medium_pack_raw::<P>(b);
    let mut out = vec![0u16; n];
    (fns.batch_sub_fn)(&a_u16, &b_u16, P as u16, &mut out);
    Some(fp_medium_unpack_raw::<P>(&out))
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_medium_try_mul_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_medium_try_add_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
fn fp_medium_try_sub_vec<const P: u64>(_a: &[Fp<P>], _b: &[Fp<P>]) -> Option<Vec<Fp<P>>> {
    None
}

/// SIMD batch dot product `Σ a[i] · b[i]` for `Fp<P>` with
/// `P ∈ (251, 65536)`; `None` when the prime is ineligible or the kernel
/// is unavailable.
///
/// Operates on Montgomery raw storage. Storage words are in
/// `[0, P) ⊆ [0, 2^16)`, so packing is a `u64 → u16` truncation. The
/// kernel computes
///
/// ```text
///   total = Σ raw(aᵢ) · raw(bᵢ)   (in u64, exact for n < 2^32)
/// ```
///
/// reduced modulo `P`, which is congruent to `R² · Σ aᵢbᵢ (mod P)` (see
/// `Fp::mul_product_sum_wide` for the representation proof). One
/// Montgomery REDC then recovers `R · Σ aᵢbᵢ (mod P)`, the storage form of
/// the dot product, matching `Fp::reduce_product_sum_wide` on the scalar
/// path.
#[cfg(feature = "simd")]
pub(crate) fn fp_medium_try_dot_product<const P: u64>(
    a: &[Fp<P>],
    b: &[Fp<P>],
    scratch_a: &mut Vec<u16>,
    scratch_b: &mut Vec<u16>,
) -> Option<Fp<P>> {
    if !fp_medium_eligible::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_medium()?;

    scratch_a.clear();
    scratch_b.clear();
    scratch_a.reserve(a.len());
    scratch_b.reserve(b.len());
    for x in a {
        scratch_a.push(x.raw_storage() as u16);
    }
    for y in b {
        scratch_b.push(y.raw_storage() as u16);
    }

    let r2_sum_mod_p = (fns.batch_dot_fn)(scratch_a, scratch_b, P as u16) as u64;
    let r_sum_mod_p = super::montgomery::redc::<P>(r2_sum_mod_p as u128);
    Some(Fp::<P>::from_raw_storage(r_sum_mod_p))
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_medium_try_dot_product<const P: u64>(
    _a: &[Fp<P>],
    _b: &[Fp<P>],
    _scratch_a: &mut Vec<u16>,
    _scratch_b: &mut Vec<u16>,
) -> Option<Fp<P>> {
    None
}

/// GEMM helper: packs `Fp<P>` Montgomery raw storage as `u16` for
/// [`fp_medium_try_dot_packed`]. Returns `None` when the medium-prime fast
/// path is unavailable.
#[cfg(feature = "simd")]
pub(crate) fn fp_medium_try_pack_u16<const P: u64>(xs: &[Fp<P>], out: &mut Vec<u16>) -> Option<()> {
    if !fp_medium_eligible::<P>() {
        return None;
    }
    crate::simd::maybe_fp_medium()?;
    out.clear();
    out.reserve(xs.len());
    for x in xs {
        out.push(x.raw_storage() as u16);
    }
    Some(())
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_medium_try_pack_u16<const P: u64>(
    _xs: &[Fp<P>],
    _out: &mut Vec<u16>,
) -> Option<()> {
    None
}

/// GEMM helper: [`fp_medium_try_dot_product`] on u16 raw-storage slices
/// pre-packed by [`fp_medium_try_pack_u16`].
#[cfg(feature = "simd")]
pub(crate) fn fp_medium_try_dot_packed<const P: u64>(
    a_packed: &[u16],
    b_packed: &[u16],
) -> Option<Fp<P>> {
    if !fp_medium_eligible::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_medium()?;
    let r2_sum_mod_p = (fns.batch_dot_fn)(a_packed, b_packed, P as u16) as u64;
    let r_sum_mod_p = super::montgomery::redc::<P>(r2_sum_mod_p as u128);
    Some(Fp::<P>::from_raw_storage(r_sum_mod_p))
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_medium_try_dot_packed<const P: u64>(
    _a_packed: &[u16],
    _b_packed: &[u16],
) -> Option<Fp<P>> {
    None
}

/// Conservative default for the tuning profile's `prime_route.f64_min_cols`
/// field: the minimum output width, in columns, at which `select_f64_path`
/// prefers the f64-FMA cascade.
pub(crate) const F64_MIN_COLS: usize = 512;

/// The `prime_route.f64_min_cols` value [`select_f64_path`] compares `n`
/// against; the mechanism is [`F32_MIN_PRIME_SELECTED`]'s.
#[cfg(all(feature = "simd", gf2_tuning_baked))]
const F64_MIN_COLS_SELECTED: usize = crate::tuning::baked::F64_MIN_COLS;

/// The `prime_route.f64_min_cols` value [`select_f64_path`] compares `n`
/// against; the mechanism is [`F32_MIN_PRIME_SELECTED`]'s.
#[cfg(all(feature = "simd", not(gf2_tuning_baked)))]
const F64_MIN_COLS_SELECTED: usize = F64_MIN_COLS;

/// Per-(P, m, k, n) f64-cascade selector for medium primes.
///
/// Returns `true` when the f64-FMA cascade is the default arm for this
/// size. The cascade's pack is one `Fp::value()` REDC per A/B^T element,
/// where the u16 panel kernel's is a `u64 → u16` truncation, so the cascade
/// is selected only from a column threshold up: the tuning profile's
/// `prime_route.f64_min_cols` field through [`F64_MIN_COLS_SELECTED`].
#[cfg(feature = "simd")]
#[inline]
const fn select_f64_path<const P: u64>(_m: usize, _k: usize, n: usize) -> bool {
    P > 251 && P < 65536 && n >= F64_MIN_COLS_SELECTED
}

/// The arm the prime-field GEMM dispatchers run for one cell.
///
/// Each variant names one kernel invocation site: `prime_gemm_select`
/// chooses the variant and the dispatcher's `match` runs the site it names, so
/// the enum is the whole vocabulary of prime-field GEMM arms. The two cascades
/// cover disjoint prime windows — the f32 variants are reachable only for
/// small primes and [`PrimeGemmRoute::F64Cascade`] only for medium ones.
#[cfg(feature = "simd")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimeGemmRoute {
    /// The f32-FMA cascade through route A's `batch_gemm_route_a_fn`: lookup-
    /// table pack and unpack around a vectorized Barrett output reduction.
    F32CascadeTabled,
    /// The f32-FMA cascade through Candidate F's `batch_gemm_fn`, which
    /// converts each element directly and allocates its own scratch.
    /// Reachable when the window predicate admits a prime other than 251,
    /// which a `prime_route.f32_min_prime` below 251 does.
    F32CascadeDirect,
    /// The f64-FMA cascade in `fp_medium_f64_try_gemm`.
    F64Cascade,
    /// Route C's pure-integer panelized micro-kernel for GF(251), reachable
    /// only through [`set_route_c_gf251_enabled`].
    U8Panel,
    /// Candidate C's byte-lane row-panel kernel, the small-prime baseline.
    ByteLaneBaseline,
    /// The u16 panel kernel, the medium-prime baseline.
    U16Panel,
    /// Neither dispatcher runs a SIMD arm: the prime falls outside both
    /// windows, the shape is degenerate, or no kernel is registered on this
    /// host. The caller's own fallback computes the product — its scalar loop
    /// for an in-window prime, or the generic Montgomery kernel for
    /// `P >= 65536`.
    Deferred,
}

#[cfg(feature = "simd")]
impl PrimeGemmRoute {
    /// Returns `true` for either f32-FMA cascade arm.
    #[must_use]
    pub fn is_f32_cascade(self) -> bool {
        matches!(
            self,
            PrimeGemmRoute::F32CascadeTabled | PrimeGemmRoute::F32CascadeDirect
        )
    }
}

/// Selects the prime-field GEMM arm for `Fp<P>` at output shape `m × n` with
/// inner dimension `k`.
///
/// This is the single selection authority for prime-field GEMM: both
/// dispatchers `match` on its result to choose which kernel to invoke, and
/// [`prime_gemm_route`] exposes it unchanged, so a reported route and an
/// executed arm cannot drift apart. The chain runs in dispatch order:
///
/// 1. the degenerate-shape guard both dispatchers apply before selecting;
/// 2. the eligibility windows `fp_small_enabled` and `fp_medium_eligible`;
/// 3. within the small window — route A's guard (the
///    [`set_route_a_gf251_enabled`] switch or the window predicate at
///    `P == 251`), then route C's switch, then Candidate F's window predicate,
///    then Candidate C;
/// 4. within the medium window — [`select_f64_path`], then the u16 panel.
///
/// Every step that names a kernel also consults that kernel's registration in
/// [`crate::simd`], because a dispatcher whose kernel is absent falls through
/// to the next arm rather than running the one its predicate named.
///
/// The window predicates [`select_f32_path`] and [`select_f64_path`] take
/// their bounds from the tuning profile's `prime_route.f32_min_prime`,
/// `prime_route.f32_min_cols` and `prime_route.f64_min_cols` fields, baked at
/// compile time; the rest of the chain is runtime state, so the selected arm
/// depends on the host's detected kernels and on the current switch settings.
#[cfg(feature = "simd")]
#[must_use]
pub(crate) fn prime_gemm_select<const P: u64>(m: usize, k: usize, n: usize) -> PrimeGemmRoute {
    if m == 0 || k == 0 || n == 0 {
        return PrimeGemmRoute::Deferred;
    }

    if fp_small_enabled::<P>() {
        let f32_selected = select_f32_path::<P>(m, k, n);
        if (route_a_gf251_enabled::<P>() || (f32_selected && P == 251))
            && crate::simd::maybe_fp_small_f32().is_some()
        {
            return PrimeGemmRoute::F32CascadeTabled;
        }
        if route_c_gf251_enabled::<P>() && crate::simd::maybe_fp_small_panel().is_some() {
            return PrimeGemmRoute::U8Panel;
        }
        if f32_selected && crate::simd::maybe_fp_small_f32().is_some() {
            return PrimeGemmRoute::F32CascadeDirect;
        }
        return if crate::simd::maybe_fp_small().is_some() {
            PrimeGemmRoute::ByteLaneBaseline
        } else {
            PrimeGemmRoute::Deferred
        };
    }

    if fp_medium_eligible::<P>() {
        if select_f64_path::<P>(m, k, n) && crate::simd::maybe_fp_medium_f64().is_some() {
            return PrimeGemmRoute::F64Cascade;
        }
        return if crate::simd::maybe_fp_medium().is_some() {
            PrimeGemmRoute::U16Panel
        } else {
            PrimeGemmRoute::Deferred
        };
    }

    PrimeGemmRoute::Deferred
}

/// Reports the prime-field GEMM arm for the field `Fp<P>` at output shape
/// `m × n` with inner dimension `k`.
///
/// The reporter is `prime_gemm_select`, the function the dispatchers
/// themselves select on, so what this reports is what the dispatcher runs.
/// The window bounds (`prime_route.f32_min_prime`, `prime_route.f32_min_cols`
/// and `prime_route.f64_min_cols`) are baked at compile time; the
/// host-detected kernels and the GF(251) switches are runtime state.
#[cfg(feature = "simd")]
#[must_use]
pub fn prime_gemm_route<const P: u64>(m: usize, k: usize, n: usize) -> PrimeGemmRoute {
    prime_gemm_select::<P>(m, k, n)
}

/// Records the arm a dispatcher executed, for the production-dispatch witness.
///
/// The dispatchers store their arm at its kernel-invocation site, so a test
/// can compare the executed arm against [`prime_gemm_route`]'s report for the
/// same cell rather than trusting that the two agree.
#[cfg(all(feature = "simd", any(test, feature = "test-support")))]
static LAST_EXECUTED_PRIME_GEMM_ROUTE: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Records `route` as the arm the running dispatcher chose.
#[cfg(all(feature = "simd", any(test, feature = "test-support")))]
#[inline]
fn record_executed_prime_gemm_route(route: PrimeGemmRoute) {
    let encoded = match route {
        PrimeGemmRoute::F32CascadeTabled => 0,
        PrimeGemmRoute::F32CascadeDirect => 1,
        PrimeGemmRoute::F64Cascade => 2,
        PrimeGemmRoute::U8Panel => 3,
        PrimeGemmRoute::ByteLaneBaseline => 4,
        PrimeGemmRoute::U16Panel => 5,
        PrimeGemmRoute::Deferred => 6,
    };
    LAST_EXECUTED_PRIME_GEMM_ROUTE.store(encoded, Ordering::Relaxed);
}

/// Clears the test-support observation of the last executed GEMM arm.
#[cfg(all(feature = "simd", any(test, feature = "test-support")))]
pub fn reset_last_executed_prime_gemm_route() {
    LAST_EXECUTED_PRIME_GEMM_ROUTE.store(usize::MAX, Ordering::Relaxed);
}

/// Returns the arm the last prime-field GEMM dispatch executed, or `None` when
/// no dispatch has run since the last reset.
///
/// This observation is available in tests and `test-support` builds so the
/// production-dispatch witness can assert that the executed arm is the one
/// [`prime_gemm_route`] reports.
#[cfg(all(feature = "simd", any(test, feature = "test-support")))]
#[must_use]
pub fn last_executed_prime_gemm_route() -> Option<PrimeGemmRoute> {
    match LAST_EXECUTED_PRIME_GEMM_ROUTE.load(Ordering::Relaxed) {
        0 => Some(PrimeGemmRoute::F32CascadeTabled),
        1 => Some(PrimeGemmRoute::F32CascadeDirect),
        2 => Some(PrimeGemmRoute::F64Cascade),
        3 => Some(PrimeGemmRoute::U8Panel),
        4 => Some(PrimeGemmRoute::ByteLaneBaseline),
        5 => Some(PrimeGemmRoute::U16Panel),
        6 => Some(PrimeGemmRoute::Deferred),
        _ => None,
    }
}

/// GEMM helper: whole-GEMM path for medium-prime `Fp<P>` with
/// `P ∈ (251, 65535]`; runs the arm [`prime_gemm_select`] names.
///
/// The u16 panel arm packs both operands as Montgomery raw u16. The panel
/// kernel computes `(Σ a_pack[i,t] * b_pack[j,t]) mod p`, which for raw
/// inputs is `R² · Σ a_canonical b_canonical mod p`; one REDC per output
/// cell maps `R² · x → R · x = Mont(x)`. The f64 cascade arm is
/// [`fp_medium_f64_try_gemm`].
///
/// Returns `true` when a kernel ran (and `out` is populated); `false` when
/// the field is out of range, a dimension is zero, the `simd` feature is
/// disabled, or AVX2 detection failed.
#[cfg(feature = "simd")]
pub(crate) fn fp_medium_try_gemm_panel<const P: u64>(
    a: &[Fp<P>],
    b_t: &[Fp<P>],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [Fp<P>],
) -> bool {
    debug_assert_eq!(a.len(), m * k, "fp_medium_try_gemm_panel: a shape");
    debug_assert_eq!(b_t.len(), n * k, "fp_medium_try_gemm_panel: b_t shape");
    debug_assert_eq!(out.len(), m * n, "fp_medium_try_gemm_panel: out shape");

    // Arm selection belongs to `prime_gemm_select` alone; see the small-prime
    // dispatcher for the shape of this match.
    match prime_gemm_select::<P>(m, k, n) {
        PrimeGemmRoute::F64Cascade => {
            #[cfg(any(test, feature = "test-support"))]
            record_executed_prime_gemm_route(PrimeGemmRoute::F64Cascade);
            fp_medium_f64_try_gemm::<P>(a, b_t, m, k, n, out)
        }

        PrimeGemmRoute::U16Panel => {
            #[cfg(any(test, feature = "test-support"))]
            record_executed_prime_gemm_route(PrimeGemmRoute::U16Panel);
            let Some(fns) = crate::simd::maybe_fp_medium() else {
                return false;
            };

            GEMM_MEDIUM_A_SCRATCH.with_borrow_mut(|a_u16| {
                GEMM_MEDIUM_BT_SCRATCH.with_borrow_mut(|bt_u16| {
                    GEMM_MEDIUM_OUT_SCRATCH.with_borrow_mut(|out_u16| {
                        a_u16.resize(m * k, 0u16);
                        bt_u16.resize(n * k, 0u16);
                        out_u16.resize(m * n, 0u16);
                        for (dst, src) in a_u16.iter_mut().zip(a.iter()) {
                            *dst = src.raw_storage() as u16;
                        }
                        for (dst, src) in bt_u16.iter_mut().zip(b_t.iter()) {
                            *dst = src.raw_storage() as u16;
                        }

                        (fns.gemm_panel_fn)(a_u16, bt_u16, m, k, n, P as u16, out_u16);

                        for (slot, &word) in out.iter_mut().zip(out_u16.iter()) {
                            let r2_sum = word as u128;
                            let r_sum = super::montgomery::redc::<P>(r2_sum);
                            *slot = Fp::<P>::from_raw_storage(r_sum);
                        }
                        true
                    })
                })
            })
        }

        PrimeGemmRoute::F32CascadeTabled
        | PrimeGemmRoute::F32CascadeDirect
        | PrimeGemmRoute::U8Panel
        | PrimeGemmRoute::ByteLaneBaseline
        | PrimeGemmRoute::Deferred => false,
    }
}

/// f64-cascade GEMM helper for medium primes. Pre-packs A and B^T as canonical
/// f64 (via per-element `Fp::value()` REDC), runs the AVX2 + FMA3 dgemm micro-kernel, then re-packs
/// the canonical-u16 output as `Fp::new(u as u64)` per cell.
///
/// Returns `true` when the kernel ran (and `out` is populated); `false`
/// when AVX2 + FMA3 is unavailable at runtime. The caller gates the shape
/// through [`select_f64_path`].
#[cfg(feature = "simd")]
fn fp_medium_f64_try_gemm<const P: u64>(
    a: &[Fp<P>],
    b_t: &[Fp<P>],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [Fp<P>],
) -> bool {
    let Some(fns_f64) = crate::simd::maybe_fp_medium_f64() else {
        return false;
    };
    GEMM_MEDIUM_F64_A_SCRATCH.with_borrow_mut(|a_f64| {
        GEMM_MEDIUM_F64_BT_SCRATCH.with_borrow_mut(|bt_f64| {
            GEMM_MEDIUM_OUT_SCRATCH.with_borrow_mut(|out_u16| {
                a_f64.resize(m * k, 0.0f64);
                bt_f64.resize(n * k, 0.0f64);
                out_u16.resize(m * n, 0u16);

                for (dst, src) in a_f64.iter_mut().zip(a.iter()) {
                    *dst = src.value() as f64;
                }
                for (dst, src) in bt_f64.iter_mut().zip(b_t.iter()) {
                    *dst = src.value() as f64;
                }

                (fns_f64.batch_gemm_fn)(a_f64, bt_f64, m, k, n, P as u16, out_u16);

                for (slot, &word) in out.iter_mut().zip(out_u16.iter()) {
                    *slot = Fp::<P>::new(word as u64);
                }
                true
            })
        })
    })
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_medium_try_gemm_panel<const P: u64>(
    _a: &[Fp<P>],
    _b_t: &[Fp<P>],
    _m: usize,
    _k: usize,
    _n: usize,
    _out: &mut [Fp<P>],
) -> bool {
    false
}

// ---------------------------------------------------------------------------
// Packed matvec entry points
// ---------------------------------------------------------------------------
//
// `fp_try_matvec` packs `A` and `x` per call for `FieldMatrix::matvec`;
// `PackedFpMatrix` packs `A` once and reuses the pack across the matvec
// calls of `cyclic_decomposition` and `wiedemann_minpoly_attempt`.

// Thread-local scratch buffers for the matvec and GEMM pack and output
// stages. They grow as needed and are never shrunk.
#[cfg(feature = "simd")]
thread_local! {
    static SMALL_X_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static SMALL_OUT_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_A_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_BT_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_OUT_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_F32_A_SCRATCH: std::cell::RefCell<Vec<f32>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_F32_BT_SCRATCH: std::cell::RefCell<Vec<f32>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_MEDIUM_A_SCRATCH: std::cell::RefCell<Vec<u16>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_MEDIUM_BT_SCRATCH: std::cell::RefCell<Vec<u16>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_MEDIUM_OUT_SCRATCH: std::cell::RefCell<Vec<u16>> =
        const { std::cell::RefCell::new(Vec::new()) };
    // The f64 cascade shares `GEMM_MEDIUM_OUT_SCRATCH` for its output.
    static GEMM_MEDIUM_F64_A_SCRATCH: std::cell::RefCell<Vec<f64>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_MEDIUM_F64_BT_SCRATCH: std::cell::RefCell<Vec<f64>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_M31_A_SCRATCH: std::cell::RefCell<Vec<u32>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_M31_BT_SCRATCH: std::cell::RefCell<Vec<u32>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Per-prime lookup tables converting Montgomery-stored `Fp<P>` values
/// (`P ≤ 251`) to and from canonical bytes without a REDC per element.
#[cfg(feature = "simd")]
pub(crate) struct SmallPrimeTables {
    from_mont: Vec<u8>, // index = raw storage word (in [0, P)); value = canonical
    to_mont: Vec<u64>,  // index = canonical value (in [0, P)); value = raw storage
    /// 16-bit Barrett constant `μ = ⌊2¹⁶ / P⌋`, passed to the `fp_small`
    /// `sub_scaled` kernel so it skips a per-call division.
    barrett_mu: u16,
    /// `from_mont_f32[raw]` — canonical value as `f32` for a Montgomery
    /// word `raw` in `[0, P)`; the pack table of the route-A f32 cascade.
    from_mont_f32: Vec<f32>,
    /// `inv_table[v]` — modular inverse of `v` in canonical-byte form,
    /// for `v ∈ [1, P)`. `inv_table[0]` is unused (kept 0). The pivot-inverse
    /// table of the panelized PLE base-case kernel.
    inv_table: Vec<u8>,
}

// One `OnceLock` slot per prime value: a static inside the generic
// `build_small_prime_tables` would be shared across all its instantiations.
#[cfg(feature = "simd")]
static SMALL_PRIME_TABLE_SLOTS: [std::sync::OnceLock<SmallPrimeTables>; 256] = {
    // All 256 slots start uninitialised.
    [const { std::sync::OnceLock::new() }; 256]
};

/// Returns the per-prime lookup tables for `Fp<P>` with `3 ≤ P ≤ 251`,
/// built at most once per prime per process.
#[cfg(feature = "simd")]
fn build_small_prime_tables<const P: u64>() -> &'static SmallPrimeTables {
    debug_assert!(
        P <= 251 && P >= 3,
        "build_small_prime_tables: P={P} out of range"
    );
    SMALL_PRIME_TABLE_SLOTS[P as usize].get_or_init(|| {
        let p = P as usize;
        let mut from_mont = vec![0u8; p];
        let mut to_mont = vec![0u64; p];
        let mut from_mont_f32 = vec![0.0f32; p];
        for (a, slot) in from_mont.iter_mut().enumerate() {
            let canon = Fp::<P>::from_raw_storage(a as u64).value(); // from_mont(a)
            *slot = canon as u8;
            from_mont_f32[a] = canon as f32;
            let raw = Fp::<P>::new(canon).raw_storage();
            to_mont[canon as usize] = raw;
        }
        let barrett_mu = gf2_kernels_simd::fp_small::barrett_mu_u16(P as u8);
        // inv_table[v] = v^{P-2} mod P for v ∈ [1, P).
        let mut inv_table = vec![0u8; p];
        for v in 1..p as u64 {
            let mut result: u64 = 1;
            let mut base: u64 = v;
            let mut e: u64 = P - 2;
            let p_u64: u64 = P;
            while e > 0 {
                if e & 1 == 1 {
                    result = (result * base) % p_u64;
                }
                e >>= 1;
                if e > 0 {
                    base = (base * base) % p_u64;
                }
            }
            inv_table[v as usize] = result as u8;
        }
        SmallPrimeTables {
            from_mont,
            to_mont,
            barrett_mu,
            from_mont_f32,
            inv_table,
        }
    })
}

/// Internal cache that holds a pre-packed copy of an `m × k` `Fp<P>`
/// matrix in the canonical-byte (`P ≤ 251`) or storage-domain-`u16`
/// (`252 ≤ P < 65536`) layout used by the AVX2 kernels.
#[cfg(feature = "simd")]
pub(crate) enum PackedFpMatrix<const P: u64> {
    /// Small-prime layout — canonical bytes, length `m · k`.
    Small {
        data: Vec<u8>,
        m: usize,
        k: usize,
        fns: gf2_kernels_simd::fp_small::SmallPrimeFns,
        tables: &'static SmallPrimeTables,
    },
    /// Medium-prime layout — storage-domain `u16`s, length `m · k`.
    /// The dot kernel returns a canonical `u32` and we apply one Montgomery
    /// REDC at the row boundary to recover `Fp<P>` storage.
    Medium { data: Vec<u16>, m: usize, k: usize },
}

#[cfg(feature = "simd")]
impl<const P: u64> std::fmt::Debug for PackedFpMatrix<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackedFpMatrix::Small { m, k, .. } => f
                .debug_struct("PackedFpMatrix::Small")
                .field("P", &P)
                .field("m", m)
                .field("k", k)
                .finish_non_exhaustive(),
            PackedFpMatrix::Medium { m, k, .. } => f
                .debug_struct("PackedFpMatrix::Medium")
                .field("P", &P)
                .field("m", m)
                .field("k", k)
                .finish(),
        }
    }
}

#[cfg(feature = "simd")]
impl<const P: u64> PackedFpMatrix<P> {
    /// Pre-packs an `m × k` row-major `Fp<P>` matrix for the AVX2
    /// matvec kernel. Returns `None` when no fast path is available
    /// (`P` out of range, the `simd` feature off, or AVX2 missing at
    /// runtime).
    pub(crate) fn try_pack(rows: &[Fp<P>], m: usize, k: usize) -> Option<Self> {
        debug_assert_eq!(rows.len(), m * k);
        if fp_small_enabled::<P>() {
            let fns = *crate::simd::maybe_fp_small()?;
            let tables = build_small_prime_tables::<P>();
            let data: Vec<u8> = rows
                .iter()
                .map(|x| tables.from_mont[x.raw_storage() as usize])
                .collect();
            return Some(PackedFpMatrix::Small {
                data,
                m,
                k,
                fns,
                tables,
            });
        }
        if fp_medium_eligible::<P>() {
            crate::simd::maybe_fp_medium()?;
            let data: Vec<u16> = rows.iter().map(|x| x.raw_storage() as u16).collect();
            return Some(PackedFpMatrix::Medium { data, m, k });
        }
        None
    }

    /// Computes `y = A · x` using the pre-packed matrix. Writes into
    /// `out` (length `m`).
    ///
    /// For `P ≤ 251` uses the AVX2 `gemm_row_panel_fn` kernel. For medium
    /// primes (`252 ≤ P < 65536`) the dot kernel is called per row.
    pub(crate) fn matvec_packed(&self, x: &[Fp<P>], out: &mut [Fp<P>]) {
        match self {
            PackedFpMatrix::Small {
                data,
                m,
                k,
                fns,
                tables,
            } => {
                debug_assert_eq!(x.len(), *k);
                debug_assert_eq!(out.len(), *m);
                let p_u8 = P as u8;

                SMALL_X_SCRATCH.with_borrow_mut(|x_u8| {
                    x_u8.resize(*k, 0u8);
                    for (dst, v) in x_u8.iter_mut().zip(x.iter()) {
                        *dst = tables.from_mont[v.raw_storage() as usize];
                    }
                    SMALL_OUT_SCRATCH.with_borrow_mut(|out_u8| {
                        out_u8.resize(*m, 0u8);
                        // Row-panel GEMM with `x` as the single left row:
                        // y[j] = sum_t x[t] * A[j*k+t].
                        (fns.gemm_row_panel_fn)(&x_u8[..*k], data, *k, *m, p_u8, &mut out_u8[..*m]);
                        for (slot, &b) in out.iter_mut().zip(out_u8[..*m].iter()) {
                            *slot = Fp::<P>::from_raw_storage(tables.to_mont[b as usize]);
                        }
                    });
                });
            }
            PackedFpMatrix::Medium { data, m, k } => {
                debug_assert_eq!(x.len(), *k);
                debug_assert_eq!(out.len(), *m);
                let fns = crate::simd::maybe_fp_medium().expect(
                    "PackedFpMatrix::Medium requires AVX2 (try_pack would have returned None)",
                );
                let x_u16: Vec<u16> = x.iter().map(|v| v.raw_storage() as u16).collect();
                for r in 0..*m {
                    let row = &data[r * *k..(r + 1) * *k];
                    let r2_sum = (fns.batch_dot_fn)(row, &x_u16, P as u16) as u64;
                    let r_sum = super::montgomery::redc::<P>(r2_sum as u128);
                    out[r] = Fp::<P>::from_raw_storage(r_sum);
                }
            }
        }
    }
}

#[cfg(feature = "simd")]
impl<const P: u64> crate::field::matrix::PackedMatvec<Fp<P>> for PackedFpMatrix<P> {
    fn matvec(&self, x: &[Fp<P>], out: &mut [Fp<P>]) {
        self.matvec_packed(x, out);
    }
}

#[cfg(not(feature = "simd"))]
#[derive(Debug)]
#[allow(dead_code)] // No-simd stub; constructed only from feature-gated code paths.
pub(crate) struct PackedFpMatrix<const P: u64>;

#[cfg(not(feature = "simd"))]
#[allow(dead_code)] // No-simd stubs; called only from feature-gated code paths.
impl<const P: u64> PackedFpMatrix<P> {
    pub(crate) fn try_pack(_rows: &[Fp<P>], _m: usize, _k: usize) -> Option<Self> {
        None
    }
    pub(crate) fn matvec_packed(&self, _x: &[Fp<P>], _out: &mut [Fp<P>]) {
        unreachable!("PackedFpMatrix::matvec_packed called without simd feature")
    }
}

/// One-shot SIMD matvec for `Fp<P>`. Packs `a` and `x` per call and
/// dispatches to the AVX2 byte-lane (`P ≤ 251`) or u16-lane
/// (`252 ≤ P < 65536`) kernel. Returns `true` on success, `false`
/// when `k == 0`, the field is out of range or the kernel is unavailable.
/// [`PackedFpMatrix`] pays the pack once for repeated calls on the same `a`.
#[cfg(feature = "simd")]
pub(crate) fn fp_try_matvec<const P: u64>(
    a: &[Fp<P>],
    x: &[Fp<P>],
    m: usize,
    k: usize,
    out: &mut [Fp<P>],
) -> bool {
    debug_assert_eq!(a.len(), m * k);
    debug_assert_eq!(x.len(), k);
    debug_assert_eq!(out.len(), m);
    if k == 0 {
        return false;
    }
    let Some(packed) = PackedFpMatrix::<P>::try_pack(a, m, k) else {
        return false;
    };
    packed.matvec_packed(x, out);
    true
}

/// SIMD-accelerated axpy (`y[i] += a · x[i]`) for `Fp<P>` with
/// `3 ≤ P < 65536`. Packs `y`, `x` and a broadcast of `a` to canonical
/// bytes (`P ≤ 251`) or canonical `u16`s (`252 ≤ P < 65536`) and runs the
/// AVX2 `batch_mul` + `batch_add` kernels. Returns `true` when `y` holds
/// the result, `false` to defer to the caller's scalar zip-loop.
#[cfg(feature = "simd")]
pub(crate) fn fp_try_axpy<const P: u64>(y: &mut [Fp<P>], a: &Fp<P>, x: &[Fp<P>]) -> bool {
    debug_assert_eq!(y.len(), x.len());
    let n = y.len();
    if n == 0 {
        return true;
    }
    if a.is_zero() {
        return true; // y unchanged
    }
    if fp_small_enabled::<P>() {
        let Some(fns) = crate::simd::maybe_fp_small() else {
            return false;
        };
        let p_u8 = P as u8;
        let a_canon = a.value() as u8;
        let mut y_u8: Vec<u8> = y.iter().map(|v| v.value() as u8).collect();
        let x_u8: Vec<u8> = x.iter().map(|v| v.value() as u8).collect();
        let bcast = vec![a_canon; n];
        let mut tmp = vec![0u8; n];
        (fns.batch_mul_fn)(&bcast, &x_u8, p_u8, &mut tmp);
        let mut new_y = vec![0u8; n];
        (fns.batch_add_fn)(&y_u8, &tmp, p_u8, &mut new_y);
        y_u8 = new_y;
        for (slot, &b) in y.iter_mut().zip(y_u8.iter()) {
            *slot = Fp::<P>::new(b as u64);
        }
        return true;
    }
    if fp_medium_eligible::<P>() {
        let Some(fns) = crate::simd::maybe_fp_medium() else {
            return false;
        };
        let p_u16 = P as u16;
        let barrett_m = gf2_kernels_simd::fp_medium::barrett_m32(p_u16);
        let a_canon = a.value() as u16;
        let mut y_u16: Vec<u16> = y.iter().map(|v| v.value() as u16).collect();
        let x_u16: Vec<u16> = x.iter().map(|v| v.value() as u16).collect();
        let bcast = vec![a_canon; n];
        let mut tmp = vec![0u16; n];
        (fns.batch_mul_fn)(&bcast, &x_u16, p_u16, barrett_m, &mut tmp);
        let mut new_y = vec![0u16; n];
        (fns.batch_add_fn)(&y_u16, &tmp, p_u16, &mut new_y);
        y_u16 = new_y;
        for (slot, &w) in y.iter_mut().zip(y_u16.iter()) {
            *slot = Fp::<P>::new(w as u64);
        }
        return true;
    }
    false
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_try_axpy<const P: u64>(_y: &mut [Fp<P>], _a: &Fp<P>, _x: &[Fp<P>]) -> bool {
    false
}

// ---------------------------------------------------------------------------
// Packed cyclic-decomposition basis cache
// ---------------------------------------------------------------------------

/// Cached canonical-form basis used by the cyclic-decomposition
/// reduce loop. Each pivot column is stored once in canonical form
/// (`P ≤ 251`: bytes; `252 ≤ P < 65536`: u16) and reused across all
/// reduce calls.
#[cfg(feature = "simd")]
pub(crate) enum PackedFpBasis<const P: u64> {
    Small {
        cols: Vec<Vec<u8>>,
        /// Pre-computed pivot inverses, one per column, indexed in lockstep
        /// with `cols`. `pivot_inv[j] = col_j[pivot_row_j]^{-1} (mod P)`,
        /// canonical byte form.
        pivot_inv: Vec<u8>,
        n: usize,
    },
    Medium {
        cols: Vec<Vec<u16>>,
        /// Pre-computed pivot inverses for the medium-prime path,
        /// `pivot_inv[j] = col_j[pivot_row_j]^{-1} (mod P)`, canonical u16.
        pivot_inv: Vec<u16>,
        n: usize,
    },
}

#[cfg(feature = "simd")]
impl<const P: u64> PackedFpBasis<P> {
    /// Constructs an empty packed basis appropriate for the field.
    /// Returns `None` for fields without a small or medium SIMD path.
    pub(crate) fn try_new(n: usize) -> Option<Self> {
        if fp_small_enabled::<P>() {
            crate::simd::maybe_fp_small()?;
            return Some(Self::Small {
                cols: Vec::new(),
                pivot_inv: Vec::new(),
                n,
            });
        }
        if fp_medium_eligible::<P>() {
            crate::simd::maybe_fp_medium()?;
            return Some(Self::Medium {
                cols: Vec::new(),
                pivot_inv: Vec::new(),
                n,
            });
        }
        None
    }

    /// Appends a column (as `Fp<P>`) by packing into canonical form and
    /// caching the inverse of its pivot entry. The caller guarantees
    /// `col[pivot_row]` is non-zero.
    pub(crate) fn push(&mut self, col: &[Fp<P>], pivot_row: usize) {
        match self {
            PackedFpBasis::Small { cols, pivot_inv, n } => {
                debug_assert_eq!(col.len(), *n);
                let packed: Vec<u8> = col.iter().map(|v| v.value() as u8).collect();
                let pivot_canon = packed[pivot_row];
                let inv = Fp::<P>::new(pivot_canon as u64)
                    .inv()
                    .expect("PackedFpBasis::push: pivot must be non-zero")
                    .value() as u8;
                cols.push(packed);
                pivot_inv.push(inv);
            }
            PackedFpBasis::Medium { cols, pivot_inv, n } => {
                debug_assert_eq!(col.len(), *n);
                let packed: Vec<u16> = col.iter().map(|v| v.value() as u16).collect();
                let pivot_canon = packed[pivot_row];
                let inv = Fp::<P>::new(pivot_canon as u64)
                    .inv()
                    .expect("PackedFpBasis::push: pivot must be non-zero")
                    .value() as u16;
                cols.push(packed);
                pivot_inv.push(inv);
            }
        }
    }
}

/// Packed `reduce` for the cyclic-decomposition basis sweep. Computes
/// `(residual, coeffs) = v − Σ coeffs[j] · basis[j]` where `coeffs[j]
/// = v[pivot_row[j]] / basis[j][pivot_row[j]]`. Operates entirely in
/// canonical form (bytes for `P ≤ 251`, u16 for `252 ≤ P < 65536`).
///
/// Returns the residual (re-packed to `Fp<P>` storage) and the coefficient
/// vector.
#[cfg(feature = "simd")]
pub(crate) fn fp_reduce_packed<const P: u64>(
    v: &[Fp<P>],
    basis: &PackedFpBasis<P>,
    pivot_row_of_col: &[usize],
) -> (Vec<Fp<P>>, Vec<Fp<P>>) {
    let n = v.len();
    let basis_len = pivot_row_of_col.len();
    match basis {
        PackedFpBasis::Small {
            cols, pivot_inv, ..
        } => {
            let fns = crate::simd::maybe_fp_small().expect("PackedFpBasis::Small requires AVX2");
            let p_u8 = P as u8;
            let tables = build_small_prime_tables::<P>();
            let from_mont = tables.from_mont.as_slice();
            let to_mont = tables.to_mont.as_slice();
            let barrett_mu = tables.barrett_mu;
            let mut residual: Vec<u8> = v
                .iter()
                .map(|x| from_mont[x.raw_storage() as usize])
                .collect();
            let mut coeffs: Vec<Fp<P>> = vec![Fp::<P>::new(0); basis_len];
            for (j, col) in cols.iter().enumerate() {
                let r = pivot_row_of_col[j];
                let v_at_r = residual[r];
                if v_at_r == 0 {
                    continue;
                }
                let factor = ((v_at_r as u32 * pivot_inv[j] as u32) % P as u32) as u8;
                // In place: residual := (residual − factor · col) mod p.
                (fns.sub_scaled_fn)(&mut residual, col, factor, p_u8, barrett_mu);
                coeffs[j] = Fp::<P>::from_raw_storage(to_mont[factor as usize]);
            }
            let unpacked: Vec<Fp<P>> = residual
                .iter()
                .map(|&b| Fp::<P>::from_raw_storage(to_mont[b as usize]))
                .collect();
            (unpacked, coeffs)
        }
        PackedFpBasis::Medium {
            cols, pivot_inv, ..
        } => {
            let fns = crate::simd::maybe_fp_medium().expect("PackedFpBasis::Medium requires AVX2");
            let p_u16 = P as u16;
            let barrett_m = gf2_kernels_simd::fp_medium::barrett_m32(p_u16);
            let mut residual: Vec<u16> = v.iter().map(|x| x.value() as u16).collect();
            let mut coeffs: Vec<Fp<P>> = vec![Fp::<P>::new(0); basis_len];
            let mut bcast: Vec<u16> = vec![0u16; n];
            let mut tmp: Vec<u16> = vec![0u16; n];
            let mut new_residual: Vec<u16> = vec![0u16; n];
            for (j, col) in cols.iter().enumerate() {
                let r = pivot_row_of_col[j];
                let v_at_r = residual[r];
                if v_at_r == 0 {
                    continue;
                }
                let factor = ((v_at_r as u64 * pivot_inv[j] as u64) % P) as u16;
                bcast.iter_mut().for_each(|s| *s = factor);
                (fns.batch_mul_fn)(&bcast, col, p_u16, barrett_m, &mut tmp);
                (fns.batch_sub_fn)(&residual, &tmp, p_u16, &mut new_residual);
                std::mem::swap(&mut residual, &mut new_residual);
                coeffs[j] = Fp::<P>::new(factor as u64);
            }
            let unpacked: Vec<Fp<P>> = residual.iter().map(|&w| Fp::<P>::new(w as u64)).collect();
            (unpacked, coeffs)
        }
    }
}

#[cfg(feature = "simd")]
impl<const P: u64> crate::field::matrix::BasisReducer<Fp<P>> for PackedFpBasis<P> {
    fn push_col(&mut self, col: &[Fp<P>]) {
        // The pivot is the first non-zero entry.
        let pivot_row = col
            .iter()
            .position(|v| !v.is_zero())
            .expect("PackedFpBasis::push_col: column must have a non-zero entry");
        self.push(col, pivot_row);
    }

    /// Skips the linear pivot scan of `push_col`.
    fn push_col_with_pivot_row(&mut self, col: &[Fp<P>], pivot_row: usize) {
        self.push(col, pivot_row);
    }

    fn reduce(&self, v: &[Fp<P>], pivot_row_of_col: &[usize]) -> (Vec<Fp<P>>, Vec<Fp<P>>) {
        fp_reduce_packed::<P>(v, self, pivot_row_of_col)
    }

    fn len(&self) -> usize {
        match self {
            PackedFpBasis::Small { cols, .. } => cols.len(),
            PackedFpBasis::Medium { cols, .. } => cols.len(),
        }
    }
}

#[cfg(feature = "simd")]
pub(crate) fn fp_try_make_basis_reducer<const P: u64>(
    n: usize,
) -> Option<Box<dyn crate::field::matrix::BasisReducer<Fp<P>>>> {
    let basis = PackedFpBasis::<P>::try_new(n)?;
    Some(Box::new(basis))
}

#[cfg(not(feature = "simd"))]
#[allow(dead_code)] // No-simd stub; constructed only from feature-gated code paths.
pub(crate) struct PackedFpBasis<const P: u64>;

#[cfg(not(feature = "simd"))]
#[allow(dead_code)] // No-simd stubs; called only from feature-gated code paths.
impl<const P: u64> PackedFpBasis<P> {
    pub(crate) fn try_new(_n: usize) -> Option<Self> {
        None
    }
    pub(crate) fn push(&mut self, _col: &[Fp<P>]) {}
}

#[cfg(not(feature = "simd"))]
#[allow(dead_code)] // No-simd stub; called only from feature-gated code paths.
pub(crate) fn fp_reduce_packed<const P: u64>(
    _v: &[Fp<P>],
    _basis: &PackedFpBasis<P>,
    _pivot_row_of_col: &[usize],
) -> (Vec<Fp<P>>, Vec<Fp<P>>) {
    unreachable!()
}

#[cfg(not(feature = "simd"))]
pub(crate) fn fp_try_make_basis_reducer<const P: u64>(
    _n: usize,
) -> Option<Box<dyn crate::field::matrix::BasisReducer<Fp<P>>>> {
    None
}

/// Pre-packs the `m × k` matrix `a` and returns it as a boxed
/// [`crate::field::matrix::PackedMatvec`] handle. Returns `None` for
/// fields without a SIMD fast path.
#[cfg(feature = "simd")]
pub(crate) fn fp_try_prepack_matvec<const P: u64>(
    a: &[Fp<P>],
    m: usize,
    k: usize,
) -> Option<Box<dyn crate::field::matrix::PackedMatvec<Fp<P>>>> {
    let packed = PackedFpMatrix::<P>::try_pack(a, m, k)?;
    Some(Box::new(packed))
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_try_prepack_matvec<const P: u64>(
    _a: &[Fp<P>],
    _m: usize,
    _k: usize,
) -> Option<Box<dyn crate::field::matrix::PackedMatvec<Fp<P>>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_try_matvec<const P: u64>(
    _a: &[Fp<P>],
    _x: &[Fp<P>],
    _m: usize,
    _k: usize,
    _out: &mut [Fp<P>],
) -> bool {
    false
}

// ---------------------------------------------------------------------------
// PackedFpChainPolys<P> — canonical-byte chain-polynomial arithmetic
// for `cyclic_decomposition`.
//
// Each chain polynomial of degree `d` is stored as a `Vec<u8>` of length
// `d + 1` in ascending-degree order (coeffs[i] = coeff of x^i), with all
// entries in `[0, P)`.  The Krylov-step update
//
//     next[d] = x · chain[d-1]  −  Σ_j α_j · chain[j]
//
// prepends a zero byte, then applies one fused `sub_scaled` per non-zero
// α_j. `finish_buf` converts the bytes back to `FieldPoly<Fp<P>>` via
// `Fp::new`.
// ---------------------------------------------------------------------------

/// Packed canonical-byte chain-polynomial store for small primes (`P ≤ 251`),
/// used by `cyclic_decomposition`.
///
/// Each `sub_scaled_into` call is one fused AVX2 kernel call
/// (`fns.sub_scaled_fn`, semantics `buf := (buf − α·chain_j) mod p`), `O(d)`
/// in the current chain length `d`; the polynomial bookkeeping for one
/// Krylov block of length `d` is `O(d²)`.
#[cfg(feature = "simd")]
pub(crate) struct PackedFpChainPolys<const P: u64> {
    /// Stored coefficients for each chain polynomial, in canonical bytes,
    /// ascending-degree order.  `polys[j]` has length `j + 1` (degree `j`).
    polys: Vec<Vec<u8>>,
    /// Per-prime conversion tables; `sub_scaled_into` reads `alpha`'s
    /// canonical byte from `from_mont` and the Barrett constant.
    tables: &'static SmallPrimeTables,
}

#[cfg(feature = "simd")]
impl<const P: u64> PackedFpChainPolys<P> {
    /// Constructs an empty store.  Returns `None` for primes outside the
    /// supported range (`P < 3` or `P > 251`) or when AVX2 is unavailable.
    pub(crate) fn try_new() -> Option<Self> {
        if !fp_small_enabled::<P>() {
            return None;
        }
        crate::simd::maybe_fp_small()?;
        Some(Self {
            polys: Vec::new(),
            tables: build_small_prime_tables::<P>(),
        })
    }
}

#[cfg(feature = "simd")]
impl<const P: u64> crate::field::matrix::ChainPolyArith<Fp<P>> for PackedFpChainPolys<P> {
    fn push_one(&mut self) {
        self.polys.push(vec![1u8]);
    }

    fn shift_x_last_into(&self, buf: &mut Vec<u8>) {
        // x · p(x) prepends a zero coefficient.
        let last = self.polys.last().expect("shift_x_last_into: empty chain");
        let new_len = last.len() + 1;
        buf.resize(new_len, 0u8);
        buf[1..new_len].copy_from_slice(last);
        buf[0] = 0;
    }

    fn sub_scaled_into(&mut self, buf: &mut Vec<u8>, alpha: &Fp<P>, j: usize) {
        let alpha_val = self.tables.from_mont[alpha.raw_storage() as usize];
        if alpha_val == 0 {
            return;
        }
        let fns = crate::simd::maybe_fp_small()
            .expect("PackedFpChainPolys::sub_scaled_into requires AVX2");
        let chain_j = &self.polys[j];
        debug_assert!(
            buf.len() >= chain_j.len(),
            "sub_scaled_into: buf len {} < chain_j len {}",
            buf.len(),
            chain_j.len()
        );
        // In place: buf[..cj_len] := (buf − α · chain_j) mod p.
        (fns.sub_scaled_fn)(
            &mut buf[..],
            chain_j,
            alpha_val,
            P as u8,
            self.tables.barrett_mu,
        );
    }

    fn push_buf(&mut self, buf: &[u8]) {
        self.polys.push(buf.to_vec());
    }

    fn finish_buf(&self, buf: &[u8], zero: &Fp<P>) -> crate::field::poly::FieldPoly<Fp<P>> {
        let coeffs: Vec<Fp<P>> = buf.iter().map(|&b| Fp::<P>::new(b as u64)).collect();
        let _ = zero;
        crate::field::poly::FieldPoly::from_coeffs_trimmed(coeffs)
    }

    fn alloc_buf(&self, max_deg: usize) -> Vec<u8> {
        vec![0u8; max_deg + 1]
    }

    fn len(&self) -> usize {
        self.polys.len()
    }
}

/// Returns a boxed [`crate::field::matrix::ChainPolyArith`] for
/// `Fp<P>` with `P ≤ 251` and AVX2 available, or `None` otherwise.
#[cfg(feature = "simd")]
pub(crate) fn fp_try_make_chain_poly_arith<const P: u64>(
    _n: usize,
) -> Option<Box<dyn crate::field::matrix::ChainPolyArith<Fp<P>>>> {
    let cpa = PackedFpChainPolys::<P>::try_new()?;
    Some(Box::new(cpa))
}

/// Non-allocating availability probe for [`fp_try_make_chain_poly_arith`].
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_chain_poly_arith_available<const P: u64>() -> bool {
    fp_small_enabled::<P>() && crate::simd::maybe_fp_small().is_some()
}

/// Non-allocating availability probe for
/// [`fp_small_try_gemm_classical`] and [`fp_medium_try_gemm_panel`].
///
/// Returns `true` when `P` is in the byte-lane range (`3..=251`) and a
/// small-prime kernel was detected at runtime (Candidate C
/// `maybe_fp_small`, route A `maybe_fp_small_f32`, or route C
/// `maybe_fp_small_panel`), or `P ∈ (251, 65535]` and the u16 kernel was
/// detected. Used by [`crate::field::matrix::gemm_axpy_into_view`] to skip
/// the contiguous-`A` scratch allocation when the kernel would decline.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_small_gemm_classical_available<const P: u64>() -> bool {
    if fp_small_enabled::<P>() {
        return crate::simd::maybe_fp_small().is_some()
            || crate::simd::maybe_fp_small_f32().is_some()
            || crate::simd::maybe_fp_small_panel().is_some();
    }
    if fp_medium_eligible::<P>() {
        return crate::simd::maybe_fp_medium().is_some();
    }
    false
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_small_gemm_classical_available<const P: u64>() -> bool {
    false
}

/// Panelized PLE base-case fast path for `Fp<P>`. Medium primes
/// (`P ∈ (251, 65536)`) delegate to [`fp_try_ple_panel_base_medium`].
///
/// For `P <= 251`, operates on the column window `[col_lo, col_hi)` of the
/// parent row-major matrix storage:
///   1. Packs the window into a canonical-byte scratch buffer (one
///      `from_mont` table lookup per cell).
///   2. Invokes the unsafe AVX2 kernel via
///      `crate::simd::maybe_fp_small_ple()`.
///   3. Propagates the kernel's row swaps to cells **outside** the
///      window (the kernel only touched the window's panel bytes).
///   4. Updates the caller-supplied `perm` and `pivot_cols` based on
///      the kernel's local result.
///   5. Unpacks the canonical-byte scratch back into Montgomery
///      storage in the parent matrix.
///
/// Returns `Some(rank)` on success; `None` when the kernel declined
/// (e.g. `P >= 65536`, the `simd` feature disabled, AVX2 unavailable at
/// runtime). The caller falls back to `ple_base_direct` in this case.
#[cfg(feature = "simd")]
pub(crate) fn fp_try_ple_panel_base<const P: u64>(
    matrix: &mut [Fp<P>],
    parent_cols: usize,
    m: usize,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> Option<usize> {
    if fp_medium_eligible::<P>() {
        return fp_try_ple_panel_base_medium::<P>(
            matrix,
            parent_cols,
            m,
            col_lo,
            col_hi,
            perm,
            pivot_cols,
        );
    }
    if !fp_small_enabled::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_small_ple()?;
    debug_assert_eq!(
        matrix.len(),
        m * parent_cols,
        "fp_try_ple_panel_base: matrix shape"
    );
    debug_assert_eq!(perm.len(), m, "fp_try_ple_panel_base: perm length");
    debug_assert!(
        col_lo <= col_hi && col_hi <= parent_cols,
        "fp_try_ple_panel_base: col window out of bounds"
    );

    let win = col_hi - col_lo;
    if m == 0 || win == 0 {
        return Some(0);
    }

    let p_u8 = P as u8;
    let tables = build_small_prime_tables::<P>();
    let from_mont = tables.from_mont.as_slice();
    let to_mont = tables.to_mont.as_slice();
    let inv_table = tables.inv_table.as_slice();

    let mut window: Vec<u8> = Vec::with_capacity(m * win);
    for r in 0..m {
        let row_base = r * parent_cols + col_lo;
        for c in 0..win {
            let raw = matrix[row_base + c].raw_storage() as usize;
            debug_assert!(raw < from_mont.len());
            window.push(from_mont[raw]);
        }
    }

    let mut row_perm: Vec<usize> = (0..m).collect();
    let mut pivot_cols_local: Vec<usize> = Vec::with_capacity(win.min(m));

    // The safe wrapper enters an `unsafe` block only after `detect`
    // confirmed AVX2 at runtime; the canonical-byte preconditions are
    // upheld by the `from_mont` pack above.
    let rank = (fns.ple_panel_base_fn)(
        &mut window,
        m,
        win,
        p_u8,
        inv_table,
        &mut row_perm,
        &mut pivot_cols_local,
    );

    // The kernel permuted only the window; `row_perm[k]` is the original
    // index of the row now at position `k`.
    apply_row_perm_outside_window::<P>(matrix, parent_cols, m, col_lo, col_hi, &row_perm);

    apply_perm_indices(perm, &row_perm);

    for r in 0..m {
        let row_base = r * parent_cols + col_lo;
        for c in 0..win {
            let canon = window[r * win + c] as usize;
            debug_assert!(canon < to_mont.len());
            matrix[row_base + c] = Fp::<P>::from_raw_storage(to_mont[canon]);
        }
    }

    for off in pivot_cols_local {
        pivot_cols.push(col_lo + off);
    }

    Some(rank)
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_try_ple_panel_base<const P: u64>(
    _matrix: &mut [Fp<P>],
    _parent_cols: usize,
    _m: usize,
    _col_lo: usize,
    _col_hi: usize,
    _perm: &mut [usize],
    _pivot_cols: &mut Vec<usize>,
) -> Option<usize> {
    None
}

/// Non-allocating lane-class probe for [`fp_try_ple_panel_base`].
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_ple_panel_lane<const P: u64>() -> Option<PlePanelLane> {
    if fp_medium_eligible::<P>() {
        return crate::simd::maybe_fp_medium_ple()
            .is_some()
            .then_some(PlePanelLane::U16);
    }
    (fp_small_enabled::<P>() && crate::simd::maybe_fp_small_ple().is_some())
        .then_some(PlePanelLane::Byte)
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_ple_panel_lane<const P: u64>() -> Option<PlePanelLane> {
    None
}

// ---------------------------------------------------------------------------
// Medium-prime PLE base-case dispatch
// ---------------------------------------------------------------------------

/// Per-prime u16 inverse table for medium primes `Fp<P>` (`P ∈ (251,
/// 65536)`). `inv_table[v]` is the modular inverse of `v` for `v ∈
/// [1, P)`; `inv_table[0]` is unused (kept 0).
///
/// Built once per prime per process, cached in a global mutex-guarded map
/// and leaked for the process lifetime (65521 × 2 bytes at most per prime).
#[cfg(feature = "simd")]
#[allow(clippy::explicit_auto_deref)]
fn build_medium_prime_inv_table<const P: u64>() -> &'static [u16] {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    static MEDIUM_INV_TABLES: OnceLock<Mutex<HashMap<u64, &'static [u16]>>> = OnceLock::new();
    let map = MEDIUM_INV_TABLES.get_or_init(|| Mutex::new(HashMap::new()));

    {
        let guard = map.lock().expect("medium inv-table mutex poisoned");
        if let Some(slot) = guard.get(&P) {
            return slot;
        }
    }

    // Computed outside the lock: `P − 1` Fermat exponentiations.
    let p_u64 = P;
    let mut table = vec![0u16; p_u64 as usize];
    for v in 1..p_u64 {
        let mut result: u64 = 1;
        let mut base: u64 = v;
        let mut e: u64 = p_u64 - 2;
        while e > 0 {
            if e & 1 == 1 {
                result = (result * base) % p_u64;
            }
            e >>= 1;
            if e > 0 {
                base = (base * base) % p_u64;
            }
        }
        table[v as usize] = result as u16;
    }
    let leaked: &'static [u16] = Box::leak(table.into_boxed_slice());

    let mut guard = map.lock().expect("medium inv-table mutex poisoned");
    // A racing thread may have inserted first; its table wins and ours
    // stays leaked. The explicit `*` copies the `&'static [u16]` out of the
    // `&mut` that `or_insert` returns, hence the `explicit_auto_deref` allow.
    *guard.entry(P).or_insert(leaked)
}

/// Medium-prime PLE base-case dispatch helper.
///
/// Operates on `Fp<P>` matrices with `P ∈ (251, 65536)`. Packs the
/// column window into canonical u16 storage via `Fp::value()`, invokes
/// the AVX2 u16-lane panel-base kernel via
/// [`crate::simd::maybe_fp_medium_ple`], propagates row swaps to cells
/// outside the column window via cycle decomposition, and unpacks the
/// (already-permuted) window scratch back into Montgomery storage via
/// `Fp::new`.
///
/// Returns `Some(rank)` on success; `None` when the kernel declined
/// (P out of range, simd feature disabled, AVX2 unavailable). The
/// caller falls back to `ple_base_direct` in this case.
#[cfg(feature = "simd")]
pub(crate) fn fp_try_ple_panel_base_medium<const P: u64>(
    matrix: &mut [Fp<P>],
    parent_cols: usize,
    m: usize,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> Option<usize> {
    if !fp_medium_eligible::<P>() {
        return None;
    }
    let fns = crate::simd::maybe_fp_medium_ple()?;
    debug_assert_eq!(
        matrix.len(),
        m * parent_cols,
        "fp_try_ple_panel_base_medium: matrix shape"
    );
    debug_assert_eq!(perm.len(), m, "fp_try_ple_panel_base_medium: perm length");
    debug_assert!(
        col_lo <= col_hi && col_hi <= parent_cols,
        "fp_try_ple_panel_base_medium: col window out of bounds"
    );

    let win = col_hi - col_lo;
    if m == 0 || win == 0 {
        return Some(0);
    }

    let p_u16 = P as u16;
    let inv_table = build_medium_prime_inv_table::<P>();

    let mut window: Vec<u16> = Vec::with_capacity(m * win);
    for r in 0..m {
        let row_base = r * parent_cols + col_lo;
        for c in 0..win {
            let canon = matrix[row_base + c].value() as u16;
            debug_assert!((canon as u64) < P, "non-canonical cell");
            window.push(canon);
        }
    }

    let mut row_perm: Vec<usize> = (0..m).collect();
    let mut pivot_cols_local: Vec<usize> = Vec::with_capacity(win.min(m));

    let rank = (fns.ple_panel_base_fn)(
        &mut window,
        m,
        win,
        p_u16,
        inv_table,
        &mut row_perm,
        &mut pivot_cols_local,
    );

    apply_row_perm_outside_window::<P>(matrix, parent_cols, m, col_lo, col_hi, &row_perm);

    apply_perm_indices(perm, &row_perm);

    for r in 0..m {
        let row_base = r * parent_cols + col_lo;
        for c in 0..win {
            let canon = window[r * win + c] as u64;
            debug_assert!(canon < P, "non-canonical cell post-kernel");
            matrix[row_base + c] = Fp::<P>::new(canon);
        }
    }

    for off in pivot_cols_local {
        pivot_cols.push(col_lo + off);
    }

    Some(rank)
}

#[cfg(not(feature = "simd"))]
#[inline]
#[allow(dead_code)] // No-simd stub; called only from feature-gated code paths.
pub(crate) fn fp_try_ple_panel_base_medium<const P: u64>(
    _matrix: &mut [Fp<P>],
    _parent_cols: usize,
    _m: usize,
    _col_lo: usize,
    _col_hi: usize,
    _perm: &mut [usize],
    _pivot_cols: &mut Vec<usize>,
) -> Option<usize> {
    None
}

/// Rearranges the rows of `matrix` according to `row_perm`, leaving the
/// column window `[col_lo, col_hi)` untouched (the kernel already permuted
/// it in its own scratch buffer). `row_perm[k]` is the original index of
/// the row that now sits at row `k`.
#[cfg(feature = "simd")]
fn apply_row_perm_outside_window<const P: u64>(
    matrix: &mut [Fp<P>],
    parent_cols: usize,
    m: usize,
    col_lo: usize,
    col_hi: usize,
    row_perm: &[usize],
) {
    if col_lo == 0 && col_hi == parent_cols {
        return;
    }
    // `where_now[src] = dst`: the row originally at `src` now lives at `dst`.
    let mut where_now: Vec<usize> = vec![0; m];
    for (dst, &src) in row_perm.iter().enumerate() {
        where_now[src] = dst;
    }

    if where_now.iter().enumerate().all(|(i, &v)| i == v) {
        return;
    }

    let mut visited = vec![false; m];
    for start in 0..m {
        if visited[start] || where_now[start] == start {
            visited[start] = true;
            continue;
        }
        let mut cycle: Vec<usize> = Vec::new();
        let mut cur = start;
        while !visited[cur] {
            cycle.push(cur);
            visited[cur] = true;
            cur = where_now[cur];
        }
        // The row originally at `cycle[i]` now lives at
        // `cycle[(i + 1) % len]`: buffer the cycle's outside-window cells,
        // then redistribute them.
        let outside_len_left = col_lo;
        let outside_len_right = parent_cols - col_hi;
        let outside_total = outside_len_left + outside_len_right;
        if outside_total == 0 {
            continue;
        }
        let mut buf: Vec<Fp<P>> = Vec::with_capacity(cycle.len() * outside_total);
        for &pos in &cycle {
            let row_base = pos * parent_cols;
            for c in 0..col_lo {
                buf.push(matrix[row_base + c]);
            }
            for c in col_hi..parent_cols {
                buf.push(matrix[row_base + c]);
            }
        }
        for i in 0..cycle.len() {
            let dst_pos = cycle[(i + 1) % cycle.len()];
            let dst_row_base = dst_pos * parent_cols;
            let buf_base = i * outside_total;
            matrix[dst_row_base..dst_row_base + col_lo]
                .copy_from_slice(&buf[buf_base..buf_base + col_lo]);
            matrix[dst_row_base + col_hi..dst_row_base + col_hi + outside_len_right]
                .copy_from_slice(
                    &buf[buf_base + outside_len_left
                        ..buf_base + outside_len_left + outside_len_right],
                );
        }
    }
}

/// Composes the caller's permutation tracker `perm` with the kernel's
/// `row_perm`: `perm_new[k] = perm_old[row_perm[k]]`.
#[cfg(feature = "simd")]
fn apply_perm_indices(perm: &mut [usize], row_perm: &[usize]) {
    debug_assert_eq!(perm.len(), row_perm.len());
    let perm_old: Vec<usize> = perm.to_vec();
    for (k, slot) in perm.iter_mut().enumerate() {
        *slot = perm_old[row_perm[k]];
    }
}

#[cfg(not(feature = "simd"))]
#[allow(dead_code)] // No-simd stub; constructed only from feature-gated code paths.
pub(crate) struct PackedFpChainPolys<const P: u64>;

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_try_make_chain_poly_arith<const P: u64>(
    _n: usize,
) -> Option<Box<dyn crate::field::matrix::ChainPolyArith<Fp<P>>>> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_chain_poly_arith_available<const P: u64>() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORD_BOUNDARY_LENS: &[usize] = &[0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257];

    /// Checks that `fp_try_prepack_matvec` + `PackedMatvec::matvec` on a
    /// pre-packed small-prime matrix matches the scalar reference at boundary
    /// lengths for both k and m.
    #[cfg(feature = "simd")]
    fn check_small_prime_prepack_matvec<const P: u64>(lens: &[usize]) {
        if crate::simd::maybe_fp_small().is_none() {
            return; // non-AVX2 host — fast path genuinely unreachable
        }
        for &k in lens {
            for &m in lens {
                if k == 0 || m == 0 {
                    // Zero-dim case: y is either empty (m=0) or all zeros
                    // (k=0 vacuous sum). Verify the prepack matvec matches
                    // the scalar semantics rather than skipping the
                    // boundary.
                    let a: Vec<Fp<P>> = vec![Fp::<P>::new(0); m * k];
                    let x: Vec<Fp<P>> = vec![Fp::<P>::new(0); k];
                    if let Some(packed) = fp_try_prepack_matvec::<P>(&a, m, k) {
                        let mut y_simd = vec![Fp::<P>::new(0); m];
                        packed.matvec(&x, &mut y_simd);
                        for &y_i in &y_simd {
                            assert_eq!(y_i, Fp::<P>::new(0), "P={P} m={m} k={k}");
                        }
                    }
                    continue;
                }
                // Build a deterministic m×k matrix.
                let a: Vec<Fp<P>> = (0..(m * k) as u64)
                    .map(|i| Fp::<P>::new(i.wrapping_mul(1_000_003).wrapping_add(17)))
                    .collect();
                // Build a deterministic x vector of length k.
                let x: Vec<Fp<P>> = (0..k as u64)
                    .map(|i| Fp::<P>::new(i.wrapping_mul(2_654_435_761).wrapping_add(11)))
                    .collect();

                // Scalar reference: y_ref[i] = sum_j a[i*k+j] * x[j]
                let mut y_ref = vec![Fp::<P>::new(0); m];
                for i in 0..m {
                    let mut acc = Fp::<P>::new(0);
                    for j in 0..k {
                        acc += a[i * k + j] * x[j];
                    }
                    y_ref[i] = acc;
                }

                // Pre-packed path.
                let packed = fp_try_prepack_matvec::<P>(&a, m, k)
                    .expect("fp_try_prepack_matvec returned None on AVX2 host for small prime");
                let mut y_simd = vec![Fp::<P>::new(0); m];
                packed.matvec(&x, &mut y_simd);

                for i in 0..m {
                    assert_eq!(
                        y_simd[i], y_ref[i],
                        "prepack matvec mismatch P={P} m={m} k={k} i={i}"
                    );
                }

                // Call matvec a second time with the SAME packed matrix to
                // verify that the scratch-buffer reuse path is correct.
                let x2: Vec<Fp<P>> = (0..k as u64)
                    .map(|i| Fp::<P>::new(i.wrapping_mul(40_503).wrapping_add(7)))
                    .collect();
                let mut y_ref2 = vec![Fp::<P>::new(0); m];
                for i in 0..m {
                    let mut acc = Fp::<P>::new(0);
                    for j in 0..k {
                        acc += a[i * k + j] * x2[j];
                    }
                    y_ref2[i] = acc;
                }
                let mut y_simd2 = vec![Fp::<P>::new(0); m];
                packed.matvec(&x2, &mut y_simd2);
                for i in 0..m {
                    assert_eq!(
                        y_simd2[i], y_ref2[i],
                        "prepack matvec reuse mismatch P={P} m={m} k={k} i={i}"
                    );
                }
            }
        }
    }

    /// Boundary-length scalar equivalence of the small-prime prepack matvec
    /// path for both m and k.
    #[test]
    fn test_small_prime_prepack_matvec_boundary_lengths() {
        #[cfg(not(feature = "simd"))]
        return;

        const BOUNDARY_LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];

        #[cfg(feature = "simd")]
        {
            check_small_prime_prepack_matvec::<251>(BOUNDARY_LENS);
            check_small_prime_prepack_matvec::<7>(BOUNDARY_LENS);
        }
    }

    /// Scalar equivalence of `fp_small_try_gemm_classical` over `lens`,
    /// including the thread-local scratch reuse of a repeated call.
    #[cfg(feature = "simd")]
    fn check_small_prime_gemm_dispatch<const P: u64>(lens: &[usize]) {
        if crate::simd::maybe_fp_small().is_none() {
            return;
        }
        for &m in lens {
            for &k in lens {
                for &n in lens {
                    // Skip zero-dim cells: the dispatch contract is that
                    // `fp_small_try_gemm_classical` returns false for
                    // m==0 || k==0 || n==0 (caller already populated
                    // out with zeros for the m×n zero-matrix case).
                    if m == 0 || k == 0 || n == 0 {
                        let a: Vec<Fp<P>> = vec![Fp::<P>::new(0); m * k];
                        let bt: Vec<Fp<P>> = vec![Fp::<P>::new(0); n * k];
                        let mut out = vec![Fp::<P>::new(0); m * n];
                        let used = fp_small_try_gemm_classical::<P>(&a, &bt, m, k, n, &mut out);
                        assert!(!used, "zero-dim shape must return false");
                        continue;
                    }
                    // Build deterministic matrices in Montgomery storage.
                    let a: Vec<Fp<P>> = (0..(m * k) as u64)
                        .map(|i| Fp::<P>::new(i.wrapping_mul(1_000_003).wrapping_add(17)))
                        .collect();
                    let bt: Vec<Fp<P>> = (0..(n * k) as u64)
                        .map(|i| Fp::<P>::new(i.wrapping_mul(2_654_435_761).wrapping_add(11)))
                        .collect();
                    // Scalar reference: out[i*n+j] = sum_t a[i*k+t] * bt[j*k+t].
                    let mut out_ref = vec![Fp::<P>::new(0); m * n];
                    for i in 0..m {
                        for j in 0..n {
                            let mut acc = Fp::<P>::new(0);
                            for t in 0..k {
                                acc += a[i * k + t] * bt[j * k + t];
                            }
                            out_ref[i * n + j] = acc;
                        }
                    }
                    let mut out_simd = vec![Fp::<P>::new(0); m * n];
                    let used = fp_small_try_gemm_classical::<P>(&a, &bt, m, k, n, &mut out_simd);
                    assert!(
                        used,
                        "fp_small_try_gemm_classical must succeed on AVX2 host"
                    );
                    for i in 0..m {
                        for j in 0..n {
                            assert_eq!(
                                out_simd[i * n + j],
                                out_ref[i * n + j],
                                "gemm dispatch mismatch P={P} m={m} k={k} n={n} i={i} j={j}"
                            );
                        }
                    }
                    // Run a second time with the SAME shape to exercise
                    // the thread-local scratch reuse path.
                    let mut out_simd2 = vec![Fp::<P>::new(0); m * n];
                    let used2 = fp_small_try_gemm_classical::<P>(&a, &bt, m, k, n, &mut out_simd2);
                    assert!(used2);
                    assert_eq!(
                        out_simd, out_simd2,
                        "scratch reuse drift P={P} m={m} k={k} n={n}"
                    );
                }
            }
        }
    }

    /// Boundary-length scalar equivalence of the small-prime GEMM dispatch
    /// for each of m, k, n.
    #[test]
    fn test_small_prime_gemm_dispatch_boundary_lengths() {
        #[cfg(not(feature = "simd"))]
        return;

        const BOUNDARY_LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65, 128, 129];

        #[cfg(feature = "simd")]
        {
            check_small_prime_gemm_dispatch::<7>(BOUNDARY_LENS);
            check_small_prime_gemm_dispatch::<31>(BOUNDARY_LENS);
            check_small_prime_gemm_dispatch::<251>(BOUNDARY_LENS);
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(32))]

        /// Property: `fp_small_try_gemm_classical` for GF(251) at random
        /// `(m, k, n)` boundary shapes matches the scalar GEMM reference
        /// bit-exactly.
        #[test]
        fn proptest_small_prime_gemm_boundary_fp251(
            m_idx in 0usize..10,
            k_idx in 0usize..10,
            n_idx in 0usize..10,
            seed in proptest::prelude::any::<u64>(),
        ) {
            const BOUNDARY_LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65, 128, 129];
            let m = BOUNDARY_LENS[m_idx];
            let k = BOUNDARY_LENS[k_idx];
            let n = BOUNDARY_LENS[n_idx];
            #[cfg(feature = "simd")]
            {
                if crate::simd::maybe_fp_small().is_none() {
                    return Ok(());
                }
                if m == 0 || k == 0 || n == 0 {
                    let a: Vec<Fp<251>> = vec![Fp::<251>::new(0); m * k];
                    let bt: Vec<Fp<251>> = vec![Fp::<251>::new(0); n * k];
                    let mut out = vec![Fp::<251>::new(0); m * n];
                    let used = fp_small_try_gemm_classical::<251>(&a, &bt, m, k, n, &mut out);
                    proptest::prop_assert!(!used);
                    return Ok(());
                }
                let mut s = seed;
                let a: Vec<Fp<251>> = (0..m * k)
                    .map(|_| {
                        s = s.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
                        Fp::<251>::new(s)
                    })
                    .collect();
                let bt: Vec<Fp<251>> = (0..n * k)
                    .map(|_| {
                        s = s.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
                        Fp::<251>::new(s)
                    })
                    .collect();
                let mut out_ref = vec![Fp::<251>::new(0); m * n];
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = Fp::<251>::new(0);
                        for t in 0..k {
                            acc += a[i * k + t] * bt[j * k + t];
                        }
                        out_ref[i * n + j] = acc;
                    }
                }
                let mut out_simd = vec![Fp::<251>::new(0); m * n];
                let used = fp_small_try_gemm_classical::<251>(&a, &bt, m, k, n, &mut out_simd);
                proptest::prop_assert!(used);
                for i in 0..m {
                    for j in 0..n {
                        proptest::prop_assert_eq!(out_simd[i * n + j], out_ref[i * n + j]);
                    }
                }
            }
            let _ = (m, k, n, seed);
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(48))]

        /// Property: the small-prime prepack matvec path returns the same result
        /// as the scalar reference for any `(m, k)` shape with each dimension in
        /// `{0, 1, 15, 16, 17, 63, 64, 65}`.
        #[test]
        fn proptest_small_prime_prepack_matvec_boundary_fp251(
            m_idx in 0usize..8,
            k_idx in 0usize..8,
            seed in proptest::prelude::any::<u64>(),
        ) {
            const BOUNDARY_LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            let m = BOUNDARY_LENS[m_idx];
            let k = BOUNDARY_LENS[k_idx];
            #[cfg(feature = "simd")]
            {
                if crate::simd::maybe_fp_small().is_none() {
                    return Ok(()); // non-AVX2 host — fast path unreachable
                }
                if m == 0 || k == 0 {
                    let a: Vec<Fp<251>> = vec![Fp::<251>::new(0); m * k];
                    let x: Vec<Fp<251>> = vec![Fp::<251>::new(0); k];
                    if let Some(packed) = fp_try_prepack_matvec::<251>(&a, m, k) {
                        let mut y_simd = vec![Fp::<251>::new(0); m];
                        packed.matvec(&x, &mut y_simd);
                        for &y_i in &y_simd {
                            proptest::prop_assert_eq!(y_i, Fp::<251>::new(0));
                        }
                    }
                    return Ok(());
                }
                let mut s = seed;
                let a: Vec<Fp<251>> = (0..m * k)
                    .map(|_| { s = s.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9); Fp::<251>::new(s) })
                    .collect();
                let x: Vec<Fp<251>> = (0..k)
                    .map(|_| { s = s.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9); Fp::<251>::new(s) })
                    .collect();
                let mut y_ref = vec![Fp::<251>::new(0); m];
                for i in 0..m {
                    let mut acc = Fp::<251>::new(0);
                    for j in 0..k { acc += a[i * k + j] * x[j]; }
                    y_ref[i] = acc;
                }
                let packed = fp_try_prepack_matvec::<251>(&a, m, k).unwrap();
                let mut y_simd = vec![Fp::<251>::new(0); m];
                packed.matvec(&x, &mut y_simd);
                for i in 0..m {
                    proptest::prop_assert_eq!(y_simd[i], y_ref[i]);
                }
            }
            let _ = (m, k, seed);
        }
    }

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

        // Small-prime AVX2 byte-lane kernels (P <= 251).
        check_small_prime::<7>();
        check_small_prime::<31>();
        check_small_prime::<251>();
    }

    /// SIMD path matches scalar element-wise across `WORD_BOUNDARY_LENS` for
    /// the small-prime byte-lane dispatch (`P <= 251`).
    fn check_small_prime<const P: u64>() {
        #[cfg(not(feature = "simd"))]
        {
            return;
        }
        #[cfg(feature = "simd")]
        {
            if crate::simd::maybe_fp_small().is_none() {
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
                    <Fp<P> as SimdVecOps>::try_simd_add_vec(&a, &b).expect("small SIMD add");
                let got_sub =
                    <Fp<P> as SimdVecOps>::try_simd_sub_vec(&a, &b).expect("small SIMD sub");
                let got_mul =
                    <Fp<P> as SimdVecOps>::try_simd_mul_vec(&a, &b).expect("small SIMD mul");

                for i in 0..len {
                    assert_eq!(got_add[i], a[i] + b[i], "add P={P}, len={len}, i={i}");
                    assert_eq!(got_sub[i], a[i] - b[i], "sub P={P}, len={len}, i={i}");
                    assert_eq!(got_mul[i], a[i] * b[i], "mul P={P}, len={len}, i={i}");
                }
            }
        }
    }

    fn check_medium_prime<const P: u64>() {
        #[cfg(not(feature = "simd"))]
        {
            return;
        }
        #[cfg(feature = "simd")]
        {
            if crate::simd::maybe_fp_medium().is_none() {
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
                    <Fp<P> as SimdVecOps>::try_simd_add_vec(&a, &b).expect("medium SIMD add");
                let got_sub =
                    <Fp<P> as SimdVecOps>::try_simd_sub_vec(&a, &b).expect("medium SIMD sub");
                let got_mul =
                    <Fp<P> as SimdVecOps>::try_simd_mul_vec(&a, &b).expect("medium SIMD mul");

                for i in 0..len {
                    assert_eq!(got_add[i], a[i] + b[i], "add P={P}, len={len}, i={i}");
                    assert_eq!(got_sub[i], a[i] - b[i], "sub P={P}, len={len}, i={i}");
                    assert_eq!(got_mul[i], a[i] * b[i], "mul P={P}, len={len}, i={i}");
                }

                // Dot product hook: the SIMD path is bit-exact against a
                // canonical scalar reference at the same word-boundary lens.
                let mut scratch_a = Vec::<u16>::new();
                let mut scratch_b = Vec::<u16>::new();
                let got_dot =
                    fp_medium_try_dot_product::<P>(&a, &b, &mut scratch_a, &mut scratch_b);
                if let Some(got_dot) = got_dot {
                    let mut expected = Fp::<P>::new(0);
                    for i in 0..len {
                        expected += a[i] * b[i];
                    }
                    assert_eq!(got_dot, expected, "dot P={P}, len={len}");
                }
            }
        }
    }

    #[test]
    fn medium_simd_matches_scalar_word_boundaries() {
        // The largest prime in the dispatch range.
        check_medium_prime::<65521>();
        // Sweep across the dispatch range (P ∈ (251, 65535]) to verify
        // Barrett reduction generality. GF(257) is the smallest in-range
        // prime; GF(509)/GF(1009)/GF(8191)/GF(32749) span small/large ends.
        check_medium_prime::<257>();
        check_medium_prime::<509>();
        check_medium_prime::<1009>();
        check_medium_prime::<8191>();
        check_medium_prime::<32749>();
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

    /// Guards the dispatch-order invariant: the `if P == M31` branch of
    /// `<Fp<P> as SimdVecOps>::try_simd_mul_vec` is reachable on AVX2 hosts
    /// and, at every word-boundary length, the SIMD-batched Mersenne31
    /// multiply matches the scalar element-wise product bit-exactly.
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
        // Medium primes route to the dedicated u16 Barrett kernel, not the
        // 64-bit generic Montgomery path.
        assert!(!fp_generic_enabled::<65521>());
        assert!(!fp_generic_enabled::<257>());
        assert!(!fp_generic_enabled::<32749>());

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
