//! Shared Barrett-reduction helpers for AVX2 + VPCLMULQDQ GF(2^m) kernels
//! over `m in {8, 16, 32}`.
//!
//! These helpers are the single source of truth for the carry-less-multiply
//! plus Barrett-reduce algorithm used by both `gf2m_batch` (per-element
//! batch multiply / square) and `gf2m_gemm` (panelized broadcast-multiply
//! GEMM). Keeping them here avoids the ~100-line copy/paste that would
//! otherwise need to be maintained in two places.
//!
//! All helpers are `pub(crate)`, `unsafe`, and `#[inline(always)]`. Callers
//! must invoke them from a `#[target_feature(enable = "avx2", enable =
//! "vpclmulqdq", enable = "pclmulqdq", enable = "sse4.1")]` context — the
//! intrinsics inside the helpers require those features. With
//! `#[inline(always)]` the helpers are inlined into the caller's
//! target-feature scope, so the compiler emits the intrinsics correctly.

#![allow(clippy::missing_safety_doc)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Single-element carry-less multiply + Barrett reduce, kept inline so
/// tail-handling in batch / GEMM kernels avoids call-pointer overhead.
///
/// Inputs:
/// - `a`, `b`: GF(2^m) elements as `u64` (high bits beyond `degree` must
///   be zero on entry; the function assumes inputs already canonical).
/// - `mu`: the precomputed Barrett constant (`floor(x^{2m} / modulus)`).
/// - `modulus`: the irreducible polynomial as a `u64` with the implicit
///   high bit at position `degree` cleared (low-`degree`-bit polynomial).
/// - `degree`: `m`, the field degree, in `{8, 16, 32, 64}`.
///
/// Output: the canonical `u64` representation of `a * b mod modulus`,
/// masked to `degree` bits.
#[inline(always)]
pub(crate) unsafe fn clmul_barrett_scalar(
    a: u64,
    b: u64,
    mu: u64,
    modulus: u64,
    degree: u32,
) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }
    let field_mask = if degree == 64 {
        u64::MAX
    } else {
        (1u64 << degree) - 1
    };

    let a_reg = _mm_set_epi64x(0, a as i64);
    let b_reg = _mm_set_epi64x(0, b as i64);
    let product_reg = _mm_clmulepi64_si128::<0x00>(a_reg, b_reg);

    let prod_lo = _mm_extract_epi64::<0>(product_reg) as u64;
    let prod_hi = _mm_extract_epi64::<1>(product_reg) as u64;
    let product = ((prod_hi as u128) << 64) | prod_lo as u128;

    if product >> degree == 0 {
        return product as u64;
    }

    let c_high = (product >> degree) as u64;
    let c_high_reg = _mm_set_epi64x(0, c_high as i64);
    let mu_reg = _mm_set_epi64x(0, mu as i64);
    let q_full_reg = _mm_clmulepi64_si128::<0x00>(c_high_reg, mu_reg);

    let q_lo = _mm_extract_epi64::<0>(q_full_reg) as u64;
    let q_hi = _mm_extract_epi64::<1>(q_full_reg) as u64;
    let q_full = ((q_hi as u128) << 64) | q_lo as u128;
    let q = (q_full >> degree) as u64;

    let q_reg = _mm_set_epi64x(0, q as i64);
    let mod_reg = _mm_set_epi64x(0, modulus as i64);
    let qp_reg = _mm_clmulepi64_si128::<0x00>(q_reg, mod_reg);

    let qp_lo = _mm_extract_epi64::<0>(qp_reg) as u64;
    let qp_hi = _mm_extract_epi64::<1>(qp_reg) as u64;
    let qp = ((qp_hi as u128) << 64) | qp_lo as u128;

    let mut r = product ^ qp;
    if r >> degree != 0 {
        r ^= modulus as u128;
    }
    if r >> degree != 0 {
        r ^= modulus as u128;
    }
    (r as u64) & field_mask
}

/// YMM-resident Barrett reduction on two 256-bit registers, each holding
/// two 128-bit lanes of carry-less products for `m in {8, 16, 32}`.
///
/// `SHIFT_BYTES` is `m / 8`: 1 for m=8, 2 for m=16, 4 for m=32. The
/// `_mm256_srli_si256` intrinsic performs a per-128-bit-lane byte shift
/// that matches the lane layout used by the batch and GEMM kernels.
///
/// Returns the partially-reduced `(r_lo, r_hi)` YMM pair; callers must
/// run the [`correct`] step on each `u64` lane to land the result in
/// `[0, P)`.
#[inline(always)]
pub(crate) unsafe fn ymm_barrett_reduce<const SHIFT_BYTES: i32>(
    prod_lo: __m256i,
    prod_hi: __m256i,
    mu_ymm: __m256i,
    mod_ymm: __m256i,
) -> (__m256i, __m256i) {
    let c_high_lo = _mm256_srli_si256::<SHIFT_BYTES>(prod_lo);
    let c_high_hi = _mm256_srli_si256::<SHIFT_BYTES>(prod_hi);

    let q_full_lo = _mm256_clmulepi64_epi128::<0x00>(c_high_lo, mu_ymm);
    let q_full_hi = _mm256_clmulepi64_epi128::<0x00>(c_high_hi, mu_ymm);

    let q_lo = _mm256_srli_si256::<SHIFT_BYTES>(q_full_lo);
    let q_hi = _mm256_srli_si256::<SHIFT_BYTES>(q_full_hi);

    let qp_lo = _mm256_clmulepi64_epi128::<0x00>(q_lo, mod_ymm);
    let qp_hi = _mm256_clmulepi64_epi128::<0x00>(q_hi, mod_ymm);

    let r_lo = _mm256_xor_si256(prod_lo, qp_lo);
    let r_hi = _mm256_xor_si256(prod_hi, qp_hi);

    (r_lo, r_hi)
}

/// Final correction: `r in [0, 2P)` -> `r in [0, P)` by conditional XOR
/// with modulus when the degree-m bit is set, plus a defensive second
/// pass for inputs near the upper edge of `[0, 2P)`.
///
/// `shift_bytes` is `m / 8`; `mask` is `(1 << m) - 1` (or `u64::MAX` for
/// m=64).
#[inline(always)]
pub(crate) fn correct(mut r: u64, modulus: u64, shift_bytes: i32, mask: u64) -> u64 {
    let degree = (shift_bytes as u32) * 8;
    if (r >> degree) != 0 {
        r ^= modulus;
    }
    if (r >> degree) != 0 {
        r ^= modulus;
    }
    r & mask
}
