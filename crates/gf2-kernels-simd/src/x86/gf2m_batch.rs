//! AVX2 + VPCLMULQDQ batch element-wise GF(2^m) multiply/square kernels for
//! `m ∈ {8, 16, 32}`. Each outer iteration processes 4 elements with the
//! Barrett reduction kept in YMM registers; tail elements take a scalar
//! PCLMULQDQ path.
//!
//! # Safety
//!
//! All entry points carry `#[target_feature(enable = "avx2",
//! enable = "vpclmulqdq", enable = "pclmulqdq", enable = "sse4.1")]`;
//! `crate::gf2m_batch::detect` publishes function pointers only when all four
//! features are present.

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use super::gf2m_common::{clmul_barrett_scalar as clmul_barrett_reduce_inline, ymm_barrett_reduce};

/// Load 2 elements into a `__m256i` placing each in the low 64 bits of its
/// 128-bit lane.
///
/// # Safety
///
/// The host supports AVX2, VPCLMULQDQ, PCLMULQDQ and SSE4.1. Every argument is
/// a value, so no pointer, length or aliasing condition applies.
#[inline(always)]
unsafe fn pack_pair(x0: u64, x1: u64) -> __m256i {
    _mm256_set_epi64x(0, x1 as i64, 0, x0 as i64)
}

/// Produce two `__m256i` registers each holding two `u64` operands placed
/// in the low half of their 128-bit lane.
///
/// # Safety
///
/// The host supports AVX2, VPCLMULQDQ, PCLMULQDQ and SSE4.1. Every argument is
/// a value, so no pointer, length or aliasing condition applies.
#[inline(always)]
unsafe fn pack_quad(x0: u64, x1: u64, x2: u64, x3: u64) -> (__m256i, __m256i) {
    (pack_pair(x0, x1), pack_pair(x2, x3))
}

/// Extract the low 64 bits of each 128-bit lane of two YMM registers.
///
/// # Safety
///
/// The host supports AVX2, VPCLMULQDQ, PCLMULQDQ and SSE4.1. Every argument is
/// a value, so no pointer, length or aliasing condition applies.
#[inline(always)]
unsafe fn extract_quad_lo(r_lo: __m256i, r_hi: __m256i) -> (u64, u64, u64, u64) {
    let lane0 = _mm256_extracti128_si256::<0>(r_lo);
    let lane1 = _mm256_extracti128_si256::<1>(r_lo);
    let lane2 = _mm256_extracti128_si256::<0>(r_hi);
    let lane3 = _mm256_extracti128_si256::<1>(r_hi);
    (
        _mm_extract_epi64::<0>(lane0) as u64,
        _mm_extract_epi64::<0>(lane1) as u64,
        _mm_extract_epi64::<0>(lane2) as u64,
        _mm_extract_epi64::<0>(lane3) as u64,
    )
}

/// Extract the high 64 bits of each 128-bit lane of two YMM registers.
///
/// # Safety
///
/// The host supports AVX2, VPCLMULQDQ, PCLMULQDQ and SSE4.1. Every argument is
/// a value, so no pointer, length or aliasing condition applies.
#[allow(dead_code)]
#[inline(always)]
unsafe fn extract_quad_hi(r_lo: __m256i, r_hi: __m256i) -> (u64, u64, u64, u64) {
    let lane0 = _mm256_extracti128_si256::<0>(r_lo);
    let lane1 = _mm256_extracti128_si256::<1>(r_lo);
    let lane2 = _mm256_extracti128_si256::<0>(r_hi);
    let lane3 = _mm256_extracti128_si256::<1>(r_hi);
    (
        _mm_extract_epi64::<1>(lane0) as u64,
        _mm_extract_epi64::<1>(lane1) as u64,
        _mm_extract_epi64::<1>(lane2) as u64,
        _mm_extract_epi64::<1>(lane3) as u64,
    )
}

/// Inner loop body: process one block of 4 elements via YMM-resident
/// reduction and the static byte-shift `SHIFT`. Caller selects `SHIFT` from
/// `degree / 8`.
///
/// # Safety
///
/// The host supports AVX2, VPCLMULQDQ, PCLMULQDQ and SSE4.1. Every argument is
/// a value, so no pointer, length or aliasing condition applies.
#[inline(always)]
unsafe fn process_quad_mul<const SHIFT: i32>(
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    b0: u64,
    b1: u64,
    b2: u64,
    b3: u64,
    mu_ymm: __m256i,
    mod_ymm: __m256i,
    mask: u64,
) -> [u64; 4] {
    let (a_lo, a_hi) = pack_quad(a0, a1, a2, a3);
    let (b_lo, b_hi) = pack_quad(b0, b1, b2, b3);

    let prod_lo = _mm256_clmulepi64_epi128::<0x00>(a_lo, b_lo);
    let prod_hi = _mm256_clmulepi64_epi128::<0x00>(a_hi, b_hi);

    let (r_lo, r_hi) = ymm_barrett_reduce::<SHIFT>(prod_lo, prod_hi, mu_ymm, mod_ymm);

    let (r0, r1, r2, r3) = extract_quad_lo(r_lo, r_hi);
    [
        correct(r0, modulus_from_ymm(mod_ymm), SHIFT, mask),
        correct(r1, modulus_from_ymm(mod_ymm), SHIFT, mask),
        correct(r2, modulus_from_ymm(mod_ymm), SHIFT, mask),
        correct(r3, modulus_from_ymm(mod_ymm), SHIFT, mask),
    ]
}

/// Recover `modulus` from the broadcast YMM-packed copy. The constant lives
/// in the low 64 bits of lane 0; this avoids passing it as a separate
/// scalar through every helper.
///
/// # Safety
///
/// The host supports AVX2, VPCLMULQDQ, PCLMULQDQ and SSE4.1. Every argument is
/// a value, so no pointer, length or aliasing condition applies.
#[inline(always)]
unsafe fn modulus_from_ymm(mod_ymm: __m256i) -> u64 {
    let lane0 = _mm256_extracti128_si256::<0>(mod_ymm);
    _mm_extract_epi64::<0>(lane0) as u64
}

use super::gf2m_common::correct;

/// Batch element-wise multiply, 4-way unrolled YMM-resident Barrett.
///
/// Processes blocks of 4 elements per outer iteration with the reduction
/// kept in YMM registers. Tail elements are handled by
/// [`clmul_barrett_reduce_inline`].
///
/// # Safety
/// Requires `avx2`, `vpclmulqdq`, `pclmulqdq`, and `sse4.1` CPU features,
/// and `degree ∈ {8, 16, 32}`.
///
/// The length assertions bound every load and store, and the slice references
/// carry pointer validity and exclusive access to `out`.
#[target_feature(
    enable = "avx2",
    enable = "vpclmulqdq",
    enable = "pclmulqdq",
    enable = "sse4.1"
)]
pub unsafe fn gf2m_batch_mul_ymm_unroll4(
    a: &[u64],
    b: &[u64],
    out: &mut [u64],
    mu: u64,
    modulus: u64,
    degree: u32,
) {
    assert_eq!(a.len(), b.len(), "input slices must have equal length");
    assert_eq!(
        a.len(),
        out.len(),
        "output slice must match input slice length"
    );
    debug_assert!(
        matches!(degree, 8 | 16 | 32),
        "gf2m_batch kernel only supports m ∈ {{8, 16, 32}}; got {degree}"
    );

    let n = a.len();
    let mask = if degree == 64 {
        u64::MAX
    } else {
        (1u64 << degree) - 1
    };

    let mu_ymm = _mm256_set_epi64x(0, mu as i64, 0, mu as i64);
    let mod_ymm = _mm256_set_epi64x(0, modulus as i64, 0, modulus as i64);

    let mut i = 0usize;
    match degree {
        8 => {
            while i + 4 <= n {
                let r = process_quad_mul::<1>(
                    a[i],
                    a[i + 1],
                    a[i + 2],
                    a[i + 3],
                    b[i],
                    b[i + 1],
                    b[i + 2],
                    b[i + 3],
                    mu_ymm,
                    mod_ymm,
                    mask,
                );
                out[i..i + 4].copy_from_slice(&r);
                i += 4;
            }
        }
        16 => {
            while i + 4 <= n {
                let r = process_quad_mul::<2>(
                    a[i],
                    a[i + 1],
                    a[i + 2],
                    a[i + 3],
                    b[i],
                    b[i + 1],
                    b[i + 2],
                    b[i + 3],
                    mu_ymm,
                    mod_ymm,
                    mask,
                );
                out[i..i + 4].copy_from_slice(&r);
                i += 4;
            }
        }
        32 => {
            while i + 4 <= n {
                let r = process_quad_mul::<4>(
                    a[i],
                    a[i + 1],
                    a[i + 2],
                    a[i + 3],
                    b[i],
                    b[i + 1],
                    b[i + 2],
                    b[i + 3],
                    mu_ymm,
                    mod_ymm,
                    mask,
                );
                out[i..i + 4].copy_from_slice(&r);
                i += 4;
            }
        }
        _ => {
            // Other degrees fall through entirely to the scalar tail path.
        }
    }

    while i < n {
        out[i] = clmul_barrett_reduce_inline(a[i], b[i], mu, modulus, degree);
        i += 1;
    }
}

/// Batch element-wise square. Specialisation of [`gf2m_batch_mul_ymm_unroll4`]
/// where `b == a`.
///
/// # Safety
/// Same as [`gf2m_batch_mul_ymm_unroll4`].
#[target_feature(
    enable = "avx2",
    enable = "vpclmulqdq",
    enable = "pclmulqdq",
    enable = "sse4.1"
)]
pub unsafe fn gf2m_batch_square_ymm_unroll4(
    a: &[u64],
    out: &mut [u64],
    mu: u64,
    modulus: u64,
    degree: u32,
) {
    assert_eq!(
        a.len(),
        out.len(),
        "output slice must match input slice length"
    );
    debug_assert!(
        matches!(degree, 8 | 16 | 32),
        "gf2m_batch kernel only supports m ∈ {{8, 16, 32}}; got {degree}"
    );

    gf2m_batch_mul_ymm_unroll4(a, a, out, mu, modulus, degree);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_mu(modulus: u64, degree: u32) -> u64 {
        let mut remainder: u128 = 1u128 << (2 * degree);
        let mut mu: u64 = 0;
        let p = modulus as u128;
        for i in (0..=degree).rev() {
            let bit_pos = degree + i;
            if (remainder >> bit_pos) & 1 == 1 {
                mu |= 1u64 << i;
                remainder ^= p << i;
            }
        }
        mu
    }

    #[test]
    fn unroll4_mul_matches_singleshot_gf256() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq")) {
            eprintln!("skipping: no AVX2+VPCLMULQDQ");
            return;
        }

        let m: u32 = 8;
        let poly: u64 = 0b100011101;
        let mu = compute_mu(poly, m);
        let mask = (1u64 << m) - 1;

        let a: Vec<u64> = (0..64u64).map(|i| (i * 0x9E37_79B9) & mask).collect();
        let b: Vec<u64> = (0..64u64).map(|i| (i * 0x6C62_272E + 7) & mask).collect();
        let mut got = vec![0u64; 64];
        // SAFETY: AVX2 and VPCLMULQDQ were detected above. In rustc's
        // target-feature hierarchy they enable PCLMULQDQ and SSE4.1.
        unsafe { gf2m_batch_mul_ymm_unroll4(&a, &b, &mut got, mu, poly, m) };

        for i in 0..64 {
            let expected =
                // SAFETY: AVX2 and VPCLMULQDQ were detected above. In rustc's
                // target-feature hierarchy they enable PCLMULQDQ and SSE4.1.
                unsafe { crate::x86::clmul::clmul_barrett_reduce(a[i], b[i], mu, poly, m) };
            assert_eq!(got[i], expected, "mismatch at i={i}, m={m}");
        }
    }

    #[test]
    fn unroll4_mul_matches_singleshot_gf2_16() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq")) {
            return;
        }
        let m: u32 = 16;
        let poly: u64 = 0b1_0001_0000_0000_1011;
        let mu = compute_mu(poly, m);
        let mask = (1u64 << m) - 1;

        let a: Vec<u64> = (0..32u64).map(|i| (i * 0xABCD) & mask).collect();
        let b: Vec<u64> = (0..32u64).map(|i| (i * 0x1234 + 5) & mask).collect();
        let mut got = vec![0u64; 32];
        // SAFETY: AVX2 and VPCLMULQDQ were detected above. In rustc's
        // target-feature hierarchy they enable PCLMULQDQ and SSE4.1.
        unsafe { gf2m_batch_mul_ymm_unroll4(&a, &b, &mut got, mu, poly, m) };
        for i in 0..32 {
            let expected =
                // SAFETY: AVX2 and VPCLMULQDQ were detected above. In rustc's
                // target-feature hierarchy they enable PCLMULQDQ and SSE4.1.
                unsafe { crate::x86::clmul::clmul_barrett_reduce(a[i], b[i], mu, poly, m) };
            assert_eq!(got[i], expected, "i={i}");
        }
    }

    #[test]
    fn unroll4_mul_matches_singleshot_gf2_32() {
        use std::arch::is_x86_feature_detected;
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq")) {
            return;
        }
        let m: u32 = 32;
        let poly: u64 = 0b1_0000_0000_0100_0000_0000_0000_0000_0111;
        let mu = compute_mu(poly, m);
        let mask = (1u64 << m) - 1;

        let a: Vec<u64> = (0..32u64)
            .map(|i| (i + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask)
            .collect();
        let b: Vec<u64> = (0..32u64)
            .map(|i| (i + 1).wrapping_mul(0x6C62_272E_07BB_0142) & mask)
            .collect();
        let mut got = vec![0u64; 32];
        // SAFETY: AVX2 and VPCLMULQDQ were detected above. In rustc's
        // target-feature hierarchy they enable PCLMULQDQ and SSE4.1.
        unsafe { gf2m_batch_mul_ymm_unroll4(&a, &b, &mut got, mu, poly, m) };
        for i in 0..32 {
            let expected =
                // SAFETY: AVX2 and VPCLMULQDQ were detected above. In rustc's
                // target-feature hierarchy they enable PCLMULQDQ and SSE4.1.
                unsafe { crate::x86::clmul::clmul_barrett_reduce(a[i], b[i], mu, poly, m) };
            assert_eq!(got[i], expected, "i={i}");
        }
    }
}
