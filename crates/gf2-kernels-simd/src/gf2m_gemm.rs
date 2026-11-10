//! Panelized GF(2^m) GEMM dispatch for `m ∈ {8, 16, 32}`. The loop order is
//! `(i, k, j)`: each scalar `a[i,k]` is broadcast and multiplied against row
//! `k` of B with VPCLMULQDQ, and the Barrett-reduced products are XORed into
//! output row `i`. `b_flat` is B itself in row-major layout
//! (`b_flat[k * n + j] = B[k, j]`), and `out` must be zeroed before the call.
//! [`detect`] returns `None` without AVX2 and VPCLMULQDQ.

/// Kernel signature for the panelized GF(2^m) GEMM.
///
/// `a_flat[i*k + ki] = A[i, ki]`; `b_flat[ki*n + j] = B[ki, j]`;
/// `out[i*n + j] = sum_k a[i,k]*b[k,j]`. `out` must be zeroed before the
/// call.
///
/// # Panics
///
/// Panics unless `a_flat.len() == m * k`, `b_flat.len() == k * n` and
/// `out.len() == m * n`.
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
    const KERNEL: &str = "gf2m_gemm_panelized";
    crate::entry_check::assert_shape(KERNEL, "a_flat", a_flat.len(), m, k);
    crate::entry_check::assert_shape(KERNEL, "b_flat", b_flat.len(), k, n);
    crate::entry_check::assert_shape(KERNEL, "out", out.len(), m, n);
    // SAFETY: `detect_x86` only returns this pointer when avx2, vpclmulqdq,
    // pclmulqdq, and sse4.1 are all present at runtime. The assertions above
    // keep the shape products from wrapping in the kernel's own assertions.
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

    /// Calls the dispatched kernel on a 2 x 3 by 3 x 4 GF(2^8) product with
    /// `extra` cells appended to the operand `grow` names. Oversized operands
    /// stay inside every slice index, so only the entry check rejects them.
    fn gemm_with_oversized(grow: &str, extra: usize) {
        let fns = crate::contract::require_detected("gf2m_gemm::detect()", detect());
        let (m, k, n) = (2usize, 3usize, 4usize);
        let poly: u64 = 0b100011101;
        let mu = compute_mu(poly, 8);
        let len = |operand: &str, cells: usize| cells + if operand == grow { extra } else { 0 };
        let a = vec![1u64; len("a_flat", m * k)];
        let b = vec![1u64; len("b_flat", k * n)];
        let mut out = vec![0u64; len("out", m * n)];
        (fns.gemm_fn)(&a, &b, &mut out, m, k, n, mu, poly, 8);
    }

    #[test]
    #[should_panic(expected = "gf2m_gemm_panelized: a_flat shape")]
    fn panelized_gemm_rejects_a_mismatched_a() {
        gemm_with_oversized("a_flat", 1);
    }

    #[test]
    #[should_panic(expected = "gf2m_gemm_panelized: b_flat shape")]
    fn panelized_gemm_rejects_a_mismatched_b() {
        gemm_with_oversized("b_flat", 1);
    }

    #[test]
    #[should_panic(expected = "gf2m_gemm_panelized: out shape")]
    fn panelized_gemm_rejects_a_mismatched_out() {
        gemm_with_oversized("out", 1);
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
        let fns = crate::contract::require_detected("gf2m_gemm::detect()", detect());
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

        for i in 0..m {
            for j in 0..n {
                let mut acc = 0u64;
                for ki in 0..k {
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
