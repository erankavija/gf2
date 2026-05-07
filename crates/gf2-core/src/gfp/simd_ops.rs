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
/// Set to 252 (above every in-scope small prime) based on the 5-trial
/// empirical criterion sweep on 2026-05-06 covering GF(7)–GF(251) at
/// n ∈ {256, 1024}: Candidate C beat Candidate F at every (prime, n) cell
/// by 5–10 % (IQR-based verdict: C_WINS at all 22 of 22 cells).
/// See `dev/bench_results/2026-05-06-662f7a15-prime-sweep-aggregate.csv`
/// and the §6.1 sub-amendment in `dev/plans/small_prime_kernel_strategy.md`
/// for the full verdict table and rationale.
///
/// To select F for primes ≥ some threshold, lower this constant (e.g.
/// `N_THRESH_PRIME = 11` would route GF(7) to C and GF(11)+ to F). The
/// dispatch wiring is forward-compatible; amending this constant is the
/// only code change needed when fresh data supports a lower threshold.
#[cfg(feature = "simd")]
const N_THRESH_PRIME: u64 = 252;

/// Per-(P, m, k, n) Candidate-F selector.
///
/// Returns `true` iff `P ≥ N_THRESH_PRIME && P ≤ 251`. With
/// `N_THRESH_PRIME = 252` (empirically set — see its doc comment) this
/// evaluates to `false` for every in-scope small prime, routing all
/// `p ≤ 251` cells to Candidate C. The `(m, k, n)` parameters are part
/// of the per-(P, n) rule's signature and are forwarded for forward
/// compatibility — a future amendment can refine the threshold without
/// changing the dispatch wiring.
#[cfg(feature = "simd")]
#[inline]
#[allow(clippy::impossible_comparisons)] // intentional: N_THRESH_PRIME=252 makes this always false
const fn select_f32_path<const P: u64>(_m: usize, _k: usize, _n: usize) -> bool {
    P >= N_THRESH_PRIME && P <= 251
}

/// Whole-gemm fast path. Pre-packs `a` (`m × k` row-major) and `b_t`
/// (`n × k` row-major, already transposed by the caller) to
/// canonical-byte SoA buffers and runs the AVX2 byte-lane batch-dot
/// kernel for every output cell against the cached packs. Unpacks the
/// output and writes it through `out` (`m × n` row-major).
///
/// **Dispatch policy (2026-05-06 prime-sweep sub-amendment):** Candidate C
/// (`_mm256_madd_epi16`-based) handles all `p ≤ 251` cells on both
/// AVX2-only and AVX2+FMA3 hosts. The 5-trial criterion sweep over
/// GF(7)–GF(251) at n ∈ {256, 1024} showed C beats F by 5–10 % at every
/// cell; `select_f32_path` returns `false` for all in-scope primes
/// (`N_THRESH_PRIME = 252`). Candidate F remains compiled in as an upgrade
/// path for future measurement on a different host or at larger n.
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
    let mut out_u8 = vec![0u8; m * n];

    // Candidate F (AVX2 + FMA3 f32-cascade) — compiled in as upgrade path;
    // currently not selected at any `P ≤ 251` cell (N_THRESH_PRIME = 252,
    // see select_f32_path doc). The selector returns `false` for all
    // in-scope small primes per the 2026-05-06 prime-sweep sub-amendment
    // (§ 6.1 of the design doc): C_WINS at every (prime, n) cell.
    let f32_taken = if select_f32_path::<P>(m, k, n) {
        if let Some(fns_f32) = crate::simd::maybe_fp_small_f32() {
            let a_f32: Vec<f32> = a.iter().map(|x| x.value() as f32).collect();
            let bt_f32: Vec<f32> = b_t.iter().map(|x| x.value() as f32).collect();
            (fns_f32.batch_gemm_fn)(&a_f32, &bt_f32, m, k, n, p_u8, &mut out_u8);
            true
        } else {
            false
        }
    } else {
        false
    };

    if !f32_taken {
        // Candidate C (AVX2 16-bit-integer Barrett kernel) — primary path
        // for all `p ≤ 251` cells per the 2026-05-06 prime-sweep amendment
        // (N_THRESH_PRIME = 252, select_f32_path always returns false).
        // Also the fallback for AVX2-only-no-FMA3 hosts (Zen 1, Sandy Bridge).
        let Some(fns) = crate::simd::maybe_fp_small() else {
            return false;
        };
        // Pack A and B-transpose as canonical bytes for the integer
        // kernel — one Montgomery REDC per element via `Fp::value()`.
        let a_u8: Vec<u8> = a.iter().map(|x| x.value() as u8).collect();
        let bt_u8: Vec<u8> = b_t.iter().map(|x| x.value() as u8).collect();
        for i in 0..m {
            let a_row = &a_u8[i * k..(i + 1) * k];
            let out_row = &mut out_u8[i * n..(i + 1) * n];
            (fns.gemm_row_panel_fn)(a_row, &bt_u8, k, n, p_u8, out_row);
        }
    }

    // Unpack canonical → Montgomery storage.
    for (slot, &byte) in out.iter_mut().zip(out_u8.iter()) {
        *slot = Fp::<P>::new(byte as u64);
    }
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
        for (a, slot) in from_mont.iter_mut().enumerate() {
            let canon = Fp::<P>::from_raw_storage(a as u64).value(); // from_mont(a)
            *slot = canon as u8;
            let raw = Fp::<P>::new(canon).raw_storage();
            to_mont[canon as usize] = raw;
        }
        SmallPrimeTables { from_mont, to_mont }
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
    Small { cols: Vec<Vec<u8>>, n: usize },
    Medium { cols: Vec<Vec<u16>>, n: usize },
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
                n,
            });
        }
        if fp_medium_eligible::<P>() {
            crate::simd::maybe_fp_medium()?;
            return Some(Self::Medium {
                cols: Vec::new(),
                n,
            });
        }
        None
    }

    /// Appends a column (as `Fp<P>`) by packing into canonical form.
    pub(crate) fn push(&mut self, col: &[Fp<P>]) {
        match self {
            PackedFpBasis::Small { cols, n } => {
                debug_assert_eq!(col.len(), *n);
                let packed: Vec<u8> = col.iter().map(|v| v.value() as u8).collect();
                cols.push(packed);
            }
            PackedFpBasis::Medium { cols, n } => {
                debug_assert_eq!(col.len(), *n);
                let packed: Vec<u16> = col.iter().map(|v| v.value() as u16).collect();
                cols.push(packed);
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
        PackedFpBasis::Small { cols, .. } => {
            let fns = crate::simd::maybe_fp_small().expect("PackedFpBasis::Small requires AVX2");
            let p_u8 = P as u8;
            let mut residual: Vec<u8> = v.iter().map(|x| x.value() as u8).collect();
            let mut coeffs: Vec<Fp<P>> = vec![Fp::<P>::new(0); basis_len];
            let mut bcast: Vec<u8> = vec![0u8; n];
            let mut tmp: Vec<u8> = vec![0u8; n];
            let mut new_residual: Vec<u8> = vec![0u8; n];
            for (j, col) in cols.iter().enumerate() {
                let r = pivot_row_of_col[j];
                let v_at_r = residual[r];
                if v_at_r == 0 {
                    continue;
                }
                let pivot_val = col[r];
                let pivot_inv = Fp::<P>::new(pivot_val as u64)
                    .inv()
                    .expect("reduce: pivot must be non-zero")
                    .value() as u8;
                // factor = v_at_r * pivot_inv mod P (canonical scalar mul).
                let factor = ((v_at_r as u32 * pivot_inv as u32) % P as u32) as u8;
                bcast.iter_mut().for_each(|s| *s = factor);
                (fns.batch_mul_fn)(&bcast, col, p_u8, &mut tmp);
                (fns.batch_sub_fn)(&residual, &tmp, p_u8, &mut new_residual);
                std::mem::swap(&mut residual, &mut new_residual);
                coeffs[j] = Fp::<P>::new(factor as u64);
            }
            let unpacked: Vec<Fp<P>> = residual.iter().map(|&b| Fp::<P>::new(b as u64)).collect();
            (unpacked, coeffs)
        }
        PackedFpBasis::Medium { cols, .. } => {
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
                let pivot_val = col[r];
                let pivot_inv = Fp::<P>::new(pivot_val as u64)
                    .inv()
                    .expect("reduce: pivot must be non-zero")
                    .value() as u16;
                let factor = ((v_at_r as u64 * pivot_inv as u64) % P) as u16;
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
        self.push(col);
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
// All arithmetic stays in canonical-byte form; the only Montgomery REDC
// calls are at `alpha.value()` (one per non-zero α_j) and at the very
// end when `finish_buf` converts bytes back to `FieldPoly<Fp<P>>` via
// `Fp::new`.
// ---------------------------------------------------------------------------

/// Packed canonical-byte chain-polynomial store for small primes (`P ≤ 251`).
///
/// Used by `cyclic_decomposition` (issue `5a3dbd5b`) to replace the scalar
/// `FieldPoly::mul_scalar` / `Sub` polynomial-bookkeeping with AVX2
/// byte-lane kernels, closing the ~10x wall-clock gap on `GF(251)/n=256
/// charpoly` reported in `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md`
/// § 6.4.
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
    /// Pre-allocated scratch buffer of 3 × max_cj_len bytes.  Layout:
    ///   [0..n]:   broadcast (alpha repeated n times)
    ///   [n..2n]:  scaled polynomial (alpha * chain_j)
    ///   [2n..3n]: subtraction result (buf - scaled)
    /// Grown lazily; never shrunk.  Using a single allocation and
    /// `split_at_mut` gives safe non-overlapping mutable borrows.
    scratch: Vec<u8>,
    /// Current capacity per lane (scratch.len() / 3).
    scratch_cap: usize,
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
            scratch: Vec::new(),
            scratch_cap: 0,
        })
    }

    /// Ensures the scratch buffer has capacity for at least `n` bytes per
    /// lane.  Reallocates at most once per Krylov block (monotonically
    /// growing polynomials).
    #[inline]
    fn ensure_scratch(&mut self, n: usize) {
        if n > self.scratch_cap {
            self.scratch.resize(n * 3, 0u8);
            self.scratch_cap = n;
        }
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
        let alpha_val = alpha.value() as u8;
        if alpha_val == 0 {
            return;
        }
        let fns = crate::simd::maybe_fp_small()
            .expect("PackedFpChainPolys::sub_scaled_into requires AVX2");
        let p_u8 = P as u8;
        let cj_len = self.polys[j].len();
        // buf must be at least as long as chain_j.
        debug_assert!(
            buf.len() >= cj_len,
            "sub_scaled_into: buf len {} < chain_j len {}",
            buf.len(),
            cj_len
        );
        // Ensure the single scratch buffer has 3 × cj_len capacity.
        // Layout: [0..cj_len] = broadcast; [cj_len..2*cj_len] = scaled;
        //         [2*cj_len..3*cj_len] = sub result.
        self.ensure_scratch(cj_len);
        // Split scratch into three non-overlapping lanes.
        // Layout:  [0..cap]=lane0  [cap..2cap]=lane1  [2cap..3cap]=lane2
        //
        // Steps:
        //  1. lane1[..cj_len] = polys[j]      (chain copy)
        //  2. lane0[..cj_len] = alpha_val      (broadcast)
        //  3. batch_mul(lane0, lane1) -> lane2 (scaled = alpha * chain)
        //  4. batch_sub(buf, lane2) -> lane0   (diff = buf - scaled)
        //  5. buf[..cj_len] = lane0[..cj_len]
        let cap = self.scratch_cap;
        // Step 1: copy polys[j] into lane1 (polys and scratch are separate
        // fields, so we can borrow them simultaneously).
        self.scratch[cap..cap + cj_len].copy_from_slice(&self.polys[j]);
        // Step 2: fill lane0 with broadcast.
        self.scratch[..cj_len].fill(alpha_val);
        // Step 3: batch_mul(lane0, lane1) -> lane2.
        // Split into (lane0+lane1) | lane2 to get mutable access to lane2
        // while borrowing lane0 and lane1 immutably.
        let (lo_and_l1, lane2_full) = self.scratch.split_at_mut(2 * cap);
        let bcast = &lo_and_l1[..cj_len];
        let chain_copy = &lo_and_l1[cap..cap + cj_len];
        let scaled = &mut lane2_full[..cj_len];
        (fns.batch_mul_fn)(bcast, chain_copy, p_u8, scaled);
        // Step 4: batch_sub(buf, lane2) -> lane0.
        // Split into lane0 | (lane1+lane2) to get mutable access to lane0
        // while borrowing lane2 immutably.
        let (lane0_full, l1_and_l2) = self.scratch.split_at_mut(cap);
        let diff_out = &mut lane0_full[..cj_len];
        let scaled_in = &l1_and_l2[cap..cap + cj_len]; // lane2[..cj_len]
        (fns.batch_sub_fn)(&buf[..cj_len], scaled_in, p_u8, diff_out);
        // Step 5: write result back to buf.
        buf[..cj_len].copy_from_slice(&self.scratch[..cj_len]);
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
                    // zero-dim matvec is trivially handled by the caller;
                    // try_prepack_matvec returns None for k=0.
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
