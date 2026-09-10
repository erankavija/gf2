//! AVX2 / PCLMULQDQ kernels for fixed-size schoolbook carry-less
//! multiplication used by `Gf2mWide`.
//!
//! The public surface contains `clmul_wide4_*` (GF(2^256)) and
//! `clmul_wide9_*` (GF(2^571), stored in 9 limbs). Each function computes the
//! full unreduced carry-less product. Barrett reduction is applied by the
//! caller.
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
//! Every function writes the same little-endian limb layout as the scalar
//! `clmul_wide_slice::<N>` helper: partial product `a[i] · b[j]` contributes
//! its low/high halves to `out[i + j]` / `out[i + j + 1]`.
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
/// Requires the `pclmulqdq` and `sse4.1` CPU features.
#[target_feature(enable = "pclmulqdq", enable = "sse4.1")]
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

// ---------------------------------------------------------------------------
// GF(2^571): 9×9 multi-word kernels
// ---------------------------------------------------------------------------

/// 9×9 schoolbook carry-less multiply using 81 scalar PCLMULQDQ instructions.
///
/// Computes the 18-limb carry-less product of two 9-limb operands. `out` is
/// cleared before accumulation.
///
/// # Safety
///
/// Requires the `pclmulqdq` and `sse4.1` CPU features.
#[target_feature(enable = "pclmulqdq", enable = "sse4.1")]
pub unsafe fn clmul_wide9_xmm(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {
    for slot in out.iter_mut() {
        *slot = 0;
    }

    let mut diag = [_mm_setzero_si128(); 17];
    for i in 0..9 {
        let ai = _mm_set_epi64x(0, a[i] as i64);
        for j in 0..9 {
            let bj = _mm_set_epi64x(0, b[j] as i64);
            let product = _mm_clmulepi64_si128::<0x00>(ai, bj);
            diag[i + j] = _mm_xor_si128(diag[i + j], product);
        }
    }

    fold_diagonals_9x9(diag, out);
}

/// 9×9 schoolbook carry-less multiply using VPCLMULQDQ on YMM lanes.
///
/// The 81 scalar word-pair products are scheduled row-major two at a time.
/// Each VPCLMULQDQ-on-YMM instruction computes two independent 64×64
/// carry-less products, one in each 128-bit lane; the odd final product is
/// paired with a zero lane. Products are accumulated by anti-diagonal in
/// XMM registers and folded to the 18 output limbs once at the end, reducing
/// scalar limb traffic in the hot loop.
///
/// # Safety
///
/// Requires the `avx2` and `vpclmulqdq` CPU features.
#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
pub unsafe fn clmul_wide9_ymm(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {
    for slot in out.iter_mut() {
        *slot = 0;
    }

    let mut d0 = _mm_setzero_si128();
    let mut d1 = _mm_setzero_si128();
    let mut d2 = _mm_setzero_si128();
    let mut d3 = _mm_setzero_si128();
    let mut d4 = _mm_setzero_si128();
    let mut d5 = _mm_setzero_si128();
    let mut d6 = _mm_setzero_si128();
    let mut d7 = _mm_setzero_si128();
    let mut d8 = _mm_setzero_si128();
    let mut d9 = _mm_setzero_si128();
    let mut d10 = _mm_setzero_si128();
    let mut d11 = _mm_setzero_si128();
    let mut d12 = _mm_setzero_si128();
    let mut d13 = _mm_setzero_si128();
    let mut d14 = _mm_setzero_si128();
    let mut d15 = _mm_setzero_si128();
    let mut d16 = _mm_setzero_si128();

    macro_rules! mul_pair {
        ($i0:literal, $j0:literal, $d0:ident, $i1:literal, $j1:literal, $d1:ident) => {{
            let a_vec = _mm256_set_epi64x(0, a[$i1] as i64, 0, a[$i0] as i64);
            let b_vec = _mm256_set_epi64x(0, b[$j1] as i64, 0, b[$j0] as i64);
            let product = _mm256_clmulepi64_epi128::<0x00>(a_vec, b_vec);
            $d0 = _mm_xor_si128($d0, _mm256_extracti128_si256::<0>(product));
            $d1 = _mm_xor_si128($d1, _mm256_extracti128_si256::<1>(product));
        }};
    }

    mul_pair!(0, 0, d0, 0, 1, d1);
    mul_pair!(0, 2, d2, 0, 3, d3);
    mul_pair!(0, 4, d4, 0, 5, d5);
    mul_pair!(0, 6, d6, 0, 7, d7);
    mul_pair!(0, 8, d8, 1, 0, d1);
    mul_pair!(1, 1, d2, 1, 2, d3);
    mul_pair!(1, 3, d4, 1, 4, d5);
    mul_pair!(1, 5, d6, 1, 6, d7);
    mul_pair!(1, 7, d8, 1, 8, d9);
    mul_pair!(2, 0, d2, 2, 1, d3);
    mul_pair!(2, 2, d4, 2, 3, d5);
    mul_pair!(2, 4, d6, 2, 5, d7);
    mul_pair!(2, 6, d8, 2, 7, d9);
    mul_pair!(2, 8, d10, 3, 0, d3);
    mul_pair!(3, 1, d4, 3, 2, d5);
    mul_pair!(3, 3, d6, 3, 4, d7);
    mul_pair!(3, 5, d8, 3, 6, d9);
    mul_pair!(3, 7, d10, 3, 8, d11);
    mul_pair!(4, 0, d4, 4, 1, d5);
    mul_pair!(4, 2, d6, 4, 3, d7);
    mul_pair!(4, 4, d8, 4, 5, d9);
    mul_pair!(4, 6, d10, 4, 7, d11);
    mul_pair!(4, 8, d12, 5, 0, d5);
    mul_pair!(5, 1, d6, 5, 2, d7);
    mul_pair!(5, 3, d8, 5, 4, d9);
    mul_pair!(5, 5, d10, 5, 6, d11);
    mul_pair!(5, 7, d12, 5, 8, d13);
    mul_pair!(6, 0, d6, 6, 1, d7);
    mul_pair!(6, 2, d8, 6, 3, d9);
    mul_pair!(6, 4, d10, 6, 5, d11);
    mul_pair!(6, 6, d12, 6, 7, d13);
    mul_pair!(6, 8, d14, 7, 0, d7);
    mul_pair!(7, 1, d8, 7, 2, d9);
    mul_pair!(7, 3, d10, 7, 4, d11);
    mul_pair!(7, 5, d12, 7, 6, d13);
    mul_pair!(7, 7, d14, 7, 8, d15);
    mul_pair!(8, 0, d8, 8, 1, d9);
    mul_pair!(8, 2, d10, 8, 3, d11);
    mul_pair!(8, 4, d12, 8, 5, d13);
    mul_pair!(8, 6, d14, 8, 7, d15);
    {
        let a_vec = _mm256_set_epi64x(0, 0, 0, a[8] as i64);
        let b_vec = _mm256_set_epi64x(0, 0, 0, b[8] as i64);
        let product = _mm256_clmulepi64_epi128::<0x00>(a_vec, b_vec);
        d16 = _mm_xor_si128(d16, _mm256_extracti128_si256::<0>(product));
    }

    fold_diagonals_9x9(
        [
            d0, d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15, d16,
        ],
        out,
    );
}
/// Fold anti-diagonal 128-bit products into little-endian output limbs.
///
/// `diag[k]` holds the XOR of all 128-bit word products with `i + j == k`.
/// Therefore output limb `t` is `hi(diag[t - 1]) XOR lo(diag[t])`.
#[target_feature(enable = "sse4.1")]
unsafe fn fold_diagonals_9x9(diag: [__m128i; 17], out: &mut [u64; 18]) {
    let lo0 = _mm_extract_epi64::<0>(diag[0]) as u64;
    out[0] = lo0;

    for t in 1..17 {
        let prev_hi = _mm_extract_epi64::<1>(diag[t - 1]) as u64;
        let curr_lo = _mm_extract_epi64::<0>(diag[t]) as u64;
        out[t] = prev_hi ^ curr_lo;
    }

    out[17] = _mm_extract_epi64::<1>(diag[16]) as u64;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m_wide::test_helpers::scalar_ref;

    fn sample_vectors4() -> Vec<([u64; 4], [u64; 4])> {
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

    fn sample_vectors9() -> Vec<([u64; 9], [u64; 9])> {
        vec![
            ([0; 9], [0; 9]),
            ([1, 0, 0, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0, 0, 0]),
            (
                [0, 0, 0, 0, 0, 0, 0, 0, 1u64 << 58],
                [0, 0, 0, 0, 0, 0, 0, 0, 1u64 << 58],
            ),
            (
                [
                    0xDEAD_BEEF_CAFE_BABE,
                    0x0123_4567_89AB_CDEF,
                    0xFEDC_BA98_7654_3210,
                    0xAAAA_5555_AAAA_5555,
                    0x1357_9BDF_2468_ACE0,
                    0x0F0F_F0F0_3333_CCCC,
                    0xFFFF_0000_FFFF_0000,
                    0x1111_2222_3333_4444,
                    0x07FF_FFFF_FFFF_FFFF,
                ],
                [
                    0x5555_AAAA_5555_AAAA,
                    0x1122_3344_5566_7788,
                    0xFFFF_FFFF_0000_0000,
                    0x0F0F_F0F0_0F0F_F0F0,
                    0x2468_ACE0_1357_9BDF,
                    0x3333_CCCC_0F0F_F0F0,
                    0x0000_FFFF_0000_FFFF,
                    0x4444_3333_2222_1111,
                    0x03FF_FFFF_FFFF_FFFF,
                ],
            ),
            ([u64::MAX; 9], [u64::MAX; 9]),
        ]
    }

    #[test]
    fn xmm_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1")) {
            eprintln!("skipping: no PCLMULQDQ+SSE4.1");
            return;
        }
        for (a, b) in sample_vectors4() {
            let mut got = [0u64; 8];
            unsafe { clmul_wide4_xmm(&a, &b, &mut got) };
            assert_eq!(
                got.as_slice(),
                scalar_ref(&a, &b).as_slice(),
                "XMM mismatch for {a:?} * {b:?}"
            );
        }
    }

    #[test]
    fn ymm_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq")) {
            eprintln!("skipping: no AVX2+VPCLMULQDQ");
            return;
        }
        for (a, b) in sample_vectors4() {
            let mut got = [0u64; 8];
            unsafe { clmul_wide4_ymm(&a, &b, &mut got) };
            assert_eq!(
                got.as_slice(),
                scalar_ref(&a, &b).as_slice(),
                "YMM mismatch for {a:?} * {b:?}"
            );
        }
    }

    #[test]
    fn xmm_wide9_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1")) {
            eprintln!("skipping: no PCLMULQDQ+SSE4.1");
            return;
        }
        for (a, b) in sample_vectors9() {
            let mut got = [0u64; 18];
            unsafe { clmul_wide9_xmm(&a, &b, &mut got) };
            assert_eq!(
                &got[..],
                scalar_ref(&a, &b).as_slice(),
                "XMM wide9 mismatch for {a:?} * {b:?}"
            );
        }
    }

    #[test]
    fn ymm_wide9_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq")) {
            eprintln!("skipping: no AVX2+VPCLMULQDQ");
            return;
        }
        for (a, b) in sample_vectors9() {
            let mut got = [0u64; 18];
            unsafe { clmul_wide9_ymm(&a, &b, &mut got) };
            assert_eq!(
                &got[..],
                scalar_ref(&a, &b).as_slice(),
                "YMM wide9 mismatch for {a:?} * {b:?}"
            );
        }
    }
}
