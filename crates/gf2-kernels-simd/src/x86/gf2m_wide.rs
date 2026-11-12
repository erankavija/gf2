//! AVX2 / PCLMULQDQ kernels for fixed-size schoolbook carry-less multiplication
//! used by `Gf2mWide` and the long product's basecase. `clmul_wide4_*`
//! (GF(2^256)), `clmul_wide8_*` and `clmul_wide9_*` (GF(2^571), 9 limbs)
//! compute the full unreduced carry-less product in the
//! little-endian limb layout of the scalar `clmul_wide_slice_portable::<N>`
//! helper: partial product `a[i] · b[j]` contributes its low/high halves to
//! `out[i + j]` / `out[i + j + 1]`.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

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
///
/// The array references fix every length and carry pointer validity and
/// exclusive access to `out`.
#[target_feature(enable = "pclmulqdq", enable = "sse4.1")]
pub unsafe fn clmul_wide4_xmm(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {
    for slot in out.iter_mut() {
        *slot = 0;
    }

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

/// Schoolbook carry-less multiply using VPCLMULQDQ on YMM (256-bit) lanes.
///
/// Each YMM register contains two independent 128-bit lanes; the
/// `_mm256_clmulepi64_epi128` intrinsic performs one clmul per lane, yielding
/// two 64×64 carry-less products per instruction. We pair the 16 scalar
/// products of the 4×4 schoolbook into 8 YMM multiplies, pairing `(i, j)` with
/// `(i, j+1)` so each pair shares the same `a[i]` operand in both lanes.
///
/// The layout matches the scalar `clmul_wide_slice_portable::<4>`: partial product
/// `a[i] · b[j]` XOR-accumulates into `out[i + j]` (lo) and `out[i + j + 1]`
/// (hi).
///
/// # Safety
///
/// Requires the `avx2` and `vpclmulqdq` CPU features.
///
/// The array references fix every length and carry pointer validity and
/// exclusive access to `out`.
#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
pub unsafe fn clmul_wide4_ymm(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {
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

        for jp in (0..4).step_by(2) {
            let b_vec = _mm256_set_epi64x(0, b[jp + 1] as i64, 0, b[jp] as i64);

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

/// 8×8 schoolbook carry-less multiply using 64 scalar PCLMULQDQ instructions.
///
/// Computes the 16-limb carry-less product of two 8-limb operands. `out` is
/// overwritten.
///
/// # Safety
///
/// Requires the `pclmulqdq` and `sse4.1` CPU features.
///
/// The array references fix every length and carry pointer validity and
/// exclusive access to `out`.
#[target_feature(enable = "pclmulqdq", enable = "sse4.1")]
pub unsafe fn clmul_wide8_xmm(a: &[u64; 8], b: &[u64; 8], out: &mut [u64; 16]) {
    let mut diag = [_mm_setzero_si128(); 15];
    for i in 0..8 {
        let ai = _mm_set_epi64x(0, a[i] as i64);
        for j in 0..8 {
            let bj = _mm_set_epi64x(0, b[j] as i64);
            diag[i + j] = _mm_xor_si128(diag[i + j], _mm_clmulepi64_si128::<0x00>(ai, bj));
        }
    }
    fold_diagonals_8x8(&diag, out);
}

/// 8×8 schoolbook carry-less multiply using 32 VPCLMULQDQ-on-YMM instructions.
///
/// `a` loads as two YMM vectors of word pairs, `(a0, a1 | a2, a3)` and
/// `(a4, a5 | a6, a7)`; each `b[j]` is broadcast, and immediates `0x00` and
/// `0x01` take the low and the high word of every 128-bit lane. One
/// instruction then yields `a[i] · b[j]` and `a[i + 2] · b[j]`, the
/// anti-diagonals `i + j` and `i + j + 2`, which accumulate in register `k =
/// i + j` holding the diagonal pair `(k, k + 2)`. `out` is overwritten.
///
/// # Safety
///
/// Requires the `avx2` and `vpclmulqdq` CPU features.
///
/// The array references fix every length and carry pointer validity and
/// exclusive access to `out`.
#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
pub unsafe fn clmul_wide8_ymm(a: &[u64; 8], b: &[u64; 8], out: &mut [u64; 16]) {
    // SAFETY: each load reads four of a's eight words.
    let (low, high) = unsafe {
        (
            _mm256_loadu_si256(a.as_ptr().cast()),
            _mm256_loadu_si256(a.as_ptr().add(4).cast()),
        )
    };
    let mut pairs = [_mm256_setzero_si256(); 13];
    macro_rules! column {
        ($($j:literal)*) => {$(
            let bj = _mm256_set1_epi64x(b[$j] as i64);
            pairs[$j] = _mm256_xor_si256(pairs[$j], _mm256_clmulepi64_epi128::<0x00>(low, bj));
            pairs[$j + 1] =
                _mm256_xor_si256(pairs[$j + 1], _mm256_clmulepi64_epi128::<0x01>(low, bj));
            pairs[$j + 4] =
                _mm256_xor_si256(pairs[$j + 4], _mm256_clmulepi64_epi128::<0x00>(high, bj));
            pairs[$j + 5] =
                _mm256_xor_si256(pairs[$j + 5], _mm256_clmulepi64_epi128::<0x01>(high, bj));
        )*};
    }
    column!(0 1 2 3 4 5 6 7);
    let mut diag = [_mm_setzero_si128(); 15];
    for (k, pair) in pairs.iter().enumerate() {
        diag[k] = _mm_xor_si128(diag[k], _mm256_castsi256_si128(*pair));
        diag[k + 2] = _mm_xor_si128(diag[k + 2], _mm256_extracti128_si256::<1>(*pair));
    }
    fold_diagonals_8x8(&diag, out);
}

/// Folds 15 anti-diagonal 128-bit sums into 16 output limbs: limb `t` is
/// `hi(diag[t - 1]) XOR lo(diag[t])`.
///
/// # Safety
///
/// The host supports SSE4.1.
#[target_feature(enable = "sse4.1")]
unsafe fn fold_diagonals_8x8(diag: &[__m128i; 15], out: &mut [u64; 16]) {
    out[0] = _mm_extract_epi64::<0>(diag[0]) as u64;
    for t in 1..15 {
        out[t] = (_mm_extract_epi64::<1>(diag[t - 1]) ^ _mm_extract_epi64::<0>(diag[t])) as u64;
    }
    out[15] = _mm_extract_epi64::<1>(diag[14]) as u64;
}

/// 9×9 schoolbook carry-less multiply using 81 scalar PCLMULQDQ instructions.
///
/// Computes the 18-limb carry-less product of two 9-limb operands. `out` is
/// cleared before accumulation.
///
/// # Safety
///
/// Requires the `pclmulqdq` and `sse4.1` CPU features.
///
/// The array references fix every length and carry pointer validity and
/// exclusive access to `out`.
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

/// 9×9 schoolbook carry-less multiply using 41 VPCLMULQDQ-on-YMM
/// instructions.
///
/// `a[0..8]` loads as two YMM vectors of word pairs and each `b[j]` is
/// broadcast, as in [`clmul_wide8_ymm`]: 36 instructions give `a[i] · b[j]`
/// for `i < 8`. A broadcast `a[8]` against the loads `b[0..4]` and `b[4..8]`
/// under immediates `0x00` and `0x10` gives `a[8] · b[j]` for `j < 8` in four
/// more, and one gives `a[8] · b[8]`. Register `k` accumulates the
/// anti-diagonal pair `(k, k + 2)`. `out` is overwritten.
///
/// # Safety
///
/// Requires the `avx2` and `vpclmulqdq` CPU features.
///
/// The array references fix every length and carry pointer validity and
/// exclusive access to `out`.
#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
pub unsafe fn clmul_wide9_ymm(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {
    // SAFETY: each load reads four of a's or b's first eight words.
    let (low, high, b_low, b_high) = unsafe {
        (
            _mm256_loadu_si256(a.as_ptr().cast()),
            _mm256_loadu_si256(a.as_ptr().add(4).cast()),
            _mm256_loadu_si256(b.as_ptr().cast()),
            _mm256_loadu_si256(b.as_ptr().add(4).cast()),
        )
    };
    let mut pairs = [_mm256_setzero_si256(); 14];
    macro_rules! column {
        ($($j:literal)*) => {$(
            let bj = _mm256_set1_epi64x(b[$j] as i64);
            pairs[$j] = _mm256_xor_si256(pairs[$j], _mm256_clmulepi64_epi128::<0x00>(low, bj));
            pairs[$j + 1] =
                _mm256_xor_si256(pairs[$j + 1], _mm256_clmulepi64_epi128::<0x01>(low, bj));
            pairs[$j + 4] =
                _mm256_xor_si256(pairs[$j + 4], _mm256_clmulepi64_epi128::<0x00>(high, bj));
            pairs[$j + 5] =
                _mm256_xor_si256(pairs[$j + 5], _mm256_clmulepi64_epi128::<0x01>(high, bj));
        )*};
    }
    column!(0 1 2 3 4 5 6 7 8);
    // a[8] · (b[j], b[j + 2]) and a[8] · (b[j + 1], b[j + 3]) land on the
    // pairs (8 + j, 10 + j) and (9 + j, 11 + j).
    let a8 = _mm256_set1_epi64x(a[8] as i64);
    pairs[8] = _mm256_xor_si256(pairs[8], _mm256_clmulepi64_epi128::<0x00>(a8, b_low));
    pairs[9] = _mm256_xor_si256(pairs[9], _mm256_clmulepi64_epi128::<0x10>(a8, b_low));
    pairs[12] = _mm256_xor_si256(pairs[12], _mm256_clmulepi64_epi128::<0x00>(a8, b_high));
    pairs[13] = _mm256_xor_si256(pairs[13], _mm256_clmulepi64_epi128::<0x10>(a8, b_high));
    let last = _mm256_clmulepi64_epi128::<0x00>(a8, _mm256_set1_epi64x(b[8] as i64));
    let mut diag = [_mm_setzero_si128(); 17];
    for (k, pair) in pairs.iter().enumerate() {
        diag[k] = _mm_xor_si128(diag[k], _mm256_castsi256_si128(*pair));
        diag[k + 2] = _mm_xor_si128(diag[k + 2], _mm256_extracti128_si256::<1>(*pair));
    }
    diag[16] = _mm_xor_si128(diag[16], _mm256_castsi256_si128(last));
    fold_diagonals_9x9(diag, out);
}

/// Fold anti-diagonal 128-bit products into little-endian output limbs.
///
/// `diag[k]` holds the XOR of all 128-bit word products with `i + j == k`.
/// Therefore output limb `t` is `hi(diag[t - 1]) XOR lo(diag[t])`.
///
/// # Safety
///
/// The host supports SSE4.1. `diag` is a value and `out` a fixed-size
/// exclusive reference, so no pointer, length or aliasing condition applies.
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
            ([0, 0, 0, 0, 0, 0, 0, 0, u64::MAX], [u64::MAX; 9]),
            ([u64::MAX; 9], [0, 0, 0, 0, 0, 0, 0, 0, u64::MAX]),
        ]
        .into_iter()
        .chain((0..16u64).map(|seed| {
            let mut state = 0x9e37_79b9_7f4a_7c15 ^ seed;
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            (
                core::array::from_fn(|_| next()),
                core::array::from_fn(|_| next()),
            )
        }))
        .collect()
    }

    #[test]
    fn xmm_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        crate::contract::require(&[(
            "pclmulqdq+sse4.1",
            is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1"),
        )]);
        for (a, b) in sample_vectors4() {
            let mut got = [0u64; 8];
            // SAFETY: PCLMULQDQ and SSE4.1 were detected above.
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
        crate::contract::require(&[(
            "avx2+vpclmulqdq",
            is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq"),
        )]);
        for (a, b) in sample_vectors4() {
            let mut got = [0u64; 8];
            // SAFETY: AVX2 and VPCLMULQDQ were detected above.
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
        crate::contract::require(&[(
            "pclmulqdq+sse4.1",
            is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1"),
        )]);
        for (a, b) in sample_vectors9() {
            let mut got = [0u64; 18];
            // SAFETY: PCLMULQDQ and SSE4.1 were detected above.
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
        crate::contract::require(&[(
            "avx2+vpclmulqdq",
            is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq"),
        )]);
        for (a, b) in sample_vectors9() {
            let mut got = [0u64; 18];
            // SAFETY: AVX2 and VPCLMULQDQ were detected above.
            unsafe { clmul_wide9_ymm(&a, &b, &mut got) };
            assert_eq!(
                &got[..],
                scalar_ref(&a, &b).as_slice(),
                "YMM wide9 mismatch for {a:?} * {b:?}"
            );
        }
    }

    fn sample_vectors8() -> Vec<([u64; 8], [u64; 8])> {
        let mut state = 0x5eed_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut pairs = vec![
            ([0; 8], [0; 8]),
            ([1, 0, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0, 0]),
            (
                [0, 0, 0, 0, 0, 0, 0, 1 << 63],
                [0, 0, 0, 0, 0, 0, 0, 1 << 63],
            ),
            ([u64::MAX; 8], [u64::MAX; 8]),
        ];
        for _ in 0..16 {
            pairs.push((
                core::array::from_fn(|_| next()),
                core::array::from_fn(|_| next()),
            ));
        }
        pairs
    }

    #[test]
    fn xmm_wide8_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        crate::contract::require(&[(
            "pclmulqdq+sse4.1",
            is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1"),
        )]);
        for (a, b) in sample_vectors8() {
            let mut got = [u64::MAX; 16];
            // SAFETY: PCLMULQDQ and SSE4.1 were detected above.
            unsafe { clmul_wide8_xmm(&a, &b, &mut got) };
            assert_eq!(&got[..], scalar_ref(&a, &b).as_slice(), "{a:?} * {b:?}");
        }
    }

    #[test]
    fn ymm_wide8_matches_scalar_reference() {
        use std::arch::is_x86_feature_detected;
        crate::contract::require(&[(
            "avx2+vpclmulqdq",
            is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq"),
        )]);
        for (a, b) in sample_vectors8() {
            let mut got = [u64::MAX; 16];
            // SAFETY: AVX2 and VPCLMULQDQ were detected above.
            unsafe { clmul_wide8_ymm(&a, &b, &mut got) };
            assert_eq!(&got[..], scalar_ref(&a, &b).as_slice(), "{a:?} * {b:?}");
        }
    }
}
