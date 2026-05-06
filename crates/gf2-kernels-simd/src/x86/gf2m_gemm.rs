//! Panelized GF(2^m) GEMM kernel using AVX2 + VPCLMULQDQ.
//!
//! Exposes a single public entry point:
//! [`gf2m_gemm_broadcast_xor_row`] — multiply a scalar `a_ik` by every
//! element of `b_row`, XOR-accumulate into `acc_row`. The caller loops
//! over rows (i) and inner columns (k) to build the full matrix product.
//!
//! This "broadcast-multiply-accumulate" inner kernel is the hot path for
//! the panelized GEMM that replaces the per-output-cell scratch-buffer
//! approach previously used by `try_gf2m_u64_batch_dot_product`. The
//! algorithm:
//!
//! ```text
//! for i in 0..M:
//!     for k in 0..K:
//!         broadcast_mul_xor(a[i,k], b[k, 0..N], &mut out[i, 0..N])
//! ```
//!
//! produces the same `out = A · B` as the triple-loop, but all K
//! multiply-accumulate steps for output row `i` share one contiguous
//! pass over `out[i, 0..N]`.
//!
//! # Layout contract
//!
//! * `b_row`: `b[k, 0..N]` — the k-th row of B, stored contiguously.
//! * `acc_row`: `out[i, 0..N]` — the i-th output row, XOR-accumulated
//!   in place. Must be zeroed by the caller before the first k=0 step.
//!
//! # Safety / feature detection
//!
//! All entry points carry `#[target_feature(enable = "avx2", ...)]`.
//! The safe wrapper in `crate::gf2m_gemm` only publishes the function
//! pointer when the CPU supports the necessary feature set.

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// Shared Barrett-reduction helpers live in `super::gf2m_common`; both this
// module and `super::gf2m_batch` import them from there. Single source of
// truth for the carry-less multiply + Barrett reduce algorithm.
// ---------------------------------------------------------------------------

use super::gf2m_common::{clmul_barrett_scalar, correct, ymm_barrett_reduce};

// ---------------------------------------------------------------------------
// Main kernel: broadcast-multiply-accumulate
// ---------------------------------------------------------------------------

/// Inner kernel for the panelized GEMM: for a fixed scalar `a_ik`, compute
/// `acc_row[j] ^= a_ik * b_row[j]` for all j.
///
/// Uses VPCLMULQDQ with `a_ik` broadcast across both 128-bit lanes of a
/// YMM register; processes 4 b-elements per outer iteration.
///
/// # Safety
///
/// Requires avx2, vpclmulqdq, pclmulqdq, sse4.1. `b_row.len()` must equal
/// `acc_row.len()`.
#[target_feature(
    enable = "avx2",
    enable = "vpclmulqdq",
    enable = "pclmulqdq",
    enable = "sse4.1"
)]
pub unsafe fn gf2m_broadcast_mul_xor<const SHIFT: i32>(
    a_ik: u64,
    b_row: &[u64],
    acc_row: &mut [u64],
    mu: u64,
    modulus: u64,
) {
    let n = b_row.len();
    debug_assert_eq!(acc_row.len(), n);

    if a_ik == 0 {
        return;
    }

    let degree = (SHIFT as u32) * 8;
    let mask = if degree == 64 {
        u64::MAX
    } else {
        (1u64 << degree) - 1
    };

    // Broadcast a_ik to both 128-bit lanes so VPCLMULQDQ can multiply it
    // against two b-values simultaneously.
    let a_ymm = _mm256_set_epi64x(0, a_ik as i64, 0, a_ik as i64);
    let mu_ymm = _mm256_set_epi64x(0, mu as i64, 0, mu as i64);
    let mod_ymm = _mm256_set_epi64x(0, modulus as i64, 0, modulus as i64);

    let mut j = 0usize;

    // Unrolled 4-way loop: process 4 b-elements per iteration.
    while j + 4 <= n {
        let b_lo = _mm256_set_epi64x(0, b_row[j + 1] as i64, 0, b_row[j] as i64);
        let b_hi = _mm256_set_epi64x(0, b_row[j + 3] as i64, 0, b_row[j + 2] as i64);

        let prod_lo = _mm256_clmulepi64_epi128::<0x00>(a_ymm, b_lo);
        let prod_hi = _mm256_clmulepi64_epi128::<0x00>(a_ymm, b_hi);

        let (r_lo, r_hi) = ymm_barrett_reduce::<SHIFT>(prod_lo, prod_hi, mu_ymm, mod_ymm);

        let lane0 = _mm256_extracti128_si256::<0>(r_lo);
        let lane1 = _mm256_extracti128_si256::<1>(r_lo);
        let lane2 = _mm256_extracti128_si256::<0>(r_hi);
        let lane3 = _mm256_extracti128_si256::<1>(r_hi);

        let r0 = correct(_mm_extract_epi64::<0>(lane0) as u64, modulus, SHIFT, mask);
        let r1 = correct(_mm_extract_epi64::<0>(lane1) as u64, modulus, SHIFT, mask);
        let r2 = correct(_mm_extract_epi64::<0>(lane2) as u64, modulus, SHIFT, mask);
        let r3 = correct(_mm_extract_epi64::<0>(lane3) as u64, modulus, SHIFT, mask);

        acc_row[j] ^= r0;
        acc_row[j + 1] ^= r1;
        acc_row[j + 2] ^= r2;
        acc_row[j + 3] ^= r3;

        j += 4;
    }

    // Scalar tail.
    while j < n {
        let p = clmul_barrett_scalar(a_ik, b_row[j], mu, modulus, degree);
        acc_row[j] ^= p;
        j += 1;
    }
}

/// Panelized GF(2^m) GEMM: `out = A · B` (all inputs / output in
/// canonical u64 elements), degree-dispatched.
///
/// * `a_flat` — m × k row-major, `a_flat[i*k + ki] = A[i, ki]`.
/// * `b_flat` — k × n row-major, `b_flat[ki*n + j] = B[ki, j]`.
/// * `out`    — m × n row-major, zeroed by the caller.
///
/// # Safety
///
/// Requires avx2, vpclmulqdq, pclmulqdq, sse4.1. `degree ∈ {8, 16, 32}`.
#[target_feature(
    enable = "avx2",
    enable = "vpclmulqdq",
    enable = "pclmulqdq",
    enable = "sse4.1"
)]
pub unsafe fn gf2m_gemm_panelized(
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
    debug_assert_eq!(a_flat.len(), m * k);
    debug_assert_eq!(b_flat.len(), k * n);
    debug_assert_eq!(out.len(), m * n);

    // Row-tiled outer loop: process I_TILE output rows simultaneously so the
    // b_flat[ki*n..(ki+1)*n] slice is shared across I_TILE rows per ki step,
    // reducing the number of times each b_flat chunk is loaded from L3.
    const I_TILE: usize = 4;

    macro_rules! run {
        ($shift:expr) => {{
            // Tiled rows: I_TILE at a time.
            let mut i = 0usize;
            while i + I_TILE <= m {
                // For each inner-dimension step ki, load b_row once and
                // apply it to all I_TILE accumulator rows.
                for ki in 0..k {
                    let b_row = &b_flat[ki * n..(ki + 1) * n];
                    // SAFETY: acc slices do not overlap (different output rows).
                    // We split the mutable slice to get non-overlapping sub-slices.
                    let (acc0, rest) = out[i * n..].split_at_mut(n);
                    let (acc1, rest) = rest.split_at_mut(n);
                    let (acc2, rest) = rest.split_at_mut(n);
                    let (acc3, _) = rest.split_at_mut(n);

                    let a0 = a_flat[(i) * k + ki];
                    let a1 = a_flat[(i + 1) * k + ki];
                    let a2 = a_flat[(i + 2) * k + ki];
                    let a3 = a_flat[(i + 3) * k + ki];

                    gf2m_broadcast_mul_xor::<$shift>(a0, b_row, acc0, mu, modulus);
                    gf2m_broadcast_mul_xor::<$shift>(a1, b_row, acc1, mu, modulus);
                    gf2m_broadcast_mul_xor::<$shift>(a2, b_row, acc2, mu, modulus);
                    gf2m_broadcast_mul_xor::<$shift>(a3, b_row, acc3, mu, modulus);
                }
                i += I_TILE;
            }
            // Tail rows not covered by the I_TILE loop.
            while i < m {
                let acc_row = &mut out[i * n..(i + 1) * n];
                for ki in 0..k {
                    let a_ik = a_flat[i * k + ki];
                    let b_row = &b_flat[ki * n..(ki + 1) * n];
                    gf2m_broadcast_mul_xor::<$shift>(a_ik, b_row, acc_row, mu, modulus);
                }
                i += 1;
            }
        }};
    }

    match degree {
        8 => run!(1),
        16 => run!(2),
        32 => run!(4),
        _ => {
            // Unsupported degree: scalar Barrett fallback.
            let mask = (1u64 << degree) - 1;
            let _ = mask;
            for i in 0..m {
                let acc_row = &mut out[i * n..(i + 1) * n];
                for ki in 0..k {
                    let a_ik = a_flat[i * k + ki];
                    if a_ik == 0 {
                        continue;
                    }
                    let b_row = &b_flat[ki * n..(ki + 1) * n];
                    for (j, &b_kj) in b_row.iter().enumerate() {
                        acc_row[j] ^= clmul_barrett_scalar(a_ik, b_kj, mu, modulus, degree);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::arch::is_x86_feature_detected;

    fn avx2_vpclmul_available() -> bool {
        is_x86_feature_detected!("avx2")
            && is_x86_feature_detected!("vpclmulqdq")
            && is_x86_feature_detected!("pclmulqdq")
    }

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

    /// Reference triple-loop scalar GEMM.
    fn scalar_gemm(
        a: &[u64],
        b: &[u64],
        out: &mut [u64],
        m: usize,
        k: usize,
        n: usize,
        modulus: u64,
        degree: u32,
    ) {
        let mu = compute_mu(modulus, degree);
        for i in 0..m {
            for j in 0..n {
                let mut acc = 0u64;
                for ki in 0..k {
                    let p = unsafe {
                        clmul_barrett_scalar(a[i * k + ki], b[ki * n + j], mu, modulus, degree)
                    };
                    acc ^= p;
                }
                out[i * n + j] = acc;
            }
        }
    }

    fn run_test(m: usize, k: usize, n: usize, degree: u32, modulus: u64) {
        if !avx2_vpclmul_available() {
            return;
        }
        let mu = compute_mu(modulus, degree);
        let mask = (1u64 << degree) - 1;

        let a: Vec<u64> = (0..m * k)
            .map(|i| (i as u64).wrapping_mul(0x9E37_79B9) & mask)
            .collect();
        let b: Vec<u64> = (0..k * n)
            .map(|i| (i as u64).wrapping_mul(0x6C62_272E + 7) & mask)
            .collect();

        let mut got = vec![0u64; m * n];
        unsafe {
            gf2m_gemm_panelized(&a, &b, &mut got, m, k, n, mu, modulus, degree);
        }

        let mut expected = vec![0u64; m * n];
        scalar_gemm(&a, &b, &mut expected, m, k, n, modulus, degree);

        for (idx, (&g, &e)) in got.iter().zip(expected.iter()).enumerate() {
            assert_eq!(g, e, "m={m} k={k} n={n} degree={degree} idx={idx}");
        }
    }

    #[test]
    fn panelized_gemm_gf256_square() {
        let poly: u64 = 0b100011101;
        run_test(8, 8, 8, 8, poly);
        run_test(16, 16, 16, 8, poly);
    }

    #[test]
    fn panelized_gemm_gf256_large() {
        let poly: u64 = 0b100011101;
        run_test(32, 32, 32, 8, poly);
    }

    #[test]
    fn panelized_gemm_gf256_rectangular() {
        let poly: u64 = 0b100011101;
        run_test(4, 7, 5, 8, poly);
        run_test(16, 8, 32, 8, poly);
    }

    #[test]
    fn panelized_gemm_gf65536_square() {
        let poly: u64 = 0b1_0001_0000_0000_1011;
        run_test(8, 8, 8, 16, poly);
        run_test(16, 16, 16, 16, poly);
    }

    #[test]
    fn panelized_gemm_gf2pow32_square() {
        let poly: u64 = 0b1_0000_0000_0100_0000_0000_0000_0000_0111;
        run_test(4, 4, 4, 32, poly);
        run_test(8, 8, 8, 32, poly);
    }

    #[test]
    fn panelized_gemm_tail_sizes() {
        let poly: u64 = 0b100011101;
        run_test(3, 5, 7, 8, poly);
        run_test(5, 3, 5, 8, poly);
    }

    #[test]
    fn broadcast_mul_xor_gf256() {
        if !avx2_vpclmul_available() {
            return;
        }
        let poly: u64 = 0b100011101;
        let mu = compute_mu(poly, 8);
        let mask = 0xFFu64;
        let a_ik = 0x53u64;
        let b_row: Vec<u64> = (0..16u64).map(|j| (j * 0x1B + 1) & mask).collect();
        let mut acc = vec![0u64; 16];

        unsafe {
            gf2m_broadcast_mul_xor::<1>(a_ik, &b_row, &mut acc, mu, poly);
        }

        // Cross-check against scalar.
        for (j, &got) in acc.iter().enumerate() {
            let expected = unsafe { clmul_barrett_scalar(a_ik, b_row[j], mu, poly, 8) };
            assert_eq!(got, expected, "j={j}");
        }
    }
}
