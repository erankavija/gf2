//! SIMD batch element-wise multiply/square kernel for GF(2^m) at m ∈ {8, 16, 32}.
//!
//! Per-element scalar dispatch via [`crate::gf2m::Gf2mFns::clmul_barrett_fn`]
//! processes one multiply per call. VPCLMULQDQ-on-YMM can pack two 64×64
//! carry-less multiplies into a single instruction (one per 128-bit lane), and
//! the Barrett reduction lookup constants for `m <= 32` fit comfortably in a
//! single YMM lane. This module exposes a vectorised batch path that
//! processes 2 element pairs per VPCLMULQDQ for the multiply step and 2
//! element pairs per VPCLMULQDQ for each Barrett reduce step (`q = c_high *
//! mu`, `r = product XOR q * P`).
//!
//! # Lane preference
//!
//! [`detect`] returns the fastest available lane:
//!
//! 1. **AVX2 + VPCLMULQDQ** (YMM, 256-bit) — 2 multiplies per VPCLMULQDQ.
//!    Primary path on Zen 3.
//! 2. `None` — callers fall back to the per-element
//!    [`crate::gf2m::Gf2mFns::clmul_barrett_fn`] dispatch (PCLMULQDQ scalar
//!    lane) or the pure-Rust scalar shift-and-add reducer in `gf2-core`.
//!
//! # Layout
//!
//! Inputs are slices of `u64`s holding canonical field elements (each `< 2^m`).
//! Output is a slice of `u64`s of the same length, written element-wise. The
//! multiply kernel computes `out[i] = a[i] * b[i] mod P(x)` and the square
//! kernel computes `out[i] = a[i]^2 mod P(x)`.
//!
//! Tail elements (count not a multiple of 4) are handled by a scalar fallback
//! path inside the same `#[target_feature]` scope to avoid call-pointer
//! overhead.
//!
//! # Why m ∈ {8, 16, 32}
//!
//! * `m = 8`: GF(2^8) — Reed-Solomon ground field, BCH code helpers.
//! * `m = 16`: GF(2^16) — DVB-T2 BCH outer field.
//! * `m = 32`: GF(2^32) — emerging research codes (Gabidulin, network
//!   coding); the largest power-of-two `m` where two operands and the
//!   Barrett constants all fit in 64-bit lanes inside a YMM register.
//!
//! For `m > 32`, the Barrett `mu` and modulus may overflow a single 64-bit
//! lane; the existing `crate::gf2m_wide` multi-word kernel covers those.

/// Kernel signature: in-place batch element-wise multiply.
///
/// Computes `out[i] = a[i] * b[i] mod P(x)` for `i ∈ 0..len`. All slices must
/// have the same length. Each input element must already be reduced
/// (< 2^m); output elements are reduced.
///
/// The kernel dispatches the Barrett constants `(mu, modulus, degree)` once
/// per call rather than per element.
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
    /// Human-readable lane tag, one of `"avx2+vpclmulqdq-ymm"`,
    /// `"avx2+vpclmulqdq-ymm-unroll4"`.
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

// ---------------------------------------------------------------------------
// Safe function-pointer wrappers (unsafe isolated in `crate::x86::gf2m_batch`)
// ---------------------------------------------------------------------------
//
// `detect_x86` only publishes these function pointers when the corresponding
// feature is detected at runtime. Callers that bypass `detect` must uphold
// the feature precondition themselves.

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

/// Test-only scalar reference matching the SIMD kernel's contract. Mirrors
/// the canonical `BarrettReducer` reduction in `gf2_core::gf2m::barrett` so
/// proptest equivalence harnesses can compare bit-exact results.
#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::clmul_u64_scalar;

    /// Reduce a 128-bit carry-less product modulo a degree-`m` polynomial
    /// using the canonical naive bit-by-bit reduction. Identical contract to
    /// `gf2_core::gf2m::barrett::naive_reduce`, duplicated here only because
    /// `gf2-kernels-simd` does not depend on `gf2-core` and we need a SSOT
    /// for the reference oracle.
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

    /// Scalar-only reference that mirrors the per-element SIMD contract.
    /// Used by every kernel test in this crate to confirm bit-exact
    /// equivalence.
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

    /// Compute Barrett `mu = x^(2m) / P(x)` for `m <= 32`. Mirrors the
    /// canonical computation in `gf2_core::gf2m::barrett::BarrettReducer::new`.
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

            // Deterministic test inputs.
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

        // Word-boundary element counts: 0/1/3/4/5/7/8/9/15/16/17 — covers
        // tail-handling on the 4-element YMM-lane unroll.
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

        // a contains zeros at various positions
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
