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
#[cfg(feature = "simd")]
use crate::field::FiniteField;

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

    /// Attempts a SIMD-accelerated dot product `∑ a[i] · b[i]`; returns
    /// `None` to defer to the scalar `mul_product_sum_wide` chunked
    /// loop in `crate::field::vec::dot_product_slices`.
    ///
    /// The returned value is the canonical reduced sum, equivalent to
    /// the scalar `dot_product_slices` result. Implementors that
    /// vectorise this hook can typically eliminate the
    /// pack/unpack round-trip that
    /// [`Self::try_simd_mul_vec`] + scalar reduction would otherwise
    /// pay, because the kernel reduces the 32-bit-lane accumulator
    /// directly to a scalar at the panel boundary.
    ///
    /// # Arguments
    ///
    /// * `a`, `b` — same-length element slices.
    ///
    /// # Complexity
    ///
    /// `O(n)` base-field MACs; lane-parallel via `_mm256_madd_epi16`
    /// when specialised on the small-prime byte-lane path.
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
/// AVX2 `m31_batch_dot_fn` kernel (issue `6a7d4c8e`).
///
/// Pre-packs both operands into contiguous `u32` buffers once per
/// GEMM call (`O(mk + kn)`), then for each output cell `(i, j)`
/// invokes the batch-dot kernel on the pre-packed row slices
/// (`O(mn)` kernel calls, each `O(k)` work). The pack step uses
/// direct `raw_storage() as u32` truncation — `Fp<M31>` uses
/// canonical storage (`raw_storage()` returns the value in `[0,
/// 2^31-1)`), so no Montgomery REDC round-trip is needed.
///
/// Returns `true` when the kernel populated `out`; `false` when the
/// field is not M31, the `simd` feature is disabled, or AVX2 is
/// unavailable at runtime.
///
/// # Arguments
///
/// * `a`   — `m × k` flattened row-major `Fp<M31>` slice.
/// * `b_t` — `n × k` flattened row-major `Fp<M31>` slice (already
///   transposed by the caller).
/// * `m`, `k`, `n` — matrix dimensions.
/// * `out` — `m × n` output slice; written cell-by-cell.
///
/// # Complexity
///
/// `O(m·k·n)` Mersenne multiplications, with AVX2 vectorisation
/// factor of 8 per lane via `m31_batch_dot_fn`.
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
            // Pack A and B^T into canonical u32 buffers.
            // M31 uses canonical storage: raw_storage() is already in [0, M31).
            a_u32.resize(m * k, 0u32);
            bt_u32.resize(n * k, 0u32);
            for (dst, src) in a_u32.iter_mut().zip(a.iter()) {
                *dst = src.raw_storage() as u32;
            }
            for (dst, src) in bt_u32.iter_mut().zip(b_t.iter()) {
                *dst = src.raw_storage() as u32;
            }

            // For each output cell (i, j), compute the dot product of
            // row i of A against row j of B^T (= column j of B) via
            // the AVX2 m31_batch_dot_fn kernel.
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

/// Non-SIMD stub for `fp_m31_try_gemm_classical`; always returns `false`.
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
///
/// Returns `true` when `P == 2^31 - 1`, the `simd` feature is enabled,
/// and AVX2 was detected at runtime. Used by
/// [`crate::field::matrix::gemm_axpy_into_view`] to decide whether to
/// allocate the contiguous-`A` scratch buffer before dispatching.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_m31_gemm_classical_available<const P: u64>() -> bool {
    P == M31 && crate::simd::maybe_mersenne().is_some()
}

/// Non-SIMD stub for `fp_m31_gemm_classical_available`; always returns `false`.
#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_m31_gemm_classical_available<const P: u64>() -> bool {
    false
}

// ---------------------------------------------------------------------------
// Small-prime (P <= 251) SIMD helpers.
//
// Operates on canonical bytes ([0, P)). For Montgomery-stored primes
// (P <= 251 means use_specialized_storage is false, so storage is
// Montgomery), we round-trip via .value() / Fp::new() at pack/unpack
// boundaries. The pack/unpack cost is O(n) and is amortised against
// the AVX2 16-element-per-iteration multiply / add / sub.
// ---------------------------------------------------------------------------

/// Returns `true` when the small-prime AVX2 byte-lane dispatch handles
/// `P` (odd prime, `3 <= P <= 251`).
///
/// `P = 2` is excluded because the byte-lane Barrett constant assumes
/// `p ≥ 3`; `P = 2` already has its own bitwise-XOR / bitwise-AND fast
/// path through `Fp<2>`'s scalar arithmetic and does not benefit from
/// byte-lane SIMD.
#[cfg(feature = "simd")]
#[inline]
fn fp_small_enabled<const P: u64>() -> bool {
    P >= 3 && P <= 251
}

/// Packs a slice of `Fp<P>` (Montgomery-stored, `P <= 251`) into
/// canonical bytes in `[0, P)`.
#[cfg(feature = "simd")]
#[inline]
fn fp_small_pack<const P: u64>(xs: &[Fp<P>]) -> Vec<u8> {
    debug_assert!(fp_small_enabled::<P>());
    xs.iter().map(|x| x.value() as u8).collect()
}

/// Unpacks a slice of canonical bytes back into `Vec<Fp<P>>` via
/// `Fp::new`, restoring Montgomery storage for non-specialised primes.
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

/// Minimum prime for which Candidate F (f32-FMA cascade) is preferred
/// over Candidate C (AVX2 16-bit Barrett).
///
/// Set to 251 (the value of the highest in-scope small prime) based on the
/// Phase 1 route-selection decision (issue 41096af5, 2026-05-25). Combined
/// with the n-threshold in `select_f32_path` (`n >= 512`), only the cell
/// `P == 251 && n >= 512` reaches the F / route-A path; all other in-scope
/// primes (GF(7), GF(31), GF(127), GF(241)) have `P < 251` and therefore
/// `P >= N_THRESH_PRIME` evaluates to `false`, routing them to Candidate C
/// unchanged. See
/// `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md`
/// for the full side-by-side evidence table and decision-rule application.
///
/// To select F for primes ≥ some threshold, lower this constant (e.g.
/// `N_THRESH_PRIME = 11` would route GF(7) to C and GF(11)+ to F). The
/// dispatch wiring is forward-compatible; amending this constant is the
/// only code change needed when fresh data supports a lower threshold.
#[cfg(feature = "simd")]
const N_THRESH_PRIME: u64 = 251;

/// Per-(P, m, k, n) Candidate-F / route-A selector.
///
/// Returns `true` when the F-path / route-A is the production default for
/// this (prime, size) cell.
///
/// With `N_THRESH_PRIME = 251` combined with the n-threshold (`n >= 512`),
/// only the cell `P == 251 && n >= 512` reaches the F / route-A path:
///
/// * `P == 251`: `251 >= 251` → prime window satisfied.
/// * `n >= 512`: pack-cost overhead amortises at this size (≈ 7% at n=1024
///   vs ≈ 28% at n=256); below this threshold Candidate C wins.
///
/// All other in-scope primes (`P ∈ {7, 31, 127, 241}`) have `P < 251` and
/// therefore `P >= N_THRESH_PRIME` evaluates to `false` → Candidate C.
/// GF(251)/n < 512 is excluded by the `n >= 512` guard → Candidate C.
///
/// GF(251)/n=1024 ratio: 0.683 vs fflas-ffpack on Zen 3, PASS (≥ 0.667).
/// GF(251)/n=256 ratio: 0.547 — pack cost dominates; Candidate C wins.
///
/// See `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md`
/// for the full side-by-side evidence table and decision-rule application.
#[cfg(feature = "simd")]
#[inline]
const fn select_f32_path<const P: u64>(_m: usize, _k: usize, n: usize) -> bool {
    // F-path / route A enabled when prime is in window AND size has
    // amortised the pack cost. With N_THRESH_PRIME = 251, the prime
    // window is exactly {251}; other in-scope primes (≤ 241) stay on
    // Candidate C.
    //
    // GF(251)/n ≥ 512: ratio 0.683 vs fflas-ffpack on Zen 3, PASS.
    // GF(251)/n < 512: pack cost dominates; Candidate C wins.
    // See `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md`.
    P >= N_THRESH_PRIME && P <= 251 && n >= 512
}

// ---------------------------------------------------------------------------
// Route-A dispatch toggle (AtomicBool, issue 68cdf4c8)
// ---------------------------------------------------------------------------

#[cfg(feature = "simd")]
use std::sync::atomic::{AtomicBool, Ordering};

/// Global runtime debug switch for the route-A GF(251) f32/FMA cascade
/// (issue 68cdf4c8). Default `false`; off by default so production
/// dispatch is unchanged. Tests and bench drivers flip this via
/// [`set_route_a_gf251_enabled`] instead of unsafe env-var mutation.
#[cfg(feature = "simd")]
static ROUTE_A_GF251_ENABLED: AtomicBool = AtomicBool::new(false);

/// Sets the runtime debug switch that opts GF(251) GEMM calls into the
/// reworked Candidate F path (vectorized AVX2 Barrett output reduction +
/// lookup-table pack / unpack).
///
/// As of issue 41096af5, route A is the production default for
/// GF(251)/n ≥ 512 via `select_f32_path` — this toggle is now an
/// **explicit override** that additionally forces route A on for
/// GF(251)/n < 512 (cells that the production dispatch routes to
/// Candidate C). With the toggle `false` (default), GF(251)/n ≥ 512 still
/// routes through route A; with the toggle `true`, all GF(251) cells do.
/// Tests and benches that need to exercise route A at small n flip this
/// flag; restore to `false` after to avoid cross-test interference (the
/// flag is a process-wide `AtomicBool`).
///
/// Originally added under issue 68cdf4c8 SC#1 as the "non-default
/// dispatch toggle (cargo feature OR runtime debug switch)" exposing the
/// reworked path "without changing default production behaviour" — that
/// remained accurate until 41096af5 wired route A as the production
/// default for n ≥ 512.
///
/// Scope: only affects `P == 251`; other primes continue to use
/// Candidate C regardless of this flag.
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

/// Returns `true` when `P == 251` and the route-A debug switch is on.
/// See [`set_route_a_gf251_enabled`].
///
/// Scope: GF(251) only. Non-GF(251) primes always return `false`.
#[cfg(feature = "simd")]
#[inline]
fn route_a_gf251_enabled<const P: u64>() -> bool {
    if P != 251 {
        return false;
    }
    ROUTE_A_GF251_ENABLED.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Route-C dispatch toggle (AtomicBool, issue fc182ed5)
// ---------------------------------------------------------------------------

/// Global runtime debug switch for the route-C GF(251) pure-integer
/// Goto/BLIS-style panelized micro-kernel (issue fc182ed5). Default
/// `false`; off by default so production dispatch is unchanged. Tests
/// and bench drivers flip this via [`set_route_c_gf251_enabled`].
///
/// Mechanically identical to [`ROUTE_A_GF251_ENABLED`]: a process-wide
/// `AtomicBool` accessed via a safe setter / `Relaxed` load. The two
/// flags coexist; if both are on for `P == 251`, route A wins (the
/// dispatch checks route A first). Bench drivers toggle one route at
/// a time.
#[cfg(feature = "simd")]
static ROUTE_C_GF251_ENABLED: AtomicBool = AtomicBool::new(false);

/// Sets the runtime debug switch that opts GF(251) GEMM calls into the
/// route-C pure-integer Goto/BLIS-style panelized micro-kernel
/// (`crate::simd::maybe_fp_small_panel`).
///
/// Default is `false` — production dispatch is unaffected. Call with
/// `true` in test or bench code to exercise route C; restore to `false`
/// after the test to avoid cross-test interference (the flag is a
/// process-wide `AtomicBool`).
///
/// This is the dispatch surface the issue's success criterion 1 names as
/// the "non-default dispatch toggle (cargo feature OR runtime debug
/// switch)" exposing the panelized integer path "without changing
/// default production behaviour."
///
/// Scope: only affects `P == 251`; other primes continue to use
/// Candidate C regardless of this flag.
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

/// Returns `true` when `P == 251` and the route-C debug switch is on.
/// See [`set_route_c_gf251_enabled`].
///
/// Scope: GF(251) only. Non-GF(251) primes always return `false`.
#[cfg(feature = "simd")]
#[inline]
fn route_c_gf251_enabled<const P: u64>() -> bool {
    if P != 251 {
        return false;
    }
    ROUTE_C_GF251_ENABLED.load(Ordering::Relaxed)
}

/// Whole-gemm fast path. Pre-packs `a` (`m × k` row-major) and `b_t`
/// (`n × k` row-major, already transposed by the caller) to
/// canonical-byte SoA buffers and runs the AVX2 byte-lane batch-dot
/// kernel for every output cell against the cached packs. Unpacks the
/// output and writes it through `out` (`m × n` row-major).
///
/// **Dispatch policy (updated 2026-05-25, issue 41096af5):** Candidate C
/// (`_mm256_madd_epi16`-based) handles all `p ≤ 251` cells except the new
/// GF(251)/n ≥ 512 production default (route A). The 5-trial criterion sweep
/// over GF(7)–GF(251) at n ∈ {256, 1024} showed C beats F by 5–10 % at
/// every cell except GF(251)/n=1024 where route A clears 1.5× of fflas-ffpack
/// (ratio 0.679 > 0.667). `select_f32_path` returns `true` for `P == 251 &&
/// n >= 512` (the pack-cost amortisation threshold determined by the Phase 1
/// route-selection decision, `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md`);
/// `N_THRESH_PRIME = 251` combined with `n >= 512` routes exactly the cell
/// `P == 251 && n >= 512` through route A; all other in-scope primes have
/// `P < 251` and stay on Candidate C.
///
/// **Route-A dispatch (issues 68cdf4c8 + 41096af5):** route A (reworked
/// Candidate F: `from_mont_f32` lookup-table pack + vectorized AVX2 Barrett
/// output reduction) runs in two cases:
///
/// 1. `route_a_selected` — explicit AtomicBool toggle via
///    [`set_route_a_gf251_enabled`]; opt-in for testing and benches at any n.
/// 2. `f32_selected` — production default for `P == 251 && n >= 512` since
///    `select_f32_path` returns `true` for that cell (issue 41096af5 wire-in).
///    The `&& P == 251` guard in the branch is a defensive belt-and-suspenders
///    check; it is a compile-time const-generic comparison that the compiler
///    optimises out, and it preserves local readability.
///
/// Both cases share the same route-A code block. GF(7) / GF(31) / GF(127) /
/// GF(241) are never affected — `route_a_selected` is scoped to `P == 251`
/// and `select_f32_path` returns `false` for all primes < 251.
///
/// **Route-A toggle (issue 68cdf4c8):** when [`set_route_a_gf251_enabled`]
/// has been called with `true` AND `P == 251`, this function routes through
/// route A for any n (not just n ≥ 512). This preserves backward
/// compatibility with bench drivers that force route A unconditionally.
/// See `dev/active/68cdf4c8-route-a-design.md`.
///
/// **Small-n overhead amortisation (issue 27bb2f75):** for `n ≤ 128` the
/// per-call constants (panel-pack heap allocations + Montgomery REDC on
/// every packed byte) are a measurable fraction of wall time. Profiling
/// at GF(7)/n=64 attributed ~7 µs (≈ 27 % of 26 µs) to the 12 288 REDCs
/// in the A-pack + B^T-pack + output-unpack loops, plus ≈ 1 µs to the
/// three `vec![]` allocations. This path replaces:
///
///  * the per-element `Fp::value()` REDC in the A and B^T pack with a
///    single byte-indexed lookup in the per-prime `from_mont` table
///    (built once per prime per process by `build_small_prime_tables`);
///  * the per-element `Fp::new(byte)` REDC in the output unpack with a
///    single u64 lookup in the per-prime `to_mont` table;
///  * the three per-call `Vec<u8>` allocations with thread-local
///    scratch buffers that grow once to the steady-state shape.
///
/// At n=k=m=64 this collapses ~12 288 REDCs into the same number of
/// L1-resident table lookups (each ≤ 1 byte / 8 bytes; the from_mont
/// table for P ≤ 251 fits in a single cache line and the to_mont table
/// in four).
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
    if !fp_small_enabled::<P>() {
        return false;
    }

    debug_assert_eq!(a.len(), m * k, "fp_small_try_gemm_classical: a shape");
    debug_assert_eq!(b_t.len(), n * k, "fp_small_try_gemm_classical: b_t shape");
    debug_assert_eq!(out.len(), m * n, "fp_small_try_gemm_classical: out shape");

    if k == 0 || m == 0 || n == 0 {
        // The caller already handled the `k == 0` (output is the m×n zero
        // matrix) and the `m == 0 || n == 0` (empty output) shapes; this
        // is a defensive early-exit in case it ever doesn't.
        return false;
    }

    let p_u8 = P as u8;

    // Resolve the small-prime SIMD fns once. The selection F-path uses
    // its own canonical-f32 pack/run/unpack, so it can short-circuit
    // before we touch the byte-lane scratches below; otherwise we walk
    // straight into Candidate C using cached lookup tables and thread-
    // local scratch buffers (issue 27bb2f75).
    let f32_selected = select_f32_path::<P>(m, k, n);
    let route_a_selected = route_a_gf251_enabled::<P>();
    let route_c_selected = route_c_gf251_enabled::<P>();

    if route_a_selected || (f32_selected && P == 251) {
        // Route A (issues 68cdf4c8 + 41096af5): reworked Candidate F for
        // GF(251) with vectorized output reduction and lookup-table pack/unpack.
        //
        // Two entry paths:
        //   (a) `route_a_selected`: explicit AtomicBool toggle via
        //       `set_route_a_gf251_enabled(true)` — opt-in for any n.
        //   (b) `f32_selected && P == 251`: production default; `f32_selected`
        //       is `true` only when `select_f32_path` returns `true`, which
        //       with N_THRESH_PRIME = 251 means `P == 251 && n >= 512`.
        //       The `&& P == 251` guard is belt-and-suspenders (compile-time
        //       const-generic comparison, optimised out by the compiler) and
        //       preserves local readability. GF(7)/GF(31)/GF(127)/GF(241)
        //       and GF(251)/n<512 use Candidate C.
        //
        // See `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md`
        // for the Phase 1 decision table and wire-in rationale.
        if let Some(fns_f32) = crate::simd::maybe_fp_small_f32() {
            let tables = build_small_prime_tables::<P>();
            let from_mont_f32 = tables.from_mont_f32.as_slice();
            let to_mont = tables.to_mont.as_slice();
            return GEMM_SMALL_F32_A_SCRATCH.with_borrow_mut(|a_f32_scratch| {
                GEMM_SMALL_F32_BT_SCRATCH.with_borrow_mut(|bt_f32_scratch| {
                    GEMM_SMALL_OUT_SCRATCH.with_borrow_mut(|out_u8_scratch| {
                        a_f32_scratch.resize(m * k, 0.0);
                        bt_f32_scratch.resize(n * k, 0.0);
                        out_u8_scratch.resize(m * n, 0u8);

                        // Pack via the `from_mont_f32` table: one L1
                        // table load + f32 store per element. Replaces
                        // the `a.iter().map(|x| x.value() as f32)`
                        // chain which does a full Montgomery REDC per
                        // element.
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

                        // Unpack canonical bytes → Montgomery storage
                        // via the `to_mont` table — same fast path as
                        // Candidate C's unpack (no REDC per element).
                        for (slot, &byte) in out.iter_mut().zip(out_u8_scratch.iter()) {
                            let canon = byte as usize;
                            debug_assert!(canon < to_mont.len());
                            *slot = Fp::<P>::from_raw_storage(to_mont[canon]);
                        }
                        true
                    })
                })
            });
        }
        // Route-A requested but kernel detection failed (no FMA3); fall
        // through to Candidate C — the byte-lane kernel is the documented
        // AVX2-only-no-FMA3 fallback.
    }

    if route_c_selected {
        // Route C (issue fc182ed5): pure-integer Goto/BLIS-style
        // panelized micro-kernel for GF(251) with explicit A/B panel
        // packing + KC blocking. The toggle is opt-in via
        // `set_route_c_gf251_enabled(true)`; default production
        // dispatch is unaffected (Candidate C continues to own all
        // `p ≤ 251` cells). See `dev/active/fc182ed5-route-c-design.md`
        // for the panel-dimension derivation (MR × NR × KC = 4 × 24 × 256).
        if let Some(fns_panel) = crate::simd::maybe_fp_small_panel() {
            let tables = build_small_prime_tables::<P>();
            let from_mont = tables.from_mont.as_slice();
            let to_mont = tables.to_mont.as_slice();
            return GEMM_SMALL_A_SCRATCH.with_borrow_mut(|a_u8| {
                GEMM_SMALL_BT_SCRATCH.with_borrow_mut(|bt_u8| {
                    GEMM_SMALL_OUT_SCRATCH.with_borrow_mut(|out_u8| {
                        a_u8.resize(m * k, 0u8);
                        bt_u8.resize(n * k, 0u8);
                        out_u8.resize(m * n, 0u8);

                        // Pack A and B^T canonical bytes via the
                        // `from_mont` table (one L1 lookup per element,
                        // no REDC). Same pre-pack the Candidate C
                        // dispatch uses.
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

                        // Unpack canonical bytes → Montgomery storage
                        // via the `to_mont` table (same fast path as
                        // Candidate C's output unpack — no REDC).
                        for (slot, &byte) in out.iter_mut().zip(out_u8.iter()) {
                            let canon = byte as usize;
                            debug_assert!(canon < to_mont.len());
                            *slot = Fp::<P>::from_raw_storage(to_mont[canon]);
                        }
                        true
                    })
                })
            });
        }
        // Route-C requested but kernel detection failed (no AVX2); fall
        // through to Candidate C below — Candidate C requires AVX2 too,
        // so it will also fall back to scalar dispatch via the
        // `maybe_fp_small` `None` arm. Behaviour is therefore identical
        // to the production no-AVX2 fallback path.
    }

    if f32_selected {
        // Legacy Candidate F (AVX2 + FMA3 f32-cascade) — this block is
        // only reachable when `select_f32_path` returns `true` but the
        // route-A block above was not taken (i.e. `route_a_selected` was
        // false and the `maybe_fp_small_f32()` lookup returned `None` for
        // the route-A path). In practice with N_THRESH_PRIME = 251 the
        // only cell that sets `f32_selected` is `P == 251 && n >= 512`,
        // which is also covered by route A above. This block therefore
        // acts as a fallback for the no-FMA3 edge case. Allocation
        // pattern is unchanged; the F-path body is kept verbatim as the
        // upgrade path.
        if let Some(fns_f32) = crate::simd::maybe_fp_small_f32() {
            let mut out_u8 = vec![0u8; m * n];
            let a_f32: Vec<f32> = a.iter().map(|x| x.value() as f32).collect();
            let bt_f32: Vec<f32> = b_t.iter().map(|x| x.value() as f32).collect();
            (fns_f32.batch_gemm_fn)(&a_f32, &bt_f32, m, k, n, p_u8, &mut out_u8);
            for (slot, &byte) in out.iter_mut().zip(out_u8.iter()) {
                *slot = Fp::<P>::new(byte as u64);
            }
            return true;
        }
        // F-path selected but kernel detection failed (no FMA3); fall
        // through to Candidate C — the byte-lane kernel is the documented
        // AVX2-only-no-FMA3 fallback.
    }

    // Candidate C (AVX2 16-bit-integer Barrett kernel) — primary path
    // for all `p ≤ 251` cells except `P == 251 && n >= 512` which is
    // handled by route A above (N_THRESH_PRIME = 251, n >= 512 guard).
    let Some(fns) = crate::simd::maybe_fp_small() else {
        return false;
    };
    // Per-prime lookup tables. Built at most once per (prime, process);
    // the OnceLock cost is paid the first time a process touches any
    // GEMM for this prime and is amortised forever afterwards.
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

                // Pack A and B^T via the from_mont table. Each entry is
                // one byte read indexed by the Montgomery storage word
                // (in `[0, P)`) — replacing what used to be a `Fp::value()`
                // REDC call per element.
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

                // Run the row-panel kernel for each row of A.
                for i in 0..m {
                    let a_row = &a_u8[i * k..(i + 1) * k];
                    let out_row = &mut out_u8[i * n..(i + 1) * n];
                    (fns.gemm_row_panel_fn)(a_row, bt_u8, k, n, p_u8, out_row);
                }

                // Unpack canonical bytes → Montgomery storage via the
                // to_mont table — replacing what used to be a
                // `Fp::new(byte as u64)` REDC call per element.
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
/// (`P ∈ (251, 65535]`) buffer, sweeps every row of the sparse left
/// matrix through the AVX2 SpMM kernel against the shared `b` pack,
/// and unpacks the output back to `Fp<P>` storage. Returns `false`
/// when `P` is outside the supported range, when the `simd` feature
/// is disabled, or when AVX2 is unavailable; in those cases the
/// caller falls back to the generic Wide-accumulator scatter path
/// in `SparseFieldMatrix::matmat`.
///
/// # Returns
///
/// `true` when the SIMD kernel populated `out`, `false` to fall back.
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
        // Pack b canonical bytes once.
        let b_u8: Vec<u8> = b.iter().map(|x| x.value() as u8).collect();
        // Pack all a_values canonical bytes once.
        let a_vals_u8: Vec<u8> = a_values.iter().map(|x| x.value() as u8).collect();
        let mut out_u8 = vec![0u8; n];
        for r in 0..m {
            let start = a_row_ptr[r];
            let end = a_row_ptr[r + 1];
            // Cleared per-row scratch.
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

// ---------------------------------------------------------------------------
// Fp<P> medium-prime SIMD helpers — `P ∈ (251, 65536)` (`word-fits-in-u16`).
// ---------------------------------------------------------------------------
//
// The kernel operates on **canonical** u16 values. Add/sub are linear in the
// Montgomery storage form (`aR + bR = (a+b)R`), so for those we pack/unpack
// via raw_storage and avoid the REDC round-trip. Multiplication is not
// linear in storage form, so we round-trip through `value()` / `Fp::new` to
// expose canonical residues to the Barrett kernel.
//
// The `P >= 252 && P < 65536` guard mirrors the dispatch in `SimdVecOps`:
// primes `P <= 251` are owned by the dedicated 8-bit small-prime kernel
// (sibling issue `662f7a15`); primes `P >= 65536` route to the 64-bit
// generic Montgomery kernel.

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
    // add/sub are linear in Montgomery storage: `aR + bR = (a+b)R`. Pack
    // raw, run, unpack raw — saves two REDC round-trips per element.
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

/// Crate-internal hook: SIMD batch dot product for `Fp<P>` with
/// `P ∈ (251, 65536)`.
///
/// Returns `Σ a[i] · b[i]` (a `Fp<P>` element), or `None` when the
/// runtime / compile-time prerequisites (AVX2, P-eligibility, simd
/// feature) are not satisfied.
///
/// # Implementation
///
/// Operates entirely on **Montgomery raw storage** to avoid the per-
/// element REDC round-trip that `value()` / `Fp::new` would impose.
/// Storage words are in `[0, P) ⊆ [0, 2^16)`, so packing is a `u64 →
/// u16` truncation. The kernel computes
///
/// ```text
///   total = Σ raw(aᵢ) · raw(bᵢ)   (in u64, exact for n < 2^32)
/// ```
///
/// which is congruent to `R² · Σ aᵢbᵢ (mod P)` because each raw word is
/// the Montgomery image `aᵢR (mod P)` (see `Fp::mul_product_sum_wide`
/// for the full representation proof). Reducing modulo `P` yields a
/// value in `[0, P²)`; one Montgomery REDC then recovers `R · Σ aᵢbᵢ
/// (mod P)`, which is exactly the Montgomery storage form of the dot
/// product. This matches the storage-domain reduction performed by
/// `Fp::reduce_product_sum_wide` for the scalar path, so the SIMD and
/// scalar dots are bit-for-bit equivalent.
///
/// This is the hot path that `crate::field::vec::dot_product_slices`
/// consults for medium primes; the GEMM kernel calls it once per output
/// cell.
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

    // Pack Montgomery raw storage into the caller-owned scratches.
    // Storage is already in [0, P), and P < 2^16, so the u64→u16
    // truncation is exact. Reusing the scratches across the surrounding
    // GEMM traversal is the difference between this path beating the
    // scalar `mul_product_sum_wide` loop and merely matching it.
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

    // batch_dot_fn returns a canonical-domain reduction
    // `total % P ≡ R² · Σ aᵢbᵢ (mod P)`. We need the Montgomery storage
    // form, so apply one REDC: `redc(R² · Σ aᵢbᵢ) = R · Σ aᵢbᵢ (mod P)`.
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

/// GEMM helper: pack a slice of `Fp<P>` Montgomery raw storage as `Vec<u16>`
/// when the medium-prime fast path is eligible. Returns `Some(())` on
/// success; `None` if the field is not eligible (so the caller skips the
/// medium-prime fast path entirely).
///
/// The pack pushes raw storage truncated to u16. See
/// [`fp_medium_try_dot_packed`] for the dot kernel that consumes the
/// packed slices and applies the final REDC.
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

/// GEMM helper: SIMD dot product on pre-packed u16 raw-storage slices for
/// medium-prime `Fp<P>`. Mirrors [`fp_medium_try_dot_product`] but skips
/// the per-call pack so the GEMM kernel pays the truncation cost once
/// per matrix instead of once per output cell.
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

/// GEMM helper: whole-GEMM panelized AVX2 kernel for medium-prime
/// `Fp<P>` with `P ∈ (251, 65535]`. Pre-packs both operands as
/// Montgomery raw u16 once per gemm call (`O(mk + kn)`), runs the
/// AVX2 panel kernel, then applies one Montgomery REDC per output
/// cell (`O(mn)` REDCs vs `O(mn)` `% p` reductions; the difference
/// is paid once at output time, identical asymptotic cost).
///
/// The panel kernel computes `c_canonical[i, j] = (Σ a_pack[i,t] *
/// b_pack[j,t]) mod p`, which when inputs carry Montgomery raw
/// storage works out to `R² · Σ a_canonical b_canonical mod p`. The
/// per-cell REDC then maps `R² · x → R · x = Mont(x)`.
///
/// Returns `true` when the kernel ran (and `out` is populated);
/// `false` when the field is out of range, the `simd` feature is
/// disabled, or AVX2 detection failed.
///
/// # Issue
///
/// jit:74ba1cdc R1 — replaces the per-cell `fp_medium_try_dot_packed`
/// dispatch in the GEMM caller (16M calls at n=4096) with a single
/// panel kernel call, closing the GF(65521)/n=4096 ratio gap.
#[cfg(feature = "simd")]
pub(crate) fn fp_medium_try_gemm_panel<const P: u64>(
    a: &[Fp<P>],
    b_t: &[Fp<P>],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [Fp<P>],
) -> bool {
    if !fp_medium_eligible::<P>() {
        return false;
    }
    if m == 0 || k == 0 || n == 0 {
        return false;
    }
    debug_assert_eq!(a.len(), m * k, "fp_medium_try_gemm_panel: a shape");
    debug_assert_eq!(b_t.len(), n * k, "fp_medium_try_gemm_panel: b_t shape");
    debug_assert_eq!(out.len(), m * n, "fp_medium_try_gemm_panel: out shape");

    let Some(fns) = crate::simd::maybe_fp_medium() else {
        return false;
    };

    // Pack A and B^T as Montgomery raw u16 (pure u64 → u16 truncation,
    // no REDC per element — same trick `fp_medium_try_dot_packed` uses).
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

                // Each `out_u16[i*n + j]` holds `(R² · Σ a_canon · b_canon) mod P`.
                // One Montgomery REDC maps that to the canonical Montgomery
                // storage `R · Σ a · b mod P`, matching the storage domain
                // the caller expects.
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
// Packed matvec entry points — issue d1dd266c
// ---------------------------------------------------------------------------
//
// Reuses the existing AVX2 small-prime byte-lane and medium-prime u16-lane
// kernels to compute `y = A · x` without forcing the per-cell scalar
// `mul_product_sum_wide` chain. Two flavours:
//
// - One-shot per-call entry point `fp_try_matvec` — packs `A` and `x`,
//   runs the kernel, unpacks `out`. Used by `FieldMatrix::matvec` for the
//   case where the caller does a single matvec at a time.
// - Pre-packed `PackedFpMatvec` cache — packs `A` once and reuses the
//   pack across many matvec calls. Used by `cyclic_decomposition` and
//   `wiedemann_minpoly_attempt` so each minpoly call pays the matrix
//   pack cost exactly once.

// Thread-local scratch buffers reused across repeated `matvec_packed`
// calls on the same thread (issue 70766cb1). Avoids the per-call heap
// allocation of `x_u8` and `out_u8` in the `Small` hot path.
//
// The buffers grow as needed (`resize` with a capacity check) and are
// never shrunk, so after the first `n`-sized call on a given thread the
// allocator is not consulted again.
#[cfg(feature = "simd")]
thread_local! {
    static SMALL_X_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static SMALL_OUT_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    // GEMM-specific scratch buffers (issue 27bb2f75). The small-n GEMM
    // dispatch path packs A, B^T, and the canonical-byte output into
    // three separate buffers; reusing thread-local Vecs avoids three
    // heap allocations per gemm call. For the GF(7)/GF(31)/n=64 target
    // cell these allocations total ~12 KB and the alloc/free overhead
    // is a measurable fraction of the ~26 µs wall time.
    static GEMM_SMALL_A_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_BT_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_OUT_SCRATCH: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
    // Route-A GEMM scratch (issue 68cdf4c8). Pre-packs `a` and `bt`
    // into f32 buffers via the `from_mont_f32` table lookup; the output
    // is written to a u8 buffer first then unpacked through `to_mont`.
    // Buffers grow as needed and are reused across repeated GEMM calls
    // on the same thread.
    static GEMM_SMALL_F32_A_SCRATCH: std::cell::RefCell<Vec<f32>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_SMALL_F32_BT_SCRATCH: std::cell::RefCell<Vec<f32>> =
        const { std::cell::RefCell::new(Vec::new()) };
    // Medium-prime GEMM scratch (issue 74ba1cdc R1). Pre-packs `a`
    // and `bt` into u16 buffers via raw-storage truncation; the output
    // is written to a u16 buffer first then unpacked through REDC into
    // the caller's `Fp<P>` slot. Buffers grow as needed and are reused
    // across repeated GEMM calls on the same thread.
    static GEMM_MEDIUM_A_SCRATCH: std::cell::RefCell<Vec<u16>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_MEDIUM_BT_SCRATCH: std::cell::RefCell<Vec<u16>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_MEDIUM_OUT_SCRATCH: std::cell::RefCell<Vec<u16>> =
        const { std::cell::RefCell::new(Vec::new()) };
    // Mersenne-31 GEMM scratch (issue 6a7d4c8e). Pre-packs `a` and `bt`
    // into u32 canonical buffers (direct `raw_storage() as u32` — M31
    // uses canonical storage, no REDC needed). The output is a u32 buffer
    // written by `m31_batch_dot_fn`; unpacked back into `Fp<M31>` by
    // direct `from_raw_storage`. Buffers grow as needed and are reused
    // across repeated GEMM calls on the same thread.
    static GEMM_M31_A_SCRATCH: std::cell::RefCell<Vec<u32>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GEMM_M31_BT_SCRATCH: std::cell::RefCell<Vec<u32>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Pre-prime Barrett-constant cache for the small-prime row-panel matvec
/// (issue 70766cb1). Stores per-prime lookup tables for converting
/// Montgomery-stored `Fp<P>` values to and from canonical bytes in O(1)
/// table-lookup rather than O(1) REDC arithmetic. At P ≤ 251 the full
/// table has ≤ 251 bytes and fits in a single cache line.
///
/// `from_mont_table[raw]` — canonical value for a Montgomery word `raw`
/// in `[0, P)`. This is the inverse of `to_mont` and replaces the
/// per-element `from_mont` REDC call in the x-pack loop.
///
/// `to_mont_table[canon]` — Montgomery word for a canonical value `canon`
/// in `[0, P)`. This replaces the per-element `to_mont` call in the
/// output-unpack loop.
#[cfg(feature = "simd")]
pub(crate) struct SmallPrimeTables {
    from_mont: Vec<u8>, // index = raw storage word (in [0, P)); value = canonical
    to_mont: Vec<u64>,  // index = canonical value (in [0, P)); value = raw storage
    /// 16-bit Barrett constant `μ = ⌊2¹⁶ / P⌋` (issue 52cce970 R1).
    ///
    /// Cached here so callers into `fp_small`'s `sub_scaled` /
    /// `batch_mul` / `batch_sub` kernels can pass `μ` as a kernel
    /// argument and skip the 22-25 cycle integer `div esi` that the
    /// kernel prologue otherwise emits once per call. At ~32 000
    /// invocations per GF(251)/n=256 charpoly call this hoist removes
    /// roughly 190 µs of wall time (7-8 %).
    barrett_mu: u16,
    /// `from_mont_f32[raw]` — canonical value as `f32` for a Montgomery
    /// word `raw` in `[0, P)`. Used by the route-A f32 cascade dispatch
    /// (issue 68cdf4c8) to replace the per-element
    /// `a.iter().map(|x| x.value() as f32)` REDC pack with a single
    /// L1-resident table load. At `P ≤ 251` the table is 251 × 4 = 1004
    /// bytes, fitting in 16 cache lines.
    from_mont_f32: Vec<f32>,
    /// `inv_table[v]` — modular inverse of `v` in canonical-byte form,
    /// for `v ∈ [1, P)`. `inv_table[0]` is unused (kept 0). Used by the
    /// panelized PLE base-case kernel (issue 6823c8a0) to look up the
    /// pivot inverse in one L1 load instead of a per-pivot Fermat
    /// exponentiation. The table is `P` bytes (≤ 251), trivially L1.
    inv_table: Vec<u8>,
}

// Global per-prime table cache for small primes (P ≤ 251).
// 256 slots, one per possible prime value; each slot is a OnceLock so
// the table is built at most once per prime per process lifetime.
// A static array avoids the shared-static problem (statics inside generic
// functions are not per-monomorphization in Rust — they are shared across
// all instantiations of the same generic). Using a global array indexed by
// prime value P gives one independent OnceLock per prime.
#[cfg(feature = "simd")]
static SMALL_PRIME_TABLE_SLOTS: [std::sync::OnceLock<SmallPrimeTables>; 256] = {
    // const initialisation: all 256 slots start as uninitialised OnceLocks.
    [const { std::sync::OnceLock::new() }; 256]
};

/// Returns a reference to the per-prime lookup tables for `Fp<P>` with
/// `P ≤ 251`. The tables are built at most once per prime per process and
/// then cached in a global array slot.
///
/// Cost: `O(P)` REDC calls on first access; `O(1)` atomic load on all
/// subsequent accesses.
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
        // Modular inverse table (issue 6823c8a0): inv_table[v] = v^{P-2}
        // mod P for v ∈ [1, P). One Fermat exponentiation per prime
        // value during table init; `O(P log P)` total, paid once per
        // prime per process.
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
///
/// Created once per `cyclic_decomposition` / `wiedemann_minpoly_attempt`
/// call and reused across the `O(n)` matvec sequence steps.
///
/// # Performance notes (issue 70766cb1)
///
/// The `Small` variant caches:
/// 1. `fns: SmallPrimeFns` (avoids the per-call `OnceLock` read)
/// 2. `tables: SmallPrimeTables` (replaces per-element REDC with O(1)
///    table lookup in the x-pack and output-unpack loops)
/// 3. Thread-local scratch buffers for `x_u8` and `out_u8`
///
/// At n = k = 64 (the GF(251)/n=64 target cell), (2) removes ~640 ns of
/// Montgomery-REDC overhead from every matvec call.
#[cfg(feature = "simd")]
pub(crate) enum PackedFpMatrix<const P: u64> {
    /// Small-prime layout — canonical bytes, length `m · k`.
    Small {
        data: Vec<u8>,
        m: usize,
        k: usize,
        /// Cached function-pointer table.
        fns: gf2_kernels_simd::fp_small::SmallPrimeFns,
        /// Per-prime lookup tables (static — built once per prime per process).
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
            // Use the from_mont table for the initial matrix pack so it's
            // consistent with subsequent matvec calls (same fast path).
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
    /// For `P ≤ 251` uses the AVX2 `gemm_row_panel_fn` kernel
    /// (which loads each 16-byte block of `x` once against four
    /// rows of `A` simultaneously, amortising the AVX2
    /// lane-broadcast and constant-table overhead across four
    /// output cells per inner pass). For medium primes
    /// (`252 ≤ P < 65536`) the dot kernel is called per row;
    /// extending the medium-prime kernel to a row-panel matvec is
    /// future work.
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

                // Use thread-local scratch buffers to avoid heap allocation
                // on every call (issue 70766cb1). `resize` extends only when
                // the buffer is shorter, so on steady-state calls (same k, m)
                // the allocator is not consulted.
                SMALL_X_SCRATCH.with_borrow_mut(|x_u8| {
                    x_u8.resize(*k, 0u8);
                    // Pack x using the pre-built from_mont lookup table
                    // (O(1) table lookup per element vs O(1) REDC per element).
                    // For GF(251) this replaces 64 REDC operations with 64
                    // byte-indexed table reads — typically ~2-3x faster.
                    for (dst, v) in x_u8.iter_mut().zip(x.iter()) {
                        *dst = tables.from_mont[v.raw_storage() as usize];
                    }
                    SMALL_OUT_SCRATCH.with_borrow_mut(|out_u8| {
                        out_u8.resize(*m, 0u8);
                        // Use the row-panel gemm kernel: y[j] = sum_t x[t] * A[j*k+t].
                        // `fns` is cached at construction time — no OnceLock read here.
                        (fns.gemm_row_panel_fn)(&x_u8[..*k], data, *k, *m, p_u8, &mut out_u8[..*m]);
                        // Unpack using the pre-built to_mont lookup table instead
                        // of calling Fp::new (which invokes to_mont REDC).
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
                // Pack x storage-domain once per matvec call.
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
pub(crate) struct PackedFpMatrix<const P: u64>;

#[cfg(not(feature = "simd"))]
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
/// when the field is out of range or the kernel is unavailable.
///
/// For repeated matvec calls on the same `a` (e.g. inside Wiedemann
/// or `cyclic_decomposition`), use [`PackedFpMatrix`] instead so the
/// per-row pack cost is paid exactly once.
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
        // y = A · 0-length x is the zero vector. Caller's responsibility
        // to populate `out` with zeros if needed; the kernel path skips.
        return false;
    }
    let Some(packed) = PackedFpMatrix::<P>::try_pack(a, m, k) else {
        return false;
    };
    packed.matvec_packed(x, out);
    true
}

/// SIMD-accelerated axpy (`y[i] += a · x[i]`) for `Fp<P>` with
/// `P ≤ 65521`. Routes through the AVX2 byte-lane (`P ≤ 251`) or
/// u16-lane (`252 ≤ P < 65536`) `batch_mul` + `batch_add` kernels
/// against a broadcast of the scalar `a`. Returns `true` when the
/// kernel populated `y`, `false` to defer to the caller's scalar
/// zip-loop.
///
/// # Algorithm
///
/// 1. Pack `y`, `x`, and the broadcast `[a; n]` to canonical bytes
///    (`P ≤ 251`) or storage-domain `u16`s (`252 ≤ P < 65536`).
/// 2. `tmp = batch_mul(broadcast, x)`.
/// 3. `y_packed = batch_add(y_packed, tmp)`.
/// 4. Unpack `y_packed` back into `y`.
///
/// The pack/unpack cost is `O(n)`; it is amortised against the
/// `O(n)` SIMD inner work but adds a constant factor versus the
/// scalar Montgomery path. The win comes from callers that do many
/// axpys on the SAME `y` (the [`cyclic_decomposition`] reduce loop
/// performs `O(basis_size)` axpys per chain step), where the SIMD
/// throughput dominates the per-call pack/unpack.
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
// Packed cyclic-decomposition basis cache (issue d1dd266c)
// ---------------------------------------------------------------------------

/// Cached canonical-form basis used by the cyclic-decomposition
/// reduce loop. Each pivot column is stored once in canonical form
/// (`P ≤ 251`: bytes; `252 ≤ P < 65536`: u16) and reused across all
/// reduce calls. With this cache, the inner reduce loop runs as
/// `factor_compute → broadcast → batch_mul → batch_sub` per pivot,
/// avoiding the per-element Montgomery REDC overhead of the scalar
/// `axpy` path.
#[cfg(feature = "simd")]
pub(crate) enum PackedFpBasis<const P: u64> {
    Small {
        cols: Vec<Vec<u8>>,
        /// Pre-computed pivot inverses, one per column, indexed in lockstep
        /// with `cols`. `pivot_inv[j] = col_j[pivot_row_j]^{-1} (mod P)`,
        /// canonical byte form. Hoists the Fermat-style `Fp::inv` out of
        /// `fp_reduce_packed`'s inner loop (issue 52cce970); profiling
        /// at GF(251)/n=256 showed `Fp::inv` consumed ~12 % of charpoly
        /// wall time before this hoist.
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
/// canonical form (bytes for `P ≤ 251`, u16 for `252 ≤ P < 65536`),
/// avoiding the Montgomery REDC chain of the scalar
/// [`crate::field::vec::FieldVec::axpy`] path.
///
/// Returns the residual as a `FieldVec<Fp<P>>` (re-packed to
/// Montgomery storage) and the coefficient vector.
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
            // Use the per-prime from_mont / to_mont lookup tables (issue
            // 27bb2f75 / 70766cb1 precedent) so packing/unpacking is a
            // byte-indexed table read rather than a Montgomery REDC per
            // element. At n=256 this removes 2 · 256 = 512 REDC calls per
            // `do_reduce` invocation; over the ~n calls per charpoly this
            // is ~131 k REDCs eliminated.
            let tables = build_small_prime_tables::<P>();
            let from_mont = tables.from_mont.as_slice();
            let to_mont = tables.to_mont.as_slice();
            // Per-prime Barrett constant μ = ⌊2¹⁶ / P⌋ (issue 52cce970 R1):
            // hoisted out of the kernel so the per-call `div esi` prologue
            // is replaced by a single broadcast load. At ~32 k sub_scaled
            // calls per GF(251)/n=256 charpoly the hoist saves ~190 µs.
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
                // factor = v_at_r * pivot_inv mod P (canonical scalar mul).
                // Pivot inverses are pre-computed once per column at `push_col`
                // time (issue 52cce970): profiling showed Fermat-style
                // `Fp::inv` consumed ~12 % of charpoly wall time when called
                // here, repeatedly, with the same column. Hoisting trims
                // that overhead entirely.
                let factor = ((v_at_r as u32 * pivot_inv[j] as u32) % P as u32) as u8;
                // Fused in-place `residual := (residual − factor · col) mod p`
                // (issue 52cce970): replaces the prior `batch_mul(bcast, col)
                // → tmp; batch_sub(residual, tmp) → new_residual; swap` triple
                // pass. Eliminates one broadcast-fill, one intermediate Vec,
                // one new_residual Vec, and one swap; keeps factor, μ, p in
                // ymm registers across the column sweep.
                (fns.sub_scaled_fn)(&mut residual, col, factor, p_u8, barrett_mu);
                // Coeff value is the canonical `factor` byte — store the
                // matching Montgomery storage word via the `to_mont` table
                // (one byte-indexed lookup; no REDC).
                coeffs[j] = Fp::<P>::from_raw_storage(to_mont[factor as usize]);
            }
            // Unpack the canonical-byte residual back to Fp<P> Montgomery
            // storage via the same per-prime `to_mont` table.
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
                // Pivot inverses pre-computed once per column at `push_col`
                // time (issue 52cce970); see Small branch for rationale.
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
        // Find the first non-zero entry to serve as pivot. The basis
        // invariant guarantees at least one exists; if not, panic.
        let pivot_row = col
            .iter()
            .position(|v| !v.is_zero())
            .expect("PackedFpBasis::push_col: column must have a non-zero entry");
        self.push(col, pivot_row);
    }

    /// Optimised override: callers that already hold `pivot_row` (all
    /// hot paths in `cyclic_decomposition`) call this variant to skip
    /// the linear pivot-scan of the default implementation.
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
pub(crate) struct PackedFpBasis<const P: u64>;

#[cfg(not(feature = "simd"))]
impl<const P: u64> PackedFpBasis<P> {
    pub(crate) fn try_new(_n: usize) -> Option<Self> {
        None
    }
    pub(crate) fn push(&mut self, _col: &[Fp<P>]) {}
}

#[cfg(not(feature = "simd"))]
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
/// fields without a SIMD fast path. The boxed handle's `matvec` method
/// runs the full AVX2 kernel against the pre-packed buffer for every
/// call, paying the matrix-pack cost exactly once.
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
// for `cyclic_decomposition` (issue `5a3dbd5b`).
//
// Each chain polynomial of degree `d` is stored as a `Vec<u8>` of length
// `d + 1` in ascending-degree order (coeffs[i] = coeff of x^i), with all
// entries in `[0, P)`.  The Krylov-step update
//
//     next[d] = x · chain[d-1]  −  Σ_j α_j · chain[j]
//
// is therefore:
//   1. shift_x: prepend a zero byte → length grows by 1.
//   2. For each j where α_j ≠ 0:
//        broadcast(α_j) · chain[j] → tmp   (batch_mul, zero-padded)
//        next − tmp → next                 (batch_sub)
//
// All arithmetic stays in canonical-byte form. `alpha`'s canonical byte
// is obtained from a per-prime `from_mont` lookup table (built once via
// `build_small_prime_tables::<P>()`) — no Montgomery REDC inside the
// `sub_scaled_into` hot loop. The only REDC remaining on this path is
// at the very end, when `finish_buf` converts bytes back to
// `FieldPoly<Fp<P>>` via `Fp::new`.
// ---------------------------------------------------------------------------

/// Packed canonical-byte chain-polynomial store for small primes (`P ≤ 251`).
///
/// Used by `cyclic_decomposition` (issue `5a3dbd5b`) to replace the scalar
/// `FieldPoly::mul_scalar` / `Sub` polynomial-bookkeeping with AVX2
/// byte-lane kernels, closing the ~10x wall-clock gap on `GF(251)/n=256
/// charpoly` reported in `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md`
/// § 6.4.
///
/// As of issue `52cce970`, `sub_scaled_into` calls a single fused AVX2
/// kernel (`fns.sub_scaled_fn`, semantics `buf := (buf − α·chain_j) mod p`)
/// instead of the two-step `batch_mul` + `batch_sub` sequence that the
/// `5a3dbd5b` implementation used. The fused path eliminates the
/// per-call scratch broadcast-fill, the intermediate-product write,
/// and the copy-back step.
///
/// # Examples
///
/// ```
/// // PackedFpChainPolys is a crate-internal type; external callers interact
/// // only through the `ChainPolyArith` trait returned by
/// // `FiniteField::try_make_chain_poly_arith`.
/// ```
///
/// # Complexity
///
/// Each `sub_scaled_into` call costs `O(d)` byte-lane AVX2 muls + subs
/// (where `d` is the current chain length), matching the scalar complexity
/// but with a 16-element SIMD factor.  The total polynomial-bookkeeping cost
/// for one Krylov block of length `d` is `O(d²)` — the same as the scalar
/// path but with the Montgomery REDC per-element overhead eliminated.
#[cfg(feature = "simd")]
pub(crate) struct PackedFpChainPolys<const P: u64> {
    /// Stored coefficients for each chain polynomial, in canonical bytes,
    /// ascending-degree order.  `polys[j]` has length `j + 1` (degree `j`).
    polys: Vec<Vec<u8>>,
    /// Per-prime conversion tables (issue 5a3dbd5b R5 review feedback):
    /// `from_mont[raw]` maps a Montgomery storage word to its canonical
    /// byte. Used in `sub_scaled_into` so `alpha`'s canonical value is
    /// obtained via a single table lookup rather than a per-call REDC.
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
        // The constant polynomial 1 has coefficients [1] (degree 0).
        self.polys.push(vec![1u8]);
    }

    fn shift_x_last_into(&self, buf: &mut Vec<u8>) {
        // x · p(x) prepends a zero coefficient.
        let last = self.polys.last().expect("shift_x_last_into: empty chain");
        let new_len = last.len() + 1;
        buf.resize(new_len, 0u8);
        // Copy last[0..] into buf[1..] (shift by one position).
        buf[1..new_len].copy_from_slice(last);
        buf[0] = 0;
    }

    fn sub_scaled_into(&mut self, buf: &mut Vec<u8>, alpha: &Fp<P>, j: usize) {
        // Convert alpha from Montgomery to canonical via the per-prime
        // lookup table built once per prime in
        // `build_small_prime_tables::<P>()` (issue 5a3dbd5b R5 review
        // feedback). Single byte read; no REDC inside the hot loop.
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
        // Single fused in-place kernel call: buf[..cj_len] := (buf − α · chain_j) mod p.
        // Replaces the prior `tmp = batch_mul(α, chain_j); buf = batch_sub(buf, tmp)`
        // two-call sequence (issue 52cce970): the fused kernel keeps α, μ, p
        // in ymm registers, threads the intermediate product through
        // registers only, and writes results back in a single pass.
        //
        // μ is precomputed in `build_small_prime_tables::<P>()` (issue
        // 52cce970 R1) and passed in to skip the kernel's per-call `div`.
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

/// Non-allocating mirror of [`fp_try_make_chain_poly_arith`]:
/// returns `true` exactly when the SIMD chain-poly path is available
/// (`P ≤ 251` and AVX2 detected at runtime), without constructing the
/// boxed handle.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_chain_poly_arith_available<const P: u64>() -> bool {
    fp_small_enabled::<P>() && crate::simd::maybe_fp_small().is_some()
}

/// Non-allocating availability probe for
/// [`fp_small_try_gemm_classical`] (issue `40195c09`).
///
/// Returns `true` exactly when the small-prime whole-GEMM kernel
/// would populate `out` for any compatible shape: `P` in the byte-lane
/// range (`3..=251`) AND a SIMD kernel was detected at runtime (either
/// Candidate C `maybe_fp_small`, route A `maybe_fp_small_f32`, or route
/// C `maybe_fp_small_panel`). Used by
/// [`crate::field::matrix::gemm_axpy_into_view`] to skip the
/// contiguous-`A` scratch allocation when the kernel would decline.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_small_gemm_classical_available<const P: u64>() -> bool {
    if fp_small_enabled::<P>() {
        // Any of the three small-prime kernels can take the call. The
        // dispatch order inside `fp_small_try_gemm_classical` is route
        // A (f32), route C (panel), then Candidate C (byte-lane).
        // Returning `true` whenever any of them is available exactly
        // mirrors the "kernel will succeed" condition.
        return crate::simd::maybe_fp_small().is_some()
            || crate::simd::maybe_fp_small_f32().is_some()
            || crate::simd::maybe_fp_small_panel().is_some();
    }
    // Medium-prime panel kernel (issue 74ba1cdc R1): `Fp<P>` with
    // `P ∈ (251, 65535]` routes through `fp_medium_try_gemm_panel`
    // inside `try_simd_gemm_classical`.
    if fp_medium_eligible::<P>() {
        return crate::simd::maybe_fp_medium().is_some();
    }
    false
}

/// Non-SIMD stub that always returns `false`.
#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_small_gemm_classical_available<const P: u64>() -> bool {
    false
}

/// Panelized PLE base-case fast path for `Fp<P>` with `P <= 251`
/// (issue `6823c8a0`, design `2e8c5a29`).
///
/// Operates on the column window `[col_lo, col_hi)` of the parent
/// row-major matrix storage:
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
/// (e.g. `P > 251`, the `simd` feature disabled, AVX2 unavailable at
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

    // Pack the window into canonical bytes: scratch[r * win + c] =
    // canonical(matrix[r, col_lo + c]).
    let mut window: Vec<u8> = Vec::with_capacity(m * win);
    for r in 0..m {
        let row_base = r * parent_cols + col_lo;
        for c in 0..win {
            let raw = matrix[row_base + c].raw_storage() as usize;
            debug_assert!(raw < from_mont.len());
            window.push(from_mont[raw]);
        }
    }

    // Initialise local row perm tracker.
    let mut row_perm: Vec<usize> = (0..m).collect();
    let mut pivot_cols_local: Vec<usize> = Vec::with_capacity(win.min(m));

    // Invoke the kernel via the safe wrapper. The wrapper internally
    // enters an `unsafe` block only after `detect` confirmed AVX2 at
    // runtime; the canonical-byte preconditions are upheld by the
    // `from_mont` pack above.
    let rank = (fns.ple_panel_base_fn)(
        &mut window,
        m,
        win,
        p_u8,
        inv_table,
        &mut row_perm,
        &mut pivot_cols_local,
    );

    // Propagate the kernel's row swaps to cells **outside** the column
    // window (the kernel only touched the panel bytes; cells in
    // `[0, col_lo)` and `[col_hi, parent_cols)` of each row still
    // reflect the pre-call row order).
    //
    // `row_perm[k] = original_row_index` means the row that originally
    // sat at `original_row_index` now sits at position `k`. We need to
    // physically rearrange the parent matrix rows outside the window
    // so the post-call matrix has consistent row order across all
    // columns. We use the cycle decomposition of `row_perm` to do the
    // outside-window swaps in-place without an extra allocation.
    apply_row_perm_outside_window::<P>(matrix, parent_cols, m, col_lo, col_hi, &row_perm);

    // Apply the same permutation to the caller's `perm` (full-matrix
    // permutation tracker).
    apply_perm_indices(perm, &row_perm);

    // Unpack the (already permuted) window scratch back into Montgomery
    // storage. After `apply_row_perm_outside_window`, the parent
    // matrix's rows are now in the post-PLE order outside the window;
    // we write `window[k * win + c]` into row `k`'s slice at
    // [col_lo, col_hi).
    for r in 0..m {
        let row_base = r * parent_cols + col_lo;
        for c in 0..win {
            let canon = window[r * win + c] as usize;
            debug_assert!(canon < to_mont.len());
            matrix[row_base + c] = Fp::<P>::from_raw_storage(to_mont[canon]);
        }
    }

    // Push panel-relative pivot column offsets as absolute column
    // indices (offset by `col_lo`).
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

/// Non-allocating availability probe for [`fp_try_ple_panel_base`]
/// (issue `6823c8a0`).
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn fp_ple_panel_base_available<const P: u64>() -> bool {
    fp_small_enabled::<P>() && crate::simd::maybe_fp_small_ple().is_some()
}

#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_ple_panel_base_available<const P: u64>() -> bool {
    false
}

/// Physically rearrange the rows of `matrix` according to `row_perm`,
/// **leaving the column window `[col_lo, col_hi)` untouched** (the
/// kernel already permuted those bytes in its own scratch buffer).
///
/// `row_perm[k]` = original row index that now sits at row `k`. We
/// build a buffer of size `m` recording, for each original row, where
/// it now lives; then swap the outside-window cells via cycle
/// decomposition so each row's external cells are moved exactly once.
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
        // Window spans the full row width; nothing to swap outside.
        return;
    }
    // Compute the inverse: `inv[src] = dst`, meaning the row originally
    // at index `src` now lives at row `dst`. We use this to walk
    // cycles.
    let mut where_now: Vec<usize> = vec![0; m];
    for (dst, &src) in row_perm.iter().enumerate() {
        where_now[src] = dst;
    }

    // Identity case fast-out.
    if where_now.iter().enumerate().all(|(i, &v)| i == v) {
        return;
    }

    // Cycle walk: visit each row, follow `where_now` until we return.
    // For each cycle of length > 1, swap the outside-window cells
    // around the cycle.
    let mut visited = vec![false; m];
    for start in 0..m {
        if visited[start] || where_now[start] == start {
            visited[start] = true;
            continue;
        }
        // Walk the cycle starting at `start`. We'll perform a sequence
        // of pairwise swaps that achieve the same permutation.
        //
        // Strategy: for cycle (r0, r1, r2, ..., rk) where r0 is the
        // smallest unvisited, with row originally at r_i moving to
        // position r_{i+1 mod k+1}, we can apply the cycle by doing
        // k swaps: swap(r_0, r_1), swap(r_0, r_2), ..., swap(r_0, r_k).
        // After these k swaps, the row originally at r_i is at
        // position r_{i+1 mod k+1}. This walks each row exactly once
        // among the cycle's non-pivot positions.
        //
        // We need to be careful: the row_perm encodes "now-row's
        // original index"; we need to ensure outside-window cells end
        // up at the row indices that match the kernel's window cells.
        //
        // Approach: use `row_perm` directly. After the kernel call,
        // window[k * win + c] holds the cell that should sit at row k
        // post-PLE. Outside the window, the cell at row k should be
        // the original cell at row `row_perm[k]`. So we need:
        //   matrix_outside[k, :] = matrix_outside_original[row_perm[k], :]
        //
        // Equivalently, for each k, copy original-row `row_perm[k]`'s
        // outside-window cells into row `k`. Since rows can move both
        // ways, we use a per-cycle scratch swap.
        //
        // Simplest correct (allocation-free per cycle): copy the cycle
        // out into a stack/heap buffer, then write back. For
        // `parent_cols - win` outside cells per row, the buffer cost
        // is bounded by `cycle_len * (parent_cols - win)` field
        // elements; cycles are short in practice (rank-revealing PLE
        // generates a small number of swaps).
        //
        // For clarity and correctness we use a small Vec per cycle.
        let mut cycle: Vec<usize> = Vec::new();
        let mut cur = start;
        while !visited[cur] {
            cycle.push(cur);
            visited[cur] = true;
            cur = where_now[cur];
        }
        // `cycle` lists positions [r_0, r_1, ..., r_{k}] in walk order
        // where row originally at r_i now lives at r_{i+1 mod len}.
        // Hence the row at the **new** position r_{i+1} came from the
        // **original** position r_i. Equivalently:
        //   new_row(r_{i+1 mod len}) = orig_row(r_i)
        //
        // We want to physically move the outside-window cells so that
        // `matrix[new_pos]` carries `original_matrix[orig_pos]`.
        //
        // Buffer the original outside-window cells for every row in
        // the cycle, then redistribute.
        let outside_len_left = col_lo;
        let outside_len_right = parent_cols - col_hi;
        let outside_total = outside_len_left + outside_len_right;
        if outside_total == 0 {
            continue;
        }
        let mut buf: Vec<Fp<P>> = Vec::with_capacity(cycle.len() * outside_total);
        // First read all the original cells.
        for &pos in &cycle {
            let row_base = pos * parent_cols;
            for c in 0..col_lo {
                buf.push(matrix[row_base + c]);
            }
            for c in col_hi..parent_cols {
                buf.push(matrix[row_base + c]);
            }
        }
        // Now write back: for cycle index i (so new-position
        // r_{i+1 mod len} should receive the original cells of r_i),
        // write buf[i * outside_total .. (i+1) * outside_total] into
        // row `cycle[(i + 1) mod len]`'s outside cells.
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

/// Compose `perm` with `row_perm` (apply `row_perm` to the existing
/// `perm` slots).
///
/// `perm` is the caller's permutation tracker (length `m`); the
/// kernel's local `row_perm` says "the row originally at `row_perm[k]`
/// now sits at row `k`". After composition, `perm[k]` reflects the
/// composed source-index.
#[cfg(feature = "simd")]
fn apply_perm_indices(perm: &mut [usize], row_perm: &[usize]) {
    debug_assert_eq!(perm.len(), row_perm.len());
    // perm_new[k] = perm_old[row_perm[k]]
    let perm_old: Vec<usize> = perm.to_vec();
    for (k, slot) in perm.iter_mut().enumerate() {
        *slot = perm_old[row_perm[k]];
    }
}

/// Non-SIMD stub for `PackedFpChainPolys<P>`.
#[cfg(not(feature = "simd"))]
pub(crate) struct PackedFpChainPolys<const P: u64>;

/// Non-SIMD stub that always returns `None`.
#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_try_make_chain_poly_arith<const P: u64>(
    _n: usize,
) -> Option<Box<dyn crate::field::matrix::ChainPolyArith<Fp<P>>>> {
    None
}

/// Non-SIMD stub that always returns `false`.
#[cfg(not(feature = "simd"))]
#[inline]
pub(crate) fn fp_chain_poly_arith_available<const P: u64>() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORD_BOUNDARY_LENS: &[usize] = &[0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257];

    // ---------------------------------------------------------------------------
    // Tests for the inlined / cached matvec path (issue 70766cb1).
    // Checks scalar-equivalence of the PackedFpMatrix::Small path at the
    // boundary lengths mandated by the issue: {0, 1, 15, 16, 17, 63, 64, 65}.
    // ---------------------------------------------------------------------------

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

    /// Boundary-length scalar-equivalence test for the inlined/cached
    /// small-prime prepack matvec path (issue 70766cb1).
    ///
    /// Tests lengths {0, 1, 15, 16, 17, 63, 64, 65} for both m and k, at
    /// GF(251) (the primary target of the 70766cb1 optimization) and
    /// GF(7) (regression guard).
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

    /// Boundary-length scalar-equivalence test for the small-n GEMM
    /// dispatch path (issue 27bb2f75). Covers the table-lookup
    /// pack/unpack and the thread-local scratch reuse for
    /// `fp_small_try_gemm_classical`. Lengths `{0, 1, 15, 16, 17, 63,
    /// 64, 65, 128, 129}` exercise the kernel's row-panel tile (4× per
    /// inner pass) at its boundaries and the per-row scratch reuse.
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

    /// Boundary-length scalar-equivalence test for the small-n GEMM
    /// dispatch path (issue 27bb2f75). Covers the lengths mandated by
    /// the issue: `{0, 1, 15, 16, 17, 63, 64, 65, 128, 129}` for each
    /// of m, k, n. Tests both GF(7) (lowest in-scope prime; small
    /// table) and GF(251) (largest in-scope prime; table spans 251
    /// slots).
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
        /// bit-exactly. Boundary lengths from issue 27bb2f75's success
        /// criteria: `{0, 1, 15, 16, 17, 63, 64, 65, 128, 129}`.
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

        /// Property: the inlined/cached small-prime prepack matvec path
        /// returns the same result as the scalar reference for any
        /// `(m, k)` shape with each dimension in
        /// `{0, 1, 15, 16, 17, 63, 64, 65}`. Issue `70766cb1` review
        /// feedback (R1) explicitly required a proptest over these
        /// boundary lengths, distinct from the deterministic
        /// `test_small_prime_prepack_matvec_boundary_lengths` unit test.
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

    /// Property test: SIMD path matches scalar element-wise across
    /// `WORD_BOUNDARY_LENS` for the small-prime byte-lane dispatch
    /// (`P <= 251`). Mirrors [`check_generic_prime`] but exercises the
    /// `fp_small_*` SIMD branch installed by `try_simd_*_vec`.
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
        // Reference prime named in the [hard] criterion of issue 9e12659b.
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
