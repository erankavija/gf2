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

/// Per-(P, m, k, n) Candidate-F selector.
///
/// Per the b9aed0d8 § 6.1 amendment (user-approved 2026-05-06): every
/// `Fp<P>` cell with `P ≤ 251` routes to Candidate F when the host
/// CPU supports AVX2 + FMA3. The `(m, k, n)` parameters are part of
/// the per-(P, n) rule's signature and are forwarded for forward
/// compatibility — a future amendment supported by fresh F bench data
/// could refine the table without changing dispatch wiring.
#[cfg(feature = "simd")]
#[inline]
const fn select_f32_path<const P: u64>(_m: usize, _k: usize, _n: usize) -> bool {
    fp_small_enabled_const::<P>()
}

/// `fp_small_enabled` evaluated at compile time so it can be used in
/// `const fn select_f32_path`. Mirrors the runtime predicate exactly.
#[cfg(feature = "simd")]
#[inline]
const fn fp_small_enabled_const<const P: u64>() -> bool {
    P >= 3 && P <= 251
}

/// Whole-gemm fast path. Pre-packs `a` (`m × k` row-major) and `b_t`
/// (`n × k` row-major, already transposed by the caller) to
/// canonical-byte SoA buffers and runs the AVX2 byte-lane batch-dot
/// kernel for every output cell against the cached packs. Unpacks the
/// output and writes it through `out` (`m × n` row-major).
///
/// On FMA3-capable hosts the f32-cascade kernel (Candidate F per
/// `dev/plans/small_prime_kernel_strategy.md` § 6.1) is preferred —
/// it issues `_mm256_fmadd_ps` at twice the throughput of
/// Candidate C's `_mm256_madd_epi16`. The Candidate C
/// (`_mm256_madd_epi16`-based) path remains compiled in as the
/// AVX2-only-no-FMA3 runtime fallback per the same amendment.
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

    // Pack A row-major, B-transpose row-major, both into canonical
    // bytes. One Montgomery REDC per element via `Fp::value()`.
    let a_u8: Vec<u8> = a.iter().map(|x| x.value() as u8).collect();
    let bt_u8: Vec<u8> = b_t.iter().map(|x| x.value() as u8).collect();
    let p_u8 = P as u8;
    let mut out_u8 = vec![0u8; m * n];

    // Candidate F (AVX2 + FMA3 f32-cascade) — preferred whenever the
    // selector authorises this (P, m, k, n) cell AND the host supports
    // FMA3 at runtime. Per § 6.1 amendment the selector resolves
    // uniformly to `true` for every `P ≤ 251`, and FMA3 is present
    // on every Zen-2+ AMD and every Haswell+ Intel part.
    let f32_taken = if select_f32_path::<P>(m, k, n) {
        if let Some(fns_f32) = crate::simd::maybe_fp_small_f32() {
            (fns_f32.batch_gemm_fn)(&a_u8, &bt_u8, m, k, n, p_u8, &mut out_u8);
            true
        } else {
            false
        }
    } else {
        false
    };

    if !f32_taken {
        // Candidate C (AVX2 16-bit-integer Barrett kernel) runtime
        // fallback for AVX2-only-no-FMA3 hosts (Zen 1, Sandy Bridge).
        let Some(fns) = crate::simd::maybe_fp_small() else {
            return false;
        };
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
