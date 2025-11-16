//! SIMD-accelerated GF(2^m) field multiplication kernels.
//!
//! This module provides carry-less multiplication using PCLMULQDQ (x86_64)
//! for efficient polynomial multiplication over GF(2), which is the core
//! operation in GF(2^m) field arithmetic.

/// GF(2^m) field multiplication function.
///
/// Multiplies two field elements and reduces modulo the primitive polynomial.
///
/// # Arguments
/// * `a` - First field element
/// * `b` - Second field element
/// * `m` - Field size (element is in GF(2^m))
/// * `primitive_poly` - Primitive polynomial for reduction
///
/// # Returns
/// Product a * b reduced modulo primitive_poly
pub type Gf2mMulFn = fn(u64, u64, usize, u64) -> u64;

/// Bundle of GF(2^m) multiplication functions for different field sizes.
pub struct Gf2mFns {
    /// General multiplication for any m ≤ 64
    pub mul_fn: Gf2mMulFn,
}

/// Detect and return the best available GF(2^m) function bundle.
pub fn detect() -> Option<Gf2mFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<Gf2mFns> {
    use std::arch::is_x86_feature_detected;

    if is_x86_feature_detected!("pclmulqdq") {
        Some(Gf2mFns {
            mul_fn: gf2m_mul_pclmul_safe,
        })
    } else {
        None
    }
}

/// Safe wrapper for PCLMULQDQ multiplication
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn gf2m_mul_pclmul_safe(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
    unsafe { gf2m_mul_pclmul(a, b, m, primitive_poly) }
}

/// Multiply two GF(2^m) elements using PCLMULQDQ.
///
/// # Safety
/// Requires PCLMULQDQ CPU feature.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
unsafe fn gf2m_mul_pclmul(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    if a == 0 || b == 0 {
        return 0;
    }

    // Carry-less multiplication
    let a_reg = _mm_set_epi64x(0, a as i64);
    let b_reg = _mm_set_epi64x(0, b as i64);
    let product = _mm_clmulepi64_si128::<0x00>(a_reg, b_reg);

    let lo = _mm_extract_epi64::<0>(product) as u64;
    let hi = _mm_extract_epi64::<1>(product) as u64;

    // Fast reduction for common field sizes
    match m {
        8 => reduce_gf256(lo, hi, primitive_poly),
        16 => reduce_gf65536(lo, hi, primitive_poly),
        _ => reduce_generic(lo, hi, m, primitive_poly),
    }
}

/// Fast reduction for GF(2^8)
#[inline(always)]
unsafe fn reduce_gf256(lo: u64, hi: u64, primitive_poly: u64) -> u64 {
    // Product is at most 14 bits (degree 7 + degree 7 = degree 14)
    // We need to reduce modulo primitive_poly (degree 8)

    let mut result = lo;

    // Reduce bits 8-14 from lo, and any bits from hi
    for bit_idx in (8..15).rev() {
        if (result >> bit_idx) & 1 == 1 {
            result ^= primitive_poly << (bit_idx - 8);
        }
    }

    // Handle any contribution from hi (bits 64+)
    if hi != 0 {
        for i in 0..7 {
            if (hi >> i) & 1 == 1 {
                result ^= primitive_poly << (64 + i - 8);
            }
        }
    }

    result & 0xFF
}

/// Fast reduction for GF(2^16)
#[inline(always)]
unsafe fn reduce_gf65536(lo: u64, hi: u64, primitive_poly: u64) -> u64 {
    // Product is at most 30 bits
    let mut result = lo;

    // Reduce bits 16-30 from lo
    for bit_idx in (16..31).rev() {
        if (result >> bit_idx) & 1 == 1 {
            result ^= primitive_poly << (bit_idx - 16);
        }
    }

    // Handle contribution from hi
    if hi != 0 {
        for i in 0..15 {
            if (hi >> i) & 1 == 1 {
                result ^= primitive_poly << (64 + i - 16);
            }
        }
    }

    result & 0xFFFF
}

/// Generic reduction for arbitrary m
#[inline(always)]
unsafe fn reduce_generic(mut lo: u64, hi: u64, m: usize, primitive_poly: u64) -> u64 {
    // Reduce high part first
    if hi != 0 {
        for i in (0..64).rev() {
            if (hi >> i) & 1 == 1 {
                let bit_pos = i + 64;
                if bit_pos >= m {
                    let shift = bit_pos - m;
                    if shift < 64 {
                        lo ^= primitive_poly << shift;
                    }
                }
            }
        }
    }

    // Reduce lo to < m bits
    for i in (m..64).rev() {
        if (lo >> i) & 1 == 1 {
            lo ^= primitive_poly << (i - m);
        }
    }

    lo & ((1u64 << m) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("pclmulqdq") {
                assert!(fns.is_some());
            }
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_gf256_mul() {
        use std::arch::is_x86_feature_detected;

        if !is_x86_feature_detected!("pclmulqdq") {
            eprintln!("Skipping: PCLMULQDQ not available");
            return;
        }

        let fns = detect().unwrap();

        // GF(2^8) with x^8 + x^4 + x^3 + x + 1
        let m = 8;
        let p = 0x11B;

        // Test identity
        assert_eq!((fns.mul_fn)(1, 5, m, p), 5);
        assert_eq!((fns.mul_fn)(5, 1, m, p), 5);

        // Test zero
        assert_eq!((fns.mul_fn)(0, 5, m, p), 0);
        assert_eq!((fns.mul_fn)(5, 0, m, p), 0);

        // Test known value
        let a = 0x53;
        let b = 0xCA;
        let result = (fns.mul_fn)(a, b, m, p);
        let expected = scalar_mul(a, b, m, p);
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_gf16_mul() {
        use std::arch::is_x86_feature_detected;

        if !is_x86_feature_detected!("pclmulqdq") {
            return;
        }

        let fns = detect().unwrap();
        let m = 4;
        let p = 0x13; // x^4 + x + 1

        // (x^2 + 1) * (x^3 + x) in GF(16)
        let a = 0b0101;
        let b = 0b1010;
        let result = (fns.mul_fn)(a, b, m, p);
        let expected = scalar_mul(a, b, m, p);
        assert_eq!(result, expected);
    }

    // Reference scalar implementation
    fn scalar_mul(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
        let mut result = 0u64;
        let mut temp = a;

        for i in 0..m {
            if (b >> i) & 1 == 1 {
                result ^= temp;
            }

            let will_overflow = (temp & (1u64 << (m - 1))) != 0;
            temp <<= 1;

            if will_overflow {
                temp ^= primitive_poly;
            }
        }

        result & ((1u64 << m) - 1)
    }
}
