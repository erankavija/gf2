//! Raw carry-less multiplication kernels using PCLMULQDQ and VPCLMULQDQ.
//!
//! These kernels perform the raw polynomial multiplication step only (no reduction).
//! Reduction is handled by the caller (e.g., Barrett reduction in `gf2-core`).

/// Raw carry-less multiplication of two 64-bit GF(2) polynomials.
///
/// Returns the full 128-bit product `a(x) * b(x)` with no modular reduction.
/// The result can have degree up to `deg(a) + deg(b)` (at most 126 for two 63-degree inputs).
///
/// # Arguments
///
/// * `a` - First polynomial (up to 64 bits).
/// * `b` - Second polynomial (up to 64 bits).
///
/// # Returns
///
/// The full 128-bit carry-less product.
///
/// # Safety
///
/// Requires the PCLMULQDQ CPU feature. Caller must verify availability before calling.
///
/// # Usage
///
/// This is a crate-internal function exposed through the safe `Gf2mFns` dispatch:
///
/// ```text
/// let fns = gf2_kernels_simd::gf2m::detect().unwrap();
/// let product = (fns.clmul_fn.unwrap())(a, b);
/// ```
///
/// # Complexity
///
/// O(1) -- single hardware instruction.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
pub unsafe fn clmul_u64(a: u64, b: u64) -> u128 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let a_reg = _mm_set_epi64x(0, a as i64);
    let b_reg = _mm_set_epi64x(0, b as i64);
    let product = _mm_clmulepi64_si128::<0x00>(a_reg, b_reg);

    let lo = _mm_extract_epi64::<0>(product) as u64;
    let hi = _mm_extract_epi64::<1>(product) as u64;

    (hi as u128) << 64 | (lo as u128)
}

/// Batch carry-less multiplication of aligned slices.
///
/// Computes `out[i] = a[i] * b[i]` (carry-less, no reduction) for each index.
/// Uses VPCLMULQDQ when available for higher throughput, falling back to
/// sequential PCLMULQDQ otherwise.
///
/// # Arguments
///
/// * `a` - First operand slice.
/// * `b` - Second operand slice. Must have the same length as `a`.
/// * `out` - Output slice. Must have the same length as `a`.
///
/// # Panics
///
/// Panics if slices have different lengths.
///
/// # Safety
///
/// Requires the PCLMULQDQ CPU feature. Caller must verify availability before calling.
///
/// # Usage
///
/// This is a crate-internal function exposed through the safe `Gf2mFns` dispatch:
///
/// ```text
/// let fns = gf2_kernels_simd::gf2m::detect().unwrap();
/// let batch_fn = fns.clmul_batch_fn.unwrap();
/// let mut out = vec![0u128; a.len()];
/// batch_fn(&a, &b, &mut out);
/// ```
///
/// # Complexity
///
/// O(n) where n is the slice length. With VPCLMULQDQ (256-bit), processes 2 elements per instruction.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
pub unsafe fn clmul_batch(a: &[u64], b: &[u64], out: &mut [u128]) {
    assert_eq!(a.len(), b.len(), "input slices must have equal length");
    assert_eq!(
        a.len(),
        out.len(),
        "output slice must have same length as inputs"
    );

    // Try VPCLMULQDQ for 2-wide processing (two 128-bit lanes per 256-bit register)
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        use std::arch::is_x86_feature_detected;
        if is_x86_feature_detected!("vpclmulqdq") && is_x86_feature_detected!("avx512vl") {
            clmul_batch_vpclmul(a, b, out);
            return;
        }
    }

    // Fallback: sequential PCLMULQDQ
    clmul_batch_sequential(a, b, out);
}

/// Sequential PCLMULQDQ fallback for batch carry-less multiplication.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
unsafe fn clmul_batch_sequential(a: &[u64], b: &[u64], out: &mut [u128]) {
    for i in 0..a.len() {
        out[i] = clmul_u64(a[i], b[i]);
    }
}

/// VPCLMULQDQ batch carry-less multiplication for 2x throughput.
///
/// Processes 2 carry-less multiplications per 256-bit VPCLMULQDQ instruction.
/// Each `__m256i` holds two 64-bit operands in the low halves of its 128-bit lanes.
///
/// # Safety
///
/// Requires AVX512VL and VPCLMULQDQ CPU features.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx512vl", enable = "vpclmulqdq")]
unsafe fn clmul_batch_vpclmul(a: &[u64], b: &[u64], out: &mut [u128]) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = a.len();
    let mut i = 0;

    // Process 2 elements at a time using 256-bit VPCLMULQDQ.
    // Each __m256i has two 128-bit lanes; we put one operand pair in each lane.
    while i + 2 <= n {
        // Pack two operand pairs into __m256i:
        // lane 0: a[i], lane 1: a[i+1]  (each in the low 64 bits of 128-bit lane)
        let a_vec = _mm256_set_epi64x(0, a[i + 1] as i64, 0, a[i] as i64);
        let b_vec = _mm256_set_epi64x(0, b[i + 1] as i64, 0, b[i] as i64);

        // VPCLMULQDQ: carry-less multiply low 64 bits of each 128-bit lane
        let product = _mm256_clmulepi64_epi128::<0x00>(a_vec, b_vec);

        // Extract results from each 128-bit lane
        let lo_lane = _mm256_extracti128_si256::<0>(product);
        let hi_lane = _mm256_extracti128_si256::<1>(product);

        let lo0 = _mm_extract_epi64::<0>(lo_lane) as u64;
        let hi0 = _mm_extract_epi64::<1>(lo_lane) as u64;
        out[i] = (hi0 as u128) << 64 | (lo0 as u128);

        let lo1 = _mm_extract_epi64::<0>(hi_lane) as u64;
        let hi1 = _mm_extract_epi64::<1>(hi_lane) as u64;
        out[i + 1] = (hi1 as u128) << 64 | (lo1 as u128);

        i += 2;
    }

    // Handle remaining element (if odd count)
    while i < n {
        out[i] = clmul_u64(a[i], b[i]);
        i += 1;
    }
}

/// Carry-less multiplication with Barrett reduction, all in one PCLMULQDQ pass.
///
/// Performs: `a * b mod P(x)` using Barrett reduction with precomputed `mu`.
/// All three carry-less multiplications use PCLMULQDQ, keeping values in
/// SIMD registers to avoid function-pointer call overhead.
///
/// # Arguments
///
/// * `a` - First field element (m bits).
/// * `b` - Second field element (m bits).
/// * `mu` - Barrett constant `x^(2m) / P(x)`, fits in `u64` for `m <= 63`.
/// * `modulus` - Irreducible polynomial `P(x)`, fits in `u64` for `m <= 63`.
/// * `degree` - Field degree m.
///
/// # Returns
///
/// The reduced product `a(x) * b(x) mod P(x)`, fitting in m bits.
///
/// # Safety
///
/// Requires the PCLMULQDQ CPU feature.
///
/// # Usage
///
/// This is a crate-internal function exposed through the safe `Gf2mFns` dispatch:
///
/// ```text
/// let fns = gf2_kernels_simd::gf2m::detect().unwrap();
/// // GF(2^4) with primitive polynomial x^4 + x + 1 = 0b10011
/// let product = (fns.clmul_reduce_fn.unwrap())(0b1010, 0b1100, mu, 0b10011, 4);
/// ```
///
/// # Complexity
///
/// O(1) — three PCLMULQDQ instructions plus constant-time correction.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
pub unsafe fn clmul_barrett_reduce(a: u64, b: u64, mu: u64, modulus: u64, degree: u32) -> u64 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    if a == 0 || b == 0 {
        return 0;
    }

    let field_mask = (1u64 << degree) - 1;

    // Step 1: raw product = a clmul b
    let a_reg = _mm_set_epi64x(0, a as i64);
    let b_reg = _mm_set_epi64x(0, b as i64);
    let product_reg = _mm_clmulepi64_si128::<0x00>(a_reg, b_reg);

    let prod_lo = _mm_extract_epi64::<0>(product_reg) as u64;
    let prod_hi = _mm_extract_epi64::<1>(product_reg) as u64;
    let product = (prod_hi as u128) << 64 | prod_lo as u128;

    // Early return if already reduced
    if product >> degree == 0 {
        return product as u64;
    }

    // Step 2: q = (product >> m) clmul mu >> m
    let c_high = (product >> degree) as u64;
    let c_high_reg = _mm_set_epi64x(0, c_high as i64);
    let mu_reg = _mm_set_epi64x(0, mu as i64);
    let q_full_reg = _mm_clmulepi64_si128::<0x00>(c_high_reg, mu_reg);

    let q_lo = _mm_extract_epi64::<0>(q_full_reg) as u64;
    let q_hi = _mm_extract_epi64::<1>(q_full_reg) as u64;
    let q_full = (q_hi as u128) << 64 | q_lo as u128;
    let q = (q_full >> degree) as u64;

    // Step 3: r = product XOR (q clmul modulus)
    let q_reg = _mm_set_epi64x(0, q as i64);
    let mod_reg = _mm_set_epi64x(0, modulus as i64);
    let qp_reg = _mm_clmulepi64_si128::<0x00>(q_reg, mod_reg);

    let qp_lo = _mm_extract_epi64::<0>(qp_reg) as u64;
    let qp_hi = _mm_extract_epi64::<1>(qp_reg) as u64;
    let qp = (qp_hi as u128) << 64 | qp_lo as u128;

    let mut r = product ^ qp;

    // Correction steps (at most two)
    if r >> degree != 0 {
        r ^= modulus as u128;
    }
    if r >> degree != 0 {
        r ^= modulus as u128;
    }

    (r as u64) & field_mask
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scalar reference carry-less multiplication for testing.
    fn scalar_clmul(a: u64, b: u64) -> u128 {
        let a = a as u128;
        let mut result: u128 = 0;
        let mut b_remaining = b;
        while b_remaining != 0 {
            let bit = b_remaining.trailing_zeros();
            result ^= a << bit;
            b_remaining &= b_remaining - 1;
        }
        result
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_clmul_u64_matches_scalar() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            eprintln!("Skipping: PCLMULQDQ not available");
            return;
        }

        let test_cases: Vec<(u64, u64)> = vec![
            (0, 0),
            (0, 1),
            (1, 0),
            (1, 1),
            (0xFF, 0xFF),
            (0xFFFF, 0xFFFF),
            (0xFFFF_FFFF_FFFF_FFFF, 1),
            (1, 0xFFFF_FFFF_FFFF_FFFF),
            (0xDEAD_BEEF, 0xCAFE_BABE),
            (0x0123_4567_89AB_CDEF, 0xFEDC_BA98_7654_3210),
            (2, 3), // (x) * (x + 1) = x^2 + x
            (0b1011, 0b1101),
            // Large operands
            (0x8000_0000_0000_0000, 0x8000_0000_0000_0000),
            (0x7FFF_FFFF_FFFF_FFFF, 0x7FFF_FFFF_FFFF_FFFF),
        ];

        for (a, b) in test_cases {
            let expected = scalar_clmul(a, b);
            let result = unsafe { clmul_u64(a, b) };
            assert_eq!(
                result, expected,
                "clmul_u64({a:#018x}, {b:#018x}): got {result:#034x}, expected {expected:#034x}"
            );
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_clmul_u64_commutative() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            return;
        }

        let pairs = [
            (0xABCD_EF01u64, 0x1234_5678u64),
            (0xFFFF, 0x0001),
            (0xDEAD_BEEF_CAFE_BABE, 0x0123_4567_89AB_CDEF),
        ];

        for (a, b) in pairs {
            let ab = unsafe { clmul_u64(a, b) };
            let ba = unsafe { clmul_u64(b, a) };
            assert_eq!(ab, ba, "commutativity failed for ({a:#x}, {b:#x})");
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_clmul_u64_exhaustive_small() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            return;
        }

        // Exhaustive for all 8-bit pairs
        for a in 0u64..=255 {
            for b in 0u64..=255 {
                let expected = scalar_clmul(a, b);
                let result = unsafe { clmul_u64(a, b) };
                assert_eq!(
                    result, expected,
                    "clmul_u64({a}, {b}): got {result:#x}, expected {expected:#x}"
                );
            }
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_clmul_batch_matches_sequential() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            return;
        }

        let a_vals: Vec<u64> = (0..100)
            .map(|i| {
                (i as u64)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .wrapping_add(0xDEAD)
            })
            .collect();
        let b_vals: Vec<u64> = (0..100)
            .map(|i| {
                (i as u64)
                    .wrapping_mul(0x6C62_272E_07BB_0142)
                    .wrapping_add(0xBEEF)
            })
            .collect();

        // Compute sequentially
        let expected: Vec<u128> = a_vals
            .iter()
            .zip(b_vals.iter())
            .map(|(&a, &b)| unsafe { clmul_u64(a, b) })
            .collect();

        // Compute via batch
        let mut batch_out = vec![0u128; 100];
        unsafe { clmul_batch(&a_vals, &b_vals, &mut batch_out) };

        assert_eq!(batch_out, expected, "batch output differs from sequential");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_clmul_batch_odd_length() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            return;
        }

        // Test with odd-length slices (exercises the remainder handling)
        let a_vals = vec![0xABu64, 0xCD, 0xEF];
        let b_vals = vec![0x12u64, 0x34, 0x56];
        let mut out = vec![0u128; 3];

        unsafe { clmul_batch(&a_vals, &b_vals, &mut out) };

        for i in 0..3 {
            let expected = unsafe { clmul_u64(a_vals[i], b_vals[i]) };
            assert_eq!(out[i], expected, "mismatch at index {i}");
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_clmul_batch_empty() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            return;
        }

        let mut out: Vec<u128> = vec![];
        unsafe { clmul_batch(&[], &[], &mut out) };
        assert!(out.is_empty());
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_clmul_barrett_reduce_matches_scalar() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("pclmulqdq") {
            eprintln!("Skipping: PCLMULQDQ not available");
            return;
        }

        // Standard primitive polynomials for m=2..16
        let polys: &[(u32, u64)] = &[
            (2, 0b111),                // x^2 + x + 1
            (3, 0b1011),               // x^3 + x + 1
            (4, 0b10011),              // x^4 + x + 1
            (5, 0b100101),             // x^5 + x^2 + 1
            (6, 0b1000011),            // x^6 + x + 1
            (7, 0b10000011),           // x^7 + x + 1
            (8, 0b100011101),          // x^8 + x^4 + x^3 + x^2 + 1
            (9, 0b1000010001),         // x^9 + x^4 + 1
            (10, 0b10000001001),       // x^10 + x^3 + 1
            (11, 0b100000000101),      // x^11 + x^2 + 1
            (12, 0b1000001010011),     // x^12 + x^6 + x^4 + x + 1
            (13, 0b10000000011011),    // x^13 + x^4 + x^3 + x + 1
            (14, 0b100010001000011),   // x^14 + x^10 + x^6 + x + 1
            (15, 0b1000000000000011),  // x^15 + x + 1
            (16, 0b10001000000001011), // x^16 + x^12 + x^3 + x + 1
        ];

        for &(m, poly) in polys {
            // Compute Barrett constant mu = x^(2m) / P(x)
            let mut remainder: u128 = 1u128 << (2 * m);
            let mut mu: u64 = 0;
            let p = poly as u128;
            for i in (0..=m).rev() {
                let bit_pos = m + i;
                if (remainder >> bit_pos) & 1 == 1 {
                    mu |= 1u64 << i;
                    remainder ^= p << i;
                }
            }

            let field_mask = (1u64 << m) - 1;
            let num_elements = 1u64 << m;
            // For small fields, test exhaustively; for larger ones, sample
            let test_count = if num_elements <= 256 {
                num_elements
            } else {
                256
            };

            for i in 0..test_count {
                let a = if num_elements <= 256 {
                    i
                } else {
                    i.wrapping_mul(0x9E37_79B9) & field_mask | 1
                };
                for j in 0..test_count {
                    let b = if num_elements <= 256 {
                        j
                    } else {
                        j.wrapping_mul(0x6C62_272E) & field_mask | 1
                    };

                    let simd_result = unsafe { clmul_barrett_reduce(a, b, mu, poly, m) };

                    // Scalar reference: clmul then naive reduce
                    let product = scalar_clmul(a, b);
                    let mut r = product;
                    for bit in (m..128).rev() {
                        if (r >> bit) & 1 == 1 {
                            r ^= (poly as u128) << (bit - m);
                        }
                    }
                    let expected = (r as u64) & field_mask;

                    assert_eq!(
                        simd_result, expected,
                        "m={m}, a={a:#x}, b={b:#x}: SIMD={simd_result:#x}, expected={expected:#x}"
                    );
                }
            }
        }
    }
}
