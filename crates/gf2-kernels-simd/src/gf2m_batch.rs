//! SIMD batch element-wise multiply/square kernel for GF(2^m) at m ∈ {8, 16, 32}.
//!
//! Inputs and outputs are `u64` slices of canonical field elements (each
//! `< 2^m`). [`detect`] returns `None` without AVX2 and VPCLMULQDQ, and
//! callers fall back to per-element [`crate::gf2m::Gf2mFns::clmul_barrett_fn`]
//! dispatch.

/// Kernel signature: in-place batch element-wise multiply.
///
/// Computes `out[i] = a[i] * b[i] mod P(x)` for `i ∈ 0..len`. All slices must
/// have the same length. Each input element must already be reduced
/// (< 2^m); output elements are reduced.
pub type Gf2mBatchMulFn =
    fn(a: &[u64], b: &[u64], out: &mut [u64], mu: u64, modulus: u64, degree: u32);

/// Kernel signature: in-place batch element-wise square.
///
/// Computes `out[i] = a[i] * a[i] mod P(x)` for `i ∈ 0..len`. Slices must have
/// equal length. Each input element must already be reduced (< 2^m); output
/// elements are reduced.
pub type Gf2mBatchSquareFn = fn(a: &[u64], out: &mut [u64], mu: u64, modulus: u64, degree: u32);

/// Bundle of dispatched batch element-wise GF(2^m) kernels.
#[derive(Copy, Clone)]
pub struct Gf2mBatchFns {
    /// Element-wise batch multiply with Barrett reduction.
    pub mul_fn: Gf2mBatchMulFn,
    /// Element-wise batch square with Barrett reduction.
    pub square_fn: Gf2mBatchSquareFn,
    /// Lane tag of the published kernels.
    pub name: &'static str,
}

/// Detect and return the best available batch GF(2^m) multiply/square bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks the
/// `avx2 + vpclmulqdq` feature pair. Callers receiving `None` must fall back
/// to per-element dispatch.
pub fn detect() -> Option<Gf2mBatchFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<Gf2mBatchFns> {
    use std::arch::is_x86_feature_detected;

    if is_x86_feature_detected!("avx2")
        && is_x86_feature_detected!("vpclmulqdq")
        && is_x86_feature_detected!("pclmulqdq")
        && is_x86_feature_detected!("sse4.1")
    {
        return Some(Gf2mBatchFns {
            mul_fn: gf2m_batch_mul_ymm_unroll4_safe,
            square_fn: gf2m_batch_square_ymm_unroll4_safe,
            name: "avx2+vpclmulqdq-ymm-unroll4",
        });
    }

    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn gf2m_batch_mul_ymm_unroll4_safe(
    a: &[u64],
    b: &[u64],
    out: &mut [u64],
    mu: u64,
    modulus: u64,
    degree: u32,
) {
    // SAFETY: `detect_x86` only returns this pointer when AVX2, VPCLMULQDQ,
    // PCLMULQDQ, and SSE4.1 are available.
    unsafe { crate::x86::gf2m_batch::gf2m_batch_mul_ymm_unroll4(a, b, out, mu, modulus, degree) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn gf2m_batch_square_ymm_unroll4_safe(
    a: &[u64],
    out: &mut [u64],
    mu: u64,
    modulus: u64,
    degree: u32,
) {
    // SAFETY: same as `gf2m_batch_mul_ymm_unroll4_safe`.
    unsafe { crate::x86::gf2m_batch::gf2m_batch_square_ymm_unroll4(a, out, mu, modulus, degree) }
}

/// Scalar reference matching the SIMD kernel's contract.
#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::clmul_u64_scalar;

    /// Reduce a 128-bit carry-less product modulo a degree-`m` polynomial
    /// bit by bit.
    pub(crate) fn naive_reduce(product: u128, modulus: u64, degree: u32) -> u64 {
        let mask = if degree == 64 {
            u64::MAX
        } else {
            (1u64 << degree) - 1
        };
        let mut r = product;
        for bit in (degree..128).rev() {
            if (r >> bit) & 1 == 1 {
                r ^= (modulus as u128) << (bit - degree);
            }
        }
        (r as u64) & mask
    }

    /// Scalar reference for the multiply kernel.
    pub(crate) fn scalar_batch_mul(
        a: &[u64],
        b: &[u64],
        out: &mut [u64],
        modulus: u64,
        degree: u32,
    ) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), out.len());
        for i in 0..a.len() {
            let p = clmul_u64_scalar(a[i], b[i]);
            out[i] = naive_reduce(p, modulus, degree);
        }
    }

    /// Scalar reference for the square kernel.
    pub(crate) fn scalar_batch_square(a: &[u64], out: &mut [u64], modulus: u64, degree: u32) {
        assert_eq!(a.len(), out.len());
        for i in 0..a.len() {
            let p = clmul_u64_scalar(a[i], a[i]);
            out[i] = naive_reduce(p, modulus, degree);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::{naive_reduce, scalar_batch_mul, scalar_batch_square};
    use super::*;

    fn primitive_polys() -> &'static [(u32, u64)] {
        &[
            (8, 0b100011101),                                  // x^8 + x^4 + x^3 + x^2 + 1
            (16, 0b1_0001_0000_0000_1011),                     // x^16 + x^12 + x^3 + x + 1
            (32, 0b1_0000_0000_0100_0000_0000_0000_0000_0111), // x^32 + x^22 + x^2 + x + 1
        ]
    }

    /// Compute Barrett `mu = x^(2m) / P(x)` for `m <= 32`.
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
    fn detect_returns_some_on_avx2_vpclmulqdq_host() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("avx2")
                && is_x86_feature_detected!("vpclmulqdq")
                && is_x86_feature_detected!("pclmulqdq")
                && is_x86_feature_detected!("sse4.1")
            {
                assert!(
                    fns.is_some(),
                    "expected AVX2+VPCLMULQDQ batch kernel on this host"
                );
            }
        }
    }

    #[test]
    fn batch_mul_matches_scalar_for_each_supported_m() {
        let fns = match detect() {
            Some(f) => f,
            None => {
                eprintln!("skipping: AVX2+VPCLMULQDQ not available");
                return;
            }
        };

        for &(m, poly) in primitive_polys() {
            let mu = compute_mu(poly, m);
            let mask = (1u64 << m) - 1;
            let n = 33; // odd to exercise the tail handler

            let a: Vec<u64> = (0..n)
                .map(|i| {
                    let v = gf2_core::rng::Lcg::new(0xA5A5_A5A5 ^ i as u64).next_u64();
                    v & mask
                })
                .collect();
            let b: Vec<u64> = (0..n)
                .map(|i| {
                    let v = gf2_core::rng::Lcg::new(0x5A5A_5A5A ^ i as u64).next_u64();
                    v & mask
                })
                .collect();

            let mut got = vec![0u64; n];
            (fns.mul_fn)(&a, &b, &mut got, mu, poly, m);

            let mut expected = vec![0u64; n];
            scalar_batch_mul(&a, &b, &mut expected, poly, m);

            assert_eq!(got, expected, "mismatch at m={m}, poly={poly:#x}");
        }
    }

    #[test]
    fn batch_square_matches_scalar_for_each_supported_m() {
        let fns = match detect() {
            Some(f) => f,
            None => {
                eprintln!("skipping: AVX2+VPCLMULQDQ not available");
                return;
            }
        };

        for &(m, poly) in primitive_polys() {
            let mu = compute_mu(poly, m);
            let mask = (1u64 << m) - 1;
            let n = 17;

            let a: Vec<u64> = (0..n)
                .map(|i| gf2_core::rng::Lcg::new(0xC3C3_C3C3 ^ i as u64).next_u64() & mask)
                .collect();

            let mut got = vec![0u64; n];
            (fns.square_fn)(&a, &mut got, mu, poly, m);

            let mut expected = vec![0u64; n];
            scalar_batch_square(&a, &mut expected, poly, m);

            assert_eq!(got, expected, "square mismatch at m={m}");
        }
    }

    #[test]
    fn batch_mul_word_boundary_lengths() {
        let fns = match detect() {
            Some(f) => f,
            None => {
                eprintln!("skipping: AVX2+VPCLMULQDQ not available");
                return;
            }
        };

        // Lengths around the 4-element YMM-lane unroll.
        let lengths = [0usize, 1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33];
        let m: u32 = 8;
        let poly: u64 = 0b100011101;
        let mu = compute_mu(poly, m);
        let mask = (1u64 << m) - 1;

        for &len in &lengths {
            let a: Vec<u64> = (0..len)
                .map(|i| (i as u64).wrapping_mul(0x9E37_79B9) & mask)
                .collect();
            let b: Vec<u64> = (0..len)
                .map(|i| (i as u64).wrapping_mul(0x6C62_272E) & mask)
                .collect();

            let mut got = vec![0u64; len];
            (fns.mul_fn)(&a, &b, &mut got, mu, poly, m);

            let mut expected = vec![0u64; len];
            scalar_batch_mul(&a, &b, &mut expected, poly, m);

            assert_eq!(got, expected, "boundary mismatch at len={len}");
        }
    }

    #[test]
    fn batch_mul_handles_zero_inputs() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };

        let m: u32 = 8;
        let poly: u64 = 0b100011101;
        let mu = compute_mu(poly, m);

        let a: Vec<u64> = vec![0, 1, 0, 0xAB, 0xCD, 0, 0xFF, 0];
        let b: Vec<u64> = vec![0xFF, 0, 1, 0, 0xAB, 0xCD, 0xEF, 0];
        let mut got = vec![0u64; a.len()];
        (fns.mul_fn)(&a, &b, &mut got, mu, poly, m);

        let mut expected = vec![0u64; a.len()];
        scalar_batch_mul(&a, &b, &mut expected, poly, m);
        assert_eq!(got, expected);
    }

    #[test]
    fn naive_reduce_smoke() {
        // x^8 + x^4 + x^3 + x^2 + 1
        let p: u64 = 0b100011101;
        // (x+1) * (x+1) = x^2 + 1, no reduction required at m=8
        let prod: u128 = crate::clmul_u64_scalar(0b11, 0b11);
        assert_eq!(naive_reduce(prod, p, 8), 0b101);
    }
}
