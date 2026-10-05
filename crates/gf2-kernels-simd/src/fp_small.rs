//! SIMD batch kernels for small `Fp<P>` with `P <= 251`, operating on canonical
//! byte slices (each element in `[0, P)`) with 16-bit lane Barrett reduction.
//! [`detect`] returns safe function-pointer wrappers in [`SmallPrimeFns`], or
//! `None` without AVX2.

/// Lane-wise batch multiply for a small prime `Fp<P>` with `P <= 251`.
///
/// Computes `out[i] = a[i] * b[i] mod p` for all `i < a.len()`.
/// Inputs and outputs are canonical bytes in `[0, p)`; `p` is an odd prime
/// in `[3, 251]`.
///
/// # Panics
///
/// Panics if the slice lengths differ.
pub type SmallPrimeBatchMulFn = fn(&[u8], &[u8], u8, &mut [u8]);

/// Lane-wise batch addition for `Fp<P>` with `P <= 251`.
///
/// Computes `out[i] = (a[i] + b[i]) mod p`.
///
/// # Panics
///
/// Panics if the slice lengths differ.
pub type SmallPrimeBatchAddFn = fn(&[u8], &[u8], u8, &mut [u8]);

/// Lane-wise batch subtraction for `Fp<P>` with `P <= 251`.
///
/// Computes `out[i] = (a[i] - b[i]) mod p` with the result in canonical
/// form `[0, p)`.
///
/// # Panics
///
/// Panics if the slice lengths differ.
pub type SmallPrimeBatchSubFn = fn(&[u8], &[u8], u8, &mut [u8]);

/// Batch dot product for `Fp<P>` with `P <= 251`.
///
/// Returns `sum_i (a[i] * b[i]) mod p` as a canonical `u8`. Inputs must
/// be canonical (`< p`).
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
pub type SmallPrimeBatchDotFn = fn(&[u8], &[u8], u8) -> u8;

/// Whole-row gemm panel for `Fp<P>` with `P <= 251`.
///
/// Computes `out[j] = (∑_t a[t] * bt[j*k + t]) mod p` for `j ∈ [0, n)`,
/// where `a` is one length-`k` row and `bt` is the `n × k` row-major
/// transpose of the right operand.
///
/// # Panics
///
/// Panics unless `a.len() == k`, `bt.len() == n * k` and `out.len() == n`.
pub type SmallPrimeGemmRowPanelFn = fn(&[u8], &[u8], usize, usize, u8, &mut [u8]);

/// Sparse-times-dense row kernel for `Fp<P>` with `P <= 251`.
///
/// Writes `out[j] = (∑_h a_vals[h] * b[a_cols[h] * b_stride + j]) mod p`
/// for `j ∈ [0, n)`. The sparse left row is given as `(a_vals, a_cols)`
/// and `b` is a row-major dense byte matrix with row stride `b_stride`.
///
/// Each index must satisfy `a_cols[h] * b_stride + n <= b.len()`, and
/// `n <= b_stride`.
///
/// # Panics
///
/// Panics if `a_vals.len() != a_cols.len()` or `out.len() != n`.
pub type SmallPrimeSpmmRowFn = fn(&[u8], &[usize], &[u8], usize, usize, u8, &mut [u8]);

/// Fused in-place `buf := (buf − α · chain_j) mod p` for `Fp<P>`
/// with `P <= 251`.
///
/// The arguments are `(buf, chain_j, alpha, p, mu)`: `buf` and `chain_j`
/// hold canonical bytes, `alpha ∈ [0, p)`, `p` is an odd prime in
/// `[3, 251]`, and `mu = ⌊2¹⁶ / p⌋` ([`barrett_mu_u16`]). Bytes of `buf`
/// beyond `chain_j.len()` are untouched.
///
/// # Panics
///
/// Panics if `buf.len() < chain_j.len()`.
pub type SmallPrimeSubScaledFn = fn(&mut [u8], &[u8], u8, u8, u16);

/// Bundle of small-prime SIMD batch operations.
///
/// Populated at runtime by [`detect`] when AVX2 is available. The prime `p`
/// is a runtime argument.
#[derive(Copy, Clone)]
pub struct SmallPrimeFns {
    /// Lane-wise batch multiply for `Fp<P>` with `P <= 251`.
    pub batch_mul_fn: SmallPrimeBatchMulFn,
    /// Lane-wise batch addition for `Fp<P>` with `P <= 251`.
    pub batch_add_fn: SmallPrimeBatchAddFn,
    /// Lane-wise batch subtraction for `Fp<P>` with `P <= 251`.
    pub batch_sub_fn: SmallPrimeBatchSubFn,
    /// Batch dot product reduced to scalar for `Fp<P>` with `P <= 251`.
    pub batch_dot_fn: SmallPrimeBatchDotFn,
    /// Whole-row gemm panel for `Fp<P>` with `P <= 251`.
    pub gemm_row_panel_fn: SmallPrimeGemmRowPanelFn,
    /// Sparse-times-dense row kernel for `Fp<P>` with `P <= 251`.
    pub spmm_row_fn: SmallPrimeSpmmRowFn,
    /// Fused in-place `buf := (buf − α · chain_j) mod p` for `Fp<P>`
    /// with `P <= 251`.
    pub sub_scaled_fn: SmallPrimeSubScaledFn,
}

/// Detect and return the best available small-prime SIMD function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// AVX2.
pub fn detect() -> Option<SmallPrimeFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<SmallPrimeFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(SmallPrimeFns {
            batch_mul_fn: batch_mul_safe,
            batch_add_fn: batch_add_safe,
            batch_sub_fn: batch_sub_safe,
            batch_dot_fn: batch_dot_safe,
            gemm_row_panel_fn: gemm_row_panel_safe,
            spmm_row_fn: spmm_row_safe,
            sub_scaled_fn: sub_scaled_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_mul_safe(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    // SAFETY: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_mul(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_add_safe(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    // SAFETY: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_add(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_sub_safe(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    // SAFETY: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_sub(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_dot_safe(a: &[u8], b: &[u8], p: u8) -> u8 {
    // SAFETY: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_dot(a, b, p) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn gemm_row_panel_safe(a: &[u8], bt: &[u8], k: usize, n: usize, p: u8, out: &mut [u8]) {
    // SAFETY: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_gemm_row_panel(a, bt, k, n, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn spmm_row_safe(
    a_vals: &[u8],
    a_cols: &[usize],
    b: &[u8],
    b_stride: usize,
    n: usize,
    p: u8,
    out: &mut [u8],
) {
    // SAFETY: `detect_x86` only returns these pointers when AVX2 is available.
    // The column bound is the caller's, as `SmallPrimeSpmmRowFn` documents; the
    // kernel asserts the other lengths.
    unsafe { crate::x86::fp_small::fp_small_spmm_row(a_vals, a_cols, b, b_stride, n, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn sub_scaled_safe(buf: &mut [u8], chain_j: &[u8], alpha: u8, p: u8, mu: u16) {
    // SAFETY: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_sub_scaled(buf, chain_j, alpha, p, mu) }
}

/// Returns the 16-bit Barrett constant `μ = ⌊2¹⁶ / p⌋` for an odd prime
/// `p ∈ [3, 255]`, as [`SmallPrimeSubScaledFn`] takes it.
///
/// # Panics
///
/// Debug builds panic if `p < 3`.
#[inline]
pub const fn barrett_mu_u16(p: u8) -> u16 {
    debug_assert!(p >= 3);
    (65536u32 / p as u32) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_some_on_avx2() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("avx2") {
                assert!(fns.is_some());
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let _ = fns;
        }
    }

    fn scalar_mul(a: u8, b: u8, p: u8) -> u8 {
        ((a as u32 * b as u32) % p as u32) as u8
    }

    fn scalar_add(a: u8, b: u8, p: u8) -> u8 {
        ((a as u32 + b as u32) % p as u32) as u8
    }

    fn scalar_sub(a: u8, b: u8, p: u8) -> u8 {
        ((a as u32 + p as u32 - b as u32) % p as u32) as u8
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_mul() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[7u8, 31, 251] {
            for &len in &[0usize, 1, 15, 16, 17, 31, 32, 33, 100, 1024] {
                let a: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 17) % p as u32) as u8)
                    .collect();
                let b: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut out = vec![0u8; len];
                (fns.batch_mul_fn)(&a, &b, p, &mut out);
                for i in 0..len {
                    assert_eq!(out[i], scalar_mul(a[i], b[i], p), "p={p} len={len} i={i}");
                }
            }
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_add() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[7u8, 31, 251] {
            for &len in &[0usize, 1, 15, 31, 32, 33, 64, 256] {
                let a: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 17) % p as u32) as u8)
                    .collect();
                let b: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut out = vec![0u8; len];
                (fns.batch_add_fn)(&a, &b, p, &mut out);
                for i in 0..len {
                    assert_eq!(out[i], scalar_add(a[i], b[i], p), "p={p} len={len} i={i}");
                }
            }
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_sub() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[7u8, 31, 251] {
            for &len in &[0usize, 1, 15, 31, 32, 33, 64, 256] {
                let a: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 17) % p as u32) as u8)
                    .collect();
                let b: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut out = vec![0u8; len];
                (fns.batch_sub_fn)(&a, &b, p, &mut out);
                for i in 0..len {
                    assert_eq!(out[i], scalar_sub(a[i], b[i], p), "p={p} len={len} i={i}");
                }
            }
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_dot() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[7u8, 31, 251] {
            for &len in &[0usize, 1, 7, 8, 15, 31, 32, 33, 100, 1024] {
                let a: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 17) % p as u32) as u8)
                    .collect();
                let b: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let got = (fns.batch_dot_fn)(&a, &b, p);
                let mut expected: u32 = 0;
                for i in 0..len {
                    expected = (expected + a[i] as u32 * b[i] as u32) % p as u32;
                }
                assert_eq!(got, expected as u8, "p={p} len={len}");
            }
        }
    }

    /// Wrapper-layer parity for the `sub_scaled` fused kernel at boundary
    /// lengths.
    #[allow(clippy::wildcard_imports)]
    mod proptest_safe_wrapper_sub_scaled_jit_52cce970 {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(256))]

            #[test]
            fn proptest_safe_wrapper_matches_scalar_sub_scaled(
                len in prop_oneof![
                    Just(0usize), Just(1), Just(15), Just(16), Just(17),
                    Just(63), Just(64), Just(65), Just(255), Just(256)
                ],
                seed in any::<u64>(),
                p_idx in 0usize..3usize,
            ) {
                let fns = match detect() {
                    Some(f) => f,
                    None => return Ok(()),
                };
                let primes: [u8; 3] = [7, 31, 251];
                let p = primes[p_idx];
                let mu = barrett_mu_u16(p);
                let s1 = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let s2 = s1.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let alpha = ((s1 >> 32) % p as u64) as u8;
                let chain_j: Vec<u8> = (0..len)
                    .map(|i| {
                        let v = s1.wrapping_mul(i as u64 + 1).wrapping_add(s2);
                        (v % p as u64) as u8
                    })
                    .collect();
                let buf_init: Vec<u8> = (0..len)
                    .map(|i| {
                        let v = s2.wrapping_mul(i as u64 + 1).wrapping_add(s1);
                        (v % p as u64) as u8
                    })
                    .collect();
                let mut buf = buf_init.clone();
                let p_u32 = p as u32;
                let mut expected = buf_init;
                for i in 0..len {
                    let prod = (alpha as u32 * chain_j[i] as u32) % p_u32;
                    expected[i] = ((expected[i] as u32 + p_u32 - prod) % p_u32) as u8;
                }
                (fns.sub_scaled_fn)(&mut buf, &chain_j, alpha, p, mu);
                prop_assert_eq!(buf, expected, "p={} alpha={} len={}", p, alpha, len);
            }
        }
    }

    /// Deterministic boundary-length check.
    #[test]
    fn safe_wrapper_matches_scalar_sub_scaled() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[7u8, 31, 251] {
            let mu = barrett_mu_u16(p);
            for &len in &[0usize, 1, 15, 16, 17, 63, 64, 65, 255, 256] {
                let chain_j: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 19 + 5) % p as u32) as u8)
                    .collect();
                let alpha: u8 = ((len as u32 * 11 + 3) % p as u32) as u8;
                let mut buf: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 31 + 11) % p as u32) as u8)
                    .collect();
                let mut expected = buf.clone();
                let p_u32 = p as u32;
                for i in 0..len {
                    let prod = (alpha as u32 * chain_j[i] as u32) % p_u32;
                    expected[i] = ((expected[i] as u32 + p_u32 - prod) % p_u32) as u8;
                }
                (fns.sub_scaled_fn)(&mut buf, &chain_j, alpha, p, mu);
                assert_eq!(buf, expected, "p={p} len={len}");
            }
        }
    }

    #[test]
    fn barrett_mu_u16_returns_correct_value_at_boundaries() {
        for &p in &[3u8, 5, 7, 11, 13, 17, 31, 127, 251] {
            assert_eq!(barrett_mu_u16(p), (65536u32 / p as u32) as u16, "p={p}");
        }
    }
}
