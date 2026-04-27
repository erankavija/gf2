//! AVX2 / PCLMULQDQ kernels for the 4×4 schoolbook carry-less multiplication
//! used by `Gf2mWide<4>` (GF(2^256)).
//!
//! The public surface is `clmul_wide4_*`: each function computes the full
//! unreduced 8-limb (512-bit) carry-less product of two 4-limb (256-bit)
//! GF(2)-polynomial operands. Barrett reduction is applied by the caller.
//!
//! # Lanes
//!
//! - [`clmul_wide4_ymm`] — uses VPCLMULQDQ on YMM (256-bit) registers via AVX2.
//!   Each VPCLMULQDQ instruction computes two 64×64 carry-less multiplies
//!   (one per 128-bit lane). The 16 scalar products of the 4×4 schoolbook
//!   fold into 8 YMM multiplies. **Primary path on Zen 3.**
//!
//! - [`clmul_wide4_xmm`] — uses PCLMULQDQ on XMM (128-bit) registers. One
//!   _mm_clmulepi64_si128 per scalar product. Universal x86_64 fallback.
//!
//! Every function XOR-accumulates into `out[i + j]` / `out[i + j + 1]` for
//! the lo/hi halves of each 64×64 clmul. This matches the scalar
//! `clmul_wide_slice::<4>` layout exactly.
//!
//! A ZMM (AVX-512VL + VPCLMULQDQ) lane is out of scope while the test host
//! is AVX2-only (Zen 3); the required `_mm512_*` carry-less-multiply and
//! 128-bit-lane extraction intrinsics are stable since Rust 1.89, available
//! under the current MSRV (1.95).

#![allow(clippy::missing_safety_doc)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// XMM (PCLMULQDQ) scalar-lane fallback
// ---------------------------------------------------------------------------

/// 4×4 schoolbook carry-less multiply using 16 scalar PCLMULQDQ instructions.
///
/// Computes the 8-limb carry-less product of two 4-limb operands. Each
/// partial product `a[i] · b[j]` is XOR-accumulated into `out[i + j]` (low
/// 64 bits) and `out[i + j + 1]` (high 64 bits). `out` is cleared before
/// accumulation.
///
/// # Safety
///
/// Requires the `pclmulqdq` CPU feature.
#[target_feature(enable = "pclmulqdq")]
pub unsafe fn clmul_wide4_xmm(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {
    // Zero out the accumulator.
    for slot in out.iter_mut() {
        *slot = 0;
    }

    // 16 scalar PCLMULQDQ calls, matching the scalar schoolbook order.
    for i in 0..4 {
        let ai = _mm_set_epi64x(0, a[i] as i64);
        for j in 0..4 {
            let bj = _mm_set_epi64x(0, b[j] as i64);
            let product = _mm_clmulepi64_si128::<0x00>(ai, bj);
            let lo = _mm_extract_epi64::<0>(product) as u64;
            let hi = _mm_extract_epi64::<1>(product) as u64;
            out[i + j] ^= lo;
            out[i + j + 1] ^= hi;
        }
    }
}

// ---------------------------------------------------------------------------
// YMM (AVX2 + VPCLMULQDQ) primary path
// ---------------------------------------------------------------------------

/// Schoolbook carry-less multiply using VPCLMULQDQ on YMM (256-bit) lanes.
///
/// Each YMM register contains two independent 128-bit lanes; the
/// `_mm256_clmulepi64_epi128` intrinsic performs one clmul per lane, yielding
/// two 64×64 carry-less products per instruction. We pair the 16 scalar
/// products of the 4×4 schoolbook into 8 YMM multiplies, pairing `(i, j)` with
/// `(i, j+1)` so each pair shares the same `a[i]` operand in both lanes.
///
/// The layout matches the scalar `clmul_wide_slice::<4>`: partial product
/// `a[i] · b[j]` XOR-accumulates into `out[i + j]` (lo) and `out[i + j + 1]`
/// (hi).
///
/// # Safety
///
/// Requires the `avx2` and `vpclmulqdq` CPU features.
#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
pub unsafe fn clmul_wide4_ymm(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {
    // Zero-initialise the 8-limb accumulator. We hold it in a stack-allocated
    // array; the inner loop extracts YMM lane halves and XORs them in.
    for slot in out.iter_mut() {
        *slot = 0;
    }

    // Pair columns (j, j+1) so each YMM operand packs b[j] (lane 0) and
    // b[j+1] (lane 1). For a[i] we broadcast the same scalar into both lanes.
    //
    // `_mm256_set_epi64x(hi_hi, hi_lo, lo_hi, lo_lo)` — the low 128-bit lane
    // gets `(lo_hi, lo_lo)`; the high 128-bit lane gets `(hi_hi, hi_lo)`.
    // We want lane 0 to carry `(0, b[j])` and lane 1 `(0, b[j+1])`.
    for i in 0..4 {
        let a_vec = _mm256_set_epi64x(0, a[i] as i64, 0, a[i] as i64);

        // j = 0, 1 paired; j = 2, 3 paired.
        for jp in (0..4).step_by(2) {
            let b_vec = _mm256_set_epi64x(0, b[jp + 1] as i64, 0, b[jp] as i64);

            // One VPCLMULQDQ produces two 128-bit products, one per lane.
            let product = _mm256_clmulepi64_epi128::<0x00>(a_vec, b_vec);

            // Lane 0 is a[i] · b[jp]; lane 1 is a[i] · b[jp + 1].
            let lane0 = _mm256_extracti128_si256::<0>(product);
            let lane1 = _mm256_extracti128_si256::<1>(product);

            let lo0 = _mm_extract_epi64::<0>(lane0) as u64;
            let hi0 = _mm_extract_epi64::<1>(lane0) as u64;
            out[i + jp] ^= lo0;
            out[i + jp + 1] ^= hi0;

            let lo1 = _mm_extract_epi64::<0>(lane1) as u64;
            let hi1 = _mm_extract_epi64::<1>(lane1) as u64;
            out[i + jp + 1] ^= lo1;
            out[i + jp + 2] ^= hi1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m_wide::test_helpers::scalar_ref;

    fn sample_vectors() -> Vec<([u64; 4], [u64; 4])> {
        vec![
            ([0, 0, 0, 0], [0, 0, 0, 0]),
            ([1, 0, 0, 0], [1, 0, 0, 0]),
            ([0, 0, 0, 1], [0, 0, 0, 1]),
            (
                [0xDEAD_BEEF_CAFE_BABE, 0x0123_4567_89AB_CDEF, 0, 0xFFFF],
                [0x5555_5555_5555_5555, 0xAAAA_AAAA_AAAA_AAAA, 0xFFFF, 1],
            ),
            (
                [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
                [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
            ),
            (
                [0x8000_0000_0000_0000, 0, 0, 0x8000_0000_0000_0000],
                [0x8000_0000_0000_0000, 0, 0, 0x8000_0000_0000_0000],
            ),
        ]
    }

    #[test]
    fn xmm_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            eprintln!("skipping: no PCLMULQDQ");
            return;
        }
        for (a, b) in sample_vectors() {
            let mut got = [0u64; 8];
            unsafe { clmul_wide4_xmm(&a, &b, &mut got) };
            assert_eq!(got, scalar_ref(&a, &b), "XMM mismatch for {a:?} * {b:?}");
        }
    }

    #[test]
    fn ymm_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq")) {
            eprintln!("skipping: no AVX2+VPCLMULQDQ");
            return;
        }
        for (a, b) in sample_vectors() {
            let mut got = [0u64; 8];
            unsafe { clmul_wide4_ymm(&a, &b, &mut got) };
            assert_eq!(got, scalar_ref(&a, &b), "YMM mismatch for {a:?} * {b:?}");
        }
    }
}
