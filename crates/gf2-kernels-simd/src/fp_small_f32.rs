//! AVX2 + FMA3 (`_mm256_fmadd_ps`) f32-cascade GEMM kernel for small
//! `Fp<P>` with `P <= 251`.
//!
//! [`detect`] returns safe function-pointer wrappers in
//! [`SmallPrimeF32Fns`], or `None` without AVX2 and FMA3. The two kernels
//! differ only in output reduction: `batch_gemm_fn` reduces each cell with a
//! scalar `% p`, `batch_gemm_route_a_fn` with a 32-bit-lane AVX2 Barrett
//! reduction.

#![allow(clippy::missing_safety_doc)]

/// Whole-gemm fast path for canonical-residue `Fp<P>` operands with
/// `P <= 251`, dispatched on AVX2 + FMA3 hosts.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`, where `bt` is the row-major
/// transpose of the right operand (length `n * k`). Inputs are `f32`
/// lanes carrying canonical residues in `[0, p)`; outputs are
/// canonical bytes in `[0, p)`; `p` is an odd prime in `[3, 251]`.
///
/// # Panics
///
/// Panics unless `a.len() == m * k`, `bt.len() == n * k` and
/// `c.len() == m * n`.
pub type SmallPrimeF32GemmFn = fn(&[f32], &[f32], usize, usize, usize, u8, &mut [u8]);

/// Bundle of small-prime f32-FMA SIMD batch operations.
///
/// Populated at runtime by [`detect`] when both AVX2 and FMA3 are
/// available. The prime `p` is a runtime argument.
#[derive(Copy, Clone)]
pub struct SmallPrimeF32Fns {
    /// Whole-gemm `Fp<P>` AVX2 + FMA3 f32-cascade kernel for `P <= 251`.
    pub batch_gemm_fn: SmallPrimeF32GemmFn,
    /// Variant of `batch_gemm_fn` that reduces each output tile with an
    /// AVX2 32-bit-lane Barrett reduction instead of a per-cell scalar
    /// `% p`.
    pub batch_gemm_route_a_fn: SmallPrimeF32GemmFn,
}

/// Detect and return the best available small-prime f32-FMA SIMD
/// function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// either AVX2 or FMA3.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_small_f32;
///
/// if let Some(fns) = fp_small_f32::detect() {
///     // Compute `[1, 2, 3, 4] · diag([1, 1, 1, 1]) mod 7 = [1, 2, 3, 4]`.
///     let a = [1.0f32, 2.0, 3.0, 4.0];
///     // 4×4 identity transpose stored row-major (it equals itself).
///     let bt = [
///         1.0f32, 0.0, 0.0, 0.0,
///         0.0, 1.0, 0.0, 0.0,
///         0.0, 0.0, 1.0, 0.0,
///         0.0, 0.0, 0.0, 1.0,
///     ];
///     let mut out = [0u8; 4];
///     (fns.batch_gemm_fn)(&a, &bt, 1, 4, 4, 7, &mut out);
///     assert_eq!(out, [1, 2, 3, 4]);
/// }
/// ```
pub fn detect() -> Option<SmallPrimeF32Fns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<SmallPrimeF32Fns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
        Some(SmallPrimeF32Fns {
            batch_gemm_fn: batch_gemm_safe,
            batch_gemm_route_a_fn: batch_gemm_route_a_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_gemm_safe(a: &[f32], bt: &[f32], m: usize, k: usize, n: usize, p: u8, c: &mut [u8]) {
    // Safety: `detect_x86` only returns this pointer when AVX2 + FMA3
    // are both available at runtime.
    unsafe { crate::x86::fp_small_f32::fp_small_f32_gemm(a, bt, m, k, n, p, c) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_gemm_route_a_safe(
    a: &[f32],
    bt: &[f32],
    m: usize,
    k: usize,
    n: usize,
    p: u8,
    c: &mut [u8],
) {
    // Safety: `detect_x86` only returns this pointer when AVX2 + FMA3
    // are both available at runtime.
    unsafe { crate::x86::fp_small_f32::fp_small_f32_gemm_route_a(a, bt, m, k, n, p, c) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_some_only_when_avx2_fma_present() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            let expected = is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma");
            assert_eq!(fns.is_some(), expected);
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            assert!(fns.is_none());
        }
    }

    fn scalar_gemm(a: &[u8], bt: &[u8], m: usize, k: usize, n: usize, p: u8) -> Vec<u8> {
        let mut out = vec![0u8; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut acc: u64 = 0;
                for t in 0..k {
                    acc += a[i * k + t] as u64 * bt[j * k + t] as u64;
                }
                out[i * n + j] = (acc % p as u64) as u8;
            }
        }
        out
    }

    fn u8_to_f32(xs: &[u8]) -> Vec<f32> {
        xs.iter().map(|&b| b as f32).collect()
    }

    #[test]
    fn safe_wrapper_matches_scalar_gemm() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[7u8, 31, 251] {
            for &(m, k, n) in &[
                (1usize, 1usize, 1usize),
                (1, 4, 4),
                (3, 5, 7),
                (4, 64, 24),
                (8, 64, 32),
                (16, 134, 16),
                (16, 134, 24),
                (4, 65, 25),
            ] {
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let a_f = u8_to_f32(&a);
                let bt_f = u8_to_f32(&bt);
                let mut got = vec![0u8; m * n];
                (fns.batch_gemm_fn)(&a_f, &bt_f, m, k, n, p, &mut got);
                let expected = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, expected, "p={p} m={m} k={k} n={n}");
            }
        }
    }

    #[test]
    fn route_a_safe_wrapper_matches_scalar_gemm() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[7u8, 31, 127, 251] {
            for &(m, k, n) in &[
                (1usize, 1usize, 1usize),
                (1, 4, 4),
                (3, 5, 7),
                (4, 64, 24),
                (8, 64, 32),
                (16, 134, 16),
                (16, 134, 24),
                (4, 65, 25),
                // Panel / k_max boundary cases for route-A.
                (4, 256, 256),
                (4, 1024, 1024),
                (4, 267, 24), // just under k_max(251) = 268
                (4, 268, 24), // exactly k_max(251) = 268
                (4, 269, 24), // just over → 2 chunks
            ] {
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let a_f = u8_to_f32(&a);
                let bt_f = u8_to_f32(&bt);
                let mut got = vec![0u8; m * n];
                (fns.batch_gemm_route_a_fn)(&a_f, &bt_f, m, k, n, p, &mut got);
                let expected = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, expected, "route-A p={p} m={m} k={k} n={n}");
            }
        }
    }

    #[test]
    fn safe_wrapper_handles_zero_dims() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        // `m == 0` or `n == 0` → output is empty; kernel must not panic.
        let a: Vec<f32> = vec![];
        let bt: Vec<f32> = vec![];
        let mut out: Vec<u8> = vec![];
        (fns.batch_gemm_fn)(&a, &bt, 0, 0, 0, 7, &mut out);
        assert!(out.is_empty());
    }
}
