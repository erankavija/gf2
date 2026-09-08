//! Panelized GF(2^m) GEMM dispatch for `m ∈ {8, 16, 32}`.
//!
//! Exposes a safe function-pointer bundle ([`Gf2mGemmFns`]) and the
//! [`detect`] function that picks the best available implementation at
//! runtime.
//!
//! # Algorithm
//!
//! The panelized algorithm replaces the per-output-cell scratch-buffer
//! approach of `try_gf2m_u64_batch_dot_product`. Instead of K
//! element-wise multiplies per (i, j) output cell, the outer loop is
//! re-ordered to (i, k, j):
//!
//! ```text
//! out = 0
//! for i in 0..M:
//!   for k in 0..K:
//!     // a_ik is scalar; multiply it by the entire k-th row of B
//!     for j in 0..N:
//!       out[i,j] ^= clmul_barrett(a[i,k], b[k,j])
//! ```
//!
//! The inner (k,j) step — "broadcast scalar a_ik × row of B, XOR into
//! accumulator row" — is vectorised with VPCLMULQDQ: a_ik is broadcast
//! into both lanes of a YMM register and 4 b-values are multiplied per
//! VPCLMULQDQ instruction.
//!
//! # Layout contract
//!
//! * `a_flat`: m × k row-major slice (no padding).
//! * `b_flat`: k × n row-major slice — `b_flat[k_idx * n + j] = B[k_idx, j]`.
//!   This is the **original** B matrix layout (not B^T). The caller is
//!   responsible for pre-extracting B elements from `FieldMatrix` storage.
//! * `out`:    m × n row-major, zeroed before the call.
//!
//! # Lane selection
//!
//! 1. **AVX2 + VPCLMULQDQ** — primary path on Zen 3.
//! 2. `None` — callers fall back to the existing
//!    `try_gf2m_u64_batch_dot_product` per-cell path.

/// Kernel signature for the panelized GF(2^m) GEMM.
///
/// `a_flat[i*k + ki] = A[i, ki]`; `b_flat[ki*n + j] = B[ki, j]`;
/// `out[i*n + j] = sum_k a[i,k]*b[k,j]`. `out` must be zeroed before the
/// call.
pub type Gf2mGemmFn = fn(
    a_flat: &[u64],
    b_flat: &[u64],
    out: &mut [u64],
    m: usize,
    k: usize,
    n: usize,
    mu: u64,
    modulus: u64,
    degree: u32,
);

/// Bundle of dispatched panelized GF(2^m) GEMM kernels.
#[derive(Copy, Clone)]
pub struct Gf2mGemmFns {
    /// Panelized broadcast-multiply-accumulate GEMM.
    pub gemm_fn: Gf2mGemmFn,
    /// Human-readable lane tag.
    pub name: &'static str,
}

/// Detect and return the best available panelized GEMM bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// the `avx2 + vpclmulqdq` feature pair.
pub fn detect() -> Option<Gf2mGemmFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<Gf2mGemmFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2")
        && is_x86_feature_detected!("vpclmulqdq")
        && is_x86_feature_detected!("pclmulqdq")
        && is_x86_feature_detected!("sse4.1")
    {
        return Some(Gf2mGemmFns {
            gemm_fn: gf2m_gemm_panelized_safe,
            name: "avx2+vpclmulqdq-panelized",
        });
    }
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[allow(clippy::too_many_arguments)]
fn gf2m_gemm_panelized_safe(
    a_flat: &[u64],
    b_flat: &[u64],
    out: &mut [u64],
    m: usize,
    k: usize,
    n: usize,
    mu: u64,
    modulus: u64,
    degree: u32,
) {
    // SAFETY: `detect_x86` only returns this pointer when avx2, vpclmulqdq,
    // pclmulqdq, and sse4.1 are all present at runtime.
    unsafe {
        crate::x86::gf2m_gemm::gf2m_gemm_panelized(
            a_flat, b_flat, out, m, k, n, mu, modulus, degree,
        )
    }
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
    fn detect_returns_some_on_avx2_vpclmulqdq_host() {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("avx2")
                && is_x86_feature_detected!("vpclmulqdq")
                && is_x86_feature_detected!("pclmulqdq")
                && is_x86_feature_detected!("sse4.1")
            {
                assert!(
                    detect().is_some(),
                    "expected panelized GEMM bundle on this host"
                );
            }
        }
    }

    #[test]
    fn panelized_gemm_gf256_correctness() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        let poly: u64 = 0b100011101;
        let degree = 8u32;
        let mu = compute_mu(poly, degree);
        let mask = 0xFFu64;
        let m = 16usize;
        let k = 16usize;
        let n = 16usize;

        let a: Vec<u64> = (0..m * k)
            .map(|i| (i as u64).wrapping_mul(0x9E37_79B9) & mask)
            .collect();
        let b: Vec<u64> = (0..k * n)
            .map(|i| (i as u64).wrapping_mul(0x6C62_272E) & mask)
            .collect();

        let mut got = vec![0u64; m * n];
        (fns.gemm_fn)(&a, &b, &mut got, m, k, n, mu, poly, degree);

        // Cross-check with scalar triple loop.
        for i in 0..m {
            for j in 0..n {
                let mut acc = 0u64;
                for ki in 0..k {
                    // Pure-Rust schoolbook GF(2^8) multiply (no SIMD).
                    let mut a_val = a[i * k + ki];
                    let mut b_val = b[ki * n + j];
                    let mut prod = 0u64;
                    for _ in 0..8 {
                        if b_val & 1 == 1 {
                            prod ^= a_val;
                        }
                        let msb = (a_val >> 7) & 1;
                        a_val = (a_val << 1) & 0xFF;
                        if msb == 1 {
                            a_val ^= 0x1D; // poly without leading bit
                        }
                        b_val >>= 1;
                    }
                    acc ^= prod;
                }
                assert_eq!(got[i * n + j], acc, "mismatch at ({i},{j})");
            }
        }
    }
}
