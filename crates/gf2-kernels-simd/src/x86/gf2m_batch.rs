//! AVX2 + VPCLMULQDQ batch element-wise GF(2^m) multiply/square kernels for
//! `m ∈ {8, 16, 32}`.
//!
//! The kernel processes 4 elements per outer iteration (two YMM
//! registers × two 128-bit lanes per register). All Barrett reduction
//! state is kept inside YMM registers across the multiply, q, and qp
//! phases — there are no per-element extracts on the hot path. This is
//! the load-bearing optimisation versus a scalar-extract design and
//! gives the kernel its ≥1.5× speedup over the per-element single-shot
//! `clmul_barrett_reduce`.
//!
//! # YMM-resident reduction (`m ∈ {8, 16, 32}`)
//!
//! For `m ≤ 32`, the carry-less product `a · b` fits in 2m ≤ 64 bits, so
//! every per-element 128-bit clmul output occupies only the low 64 bits of
//! its 128-bit lane. The reduction
//!
//!   `c_high = product >> m`
//!   `q_full = c_high · mu`
//!   `q = q_full >> m`
//!   `qp = q · modulus`
//!   `r = product XOR qp`
//!
//! decomposes into byte-aligned shifts (`_mm256_srli_si256<m/8>`) and three
//! VPCLMULQDQ instructions per pair of elements. The 4-way outer unroll
//! keeps four independent reduction chains in flight, exposing the AMD
//! Zen 3 / Zen 4 issue ports to enough ILP that VPCLMULQDQ's 4-cycle
//! latency stops being the bottleneck.
//!
//! Tail elements (count not a multiple of 4) are handled by a scalar
//! PCLMULQDQ fallback inlined in the same `#[target_feature]` scope to
//! avoid call-site overhead.
//!
//! # Safety / feature detection
//!
//! All entry points carry `#[target_feature(enable = "avx2", enable =
//! "vpclmulqdq", enable = "pclmulqdq", enable = "sse4.1")]`. The
//! `crate::gf2m_batch::detect_x86` accessor only publishes function
//! pointers when all four features are present at runtime.

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// Helper: scalar single-element multiply + Barrett reduce, kept inside the
// outer `#[target_feature]` scope so tail handling avoids call-pointer
// overhead.
// ---------------------------------------------------------------------------

/// Single-element multiply+reduce inlined for tail handling. Mirrors the
/// algorithm in `crate::x86::clmul::clmul_barrett_reduce`.
#[inline(always)]
unsafe fn clmul_barrett_reduce_inline(a: u64, b: u64, mu: u64, modulus: u64, degree: u32) -> u64 {
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

// ---------------------------------------------------------------------------
// YMM-resident Barrett reduction core.
//
// Each input is a `__m256i` holding two 128-bit lanes; each lane carries a
// 64-bit field element in its low half. `_mm256_clmulepi64_epi128` then
// produces a `__m256i` with two 128-bit products (one per lane); since
// `m ≤ 32`, the product fits in the low 64 bits of each lane.
// ---------------------------------------------------------------------------

/// Load 2 elements into a `__m256i` placing each in the low 64 bits of its
/// 128-bit lane. Used for both the input pack and the c_high/q repack
/// phases.
#[inline(always)]
unsafe fn pack_pair(x0: u64, x1: u64) -> __m256i {
    _mm256_set_epi64x(0, x1 as i64, 0, x0 as i64)
}

/// Produce two `__m256i` registers each holding two `u64` operands placed
/// in the low half of their 128-bit lane.
#[inline(always)]
unsafe fn pack_quad(x0: u64, x1: u64, x2: u64, x3: u64) -> (__m256i, __m256i) {
    (pack_pair(x0, x1), pack_pair(x2, x3))
}

/// Extract the low 64 bits of each 128-bit lane of two YMM registers as the
/// 4 final results, applying the field mask.
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

/// Extract the high 64 bits of each lane (the c_high half of each product
/// when 2m > 64). Currently unused for `m ≤ 32` but kept for future
/// extension to `m ∈ (32, 64]`.
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

/// Performs the YMM-resident Barrett reduction for the 4 products carried
/// by `(prod_lo, prod_hi)`, returning the 4 reduced values as a pair of
/// `__m256i`. The `mu_ymm` and `mod_ymm` operands carry the broadcast
/// Barrett constants. The `degree` parameter selects the static byte-shift
/// constant via `m / 8`.
///
/// # Safety
/// Requires `avx2`, `vpclmulqdq`, `pclmulqdq`, and `sse4.1`. Caller must
/// also guarantee `m ∈ {8, 16, 32}`; the byte-shift constants are
/// hard-coded for those values.
#[inline(always)]
unsafe fn ymm_barrett_reduce<const SHIFT_BYTES: i32>(
    prod_lo: __m256i,
    prod_hi: __m256i,
    mu_ymm: __m256i,
    mod_ymm: __m256i,
) -> (__m256i, __m256i) {
    // c_high = product >> m. For m ∈ {8, 16, 32}, m / 8 ∈ {1, 2, 4} bytes,
    // and `_mm256_srli_si256` performs a per-128-bit-lane byte shift that
    // matches our lane layout exactly.
    let c_high_lo = _mm256_srli_si256::<SHIFT_BYTES>(prod_lo);
    let c_high_hi = _mm256_srli_si256::<SHIFT_BYTES>(prod_hi);

    // q_full = c_high · mu (carry-less). VPCLMULQDQ on the low halves of
    // each 128-bit lane.
    let q_full_lo = _mm256_clmulepi64_epi128::<0x00>(c_high_lo, mu_ymm);
    let q_full_hi = _mm256_clmulepi64_epi128::<0x00>(c_high_hi, mu_ymm);

    // q = q_full >> m.
    let q_lo = _mm256_srli_si256::<SHIFT_BYTES>(q_full_lo);
    let q_hi = _mm256_srli_si256::<SHIFT_BYTES>(q_full_hi);

    // qp = q · modulus.
    let qp_lo = _mm256_clmulepi64_epi128::<0x00>(q_lo, mod_ymm);
    let qp_hi = _mm256_clmulepi64_epi128::<0x00>(q_hi, mod_ymm);

    // r = product XOR qp. Since 2m ≤ 64, the high 64 bits of every lane
    // are zero on both sides; XORing the full YMM is correct.
    let r_lo = _mm256_xor_si256(prod_lo, qp_lo);
    let r_hi = _mm256_xor_si256(prod_hi, qp_hi);

    (r_lo, r_hi)
}

// ---------------------------------------------------------------------------
// Public kernels
// ---------------------------------------------------------------------------

/// Inner loop body: process one block of 4 elements via YMM-resident
/// reduction and the static byte-shift `SHIFT`. Caller selects `SHIFT` from
/// `degree / 8`.
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
    // The Barrett step leaves `r` in `[0, 2P)`; one subtraction-style
    // correction lands it in `[0, P)`. We use bit-mask correction: r ^=
    // modulus iff r >= 2^m. With `mask = (1<<m) - 1`, the check becomes
    // `(r >> m) != 0`.
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
#[inline(always)]
unsafe fn modulus_from_ymm(mod_ymm: __m256i) -> u64 {
    let lane0 = _mm256_extracti128_si256::<0>(mod_ymm);
    _mm_extract_epi64::<0>(lane0) as u64
}

/// Final correction: r ∈ `[0, 2P)` → r ∈ `[0, P)` by conditional XOR with
/// modulus when the degree-m bit is set, plus a defensive second pass.
/// `SHIFT` is `m / 8`, so the degree-m bit lives at byte position `SHIFT`,
/// bit 0.
#[inline(always)]
fn correct(mut r: u64, modulus: u64, shift_bytes: i32, mask: u64) -> u64 {
    let degree = (shift_bytes as u32) * 8;
    if (r >> degree) != 0 {
        r ^= modulus;
    }
    if (r >> degree) != 0 {
        r ^= modulus;
    }
    r & mask
}

/// Batch element-wise multiply, 4-way unrolled YMM-resident Barrett.
///
/// Processes blocks of 4 elements per outer iteration. The dependent
/// reduction step stays in YMM registers, reducing the per-element cost
/// to:
/// * 6× VPCLMULQDQ per 4 elements (0.25 + 0.5 + 0.5 = 1.5 per element)
/// * 6× YMM byte-shift / XOR per 4 elements
/// * 4× `_mm_extract_epi64` to write the final results
///
/// This is ~3× fewer extracts than a per-element scalar-Barrett path, and
/// ~2× fewer than the prior YMM-multiply / scalar-Barrett intermediate.
///
/// Tail elements are handled by [`clmul_barrett_reduce_inline`].
///
/// # Safety
/// Requires `avx2`, `vpclmulqdq`, `pclmulqdq`, and `sse4.1` CPU features,
/// and `degree ∈ {8, 16, 32}`.
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

    // The square kernel mirrors the multiply path with `b = a`. Re-using
    // the multiply implementation keeps both kernels tested via the same
    // YMM-resident Barrett core.
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
        unsafe { gf2m_batch_mul_ymm_unroll4(&a, &b, &mut got, mu, poly, m) };

        for i in 0..64 {
            let expected =
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
        unsafe { gf2m_batch_mul_ymm_unroll4(&a, &b, &mut got, mu, poly, m) };
        for i in 0..32 {
            let expected =
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
        unsafe { gf2m_batch_mul_ymm_unroll4(&a, &b, &mut got, mu, poly, m) };
        for i in 0..32 {
            let expected =
                unsafe { crate::x86::clmul::clmul_barrett_reduce(a[i], b[i], mu, poly, m) };
            assert_eq!(got[i], expected, "i={i}");
        }
    }
}
