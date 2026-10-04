//! AVX2 + FMA3 (`_mm256_fmadd_pd`) f64-cascade GEMM kernel for medium `Fp<P>`
//! with `P ∈ (251, 65536)`. [`detect`] returns the safe function-pointer
//! wrapper in [`FpMediumF64Fns`], or `None` without AVX2 and FMA3; callers then
//! fall back to the u16-lane [`crate::fp_medium`] panel kernel or scalar code.

/// Whole-gemm fast path for canonical-residue `Fp<P>` operands with
/// `P ∈ (251, 65535]`, dispatched on AVX2 + FMA3 hosts.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`, where `bt` is the row-major
/// transpose of the right operand (length `n * k`). Inputs are `f64`
/// lanes carrying canonical residues in `[0, p)`; outputs are
/// canonical u16 cells in `[0, p)`.
///
/// # Panics
///
/// Panics unless `a.len() == m * k`, `bt.len() == n * k` and
/// `c.len() == m * n`.
pub type FpMediumF64GemmFn = fn(&[f64], &[f64], usize, usize, usize, u16, &mut [u16]);

/// Bundle of medium-prime f64-FMA SIMD batch operations.
///
/// Populated at runtime by [`detect`] when both AVX2 and FMA3 are
/// available. The prime `p` is a runtime argument.
#[derive(Copy, Clone)]
pub struct FpMediumF64Fns {
    /// Whole-gemm `Fp<P>` AVX2 + FMA3 f64-cascade kernel for medium primes.
    pub batch_gemm_fn: FpMediumF64GemmFn,
}

/// Detect and return the best available medium-prime f64-FMA SIMD
/// function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// either AVX2 or FMA3.
pub fn detect() -> Option<FpMediumF64Fns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<FpMediumF64Fns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
        Some(FpMediumF64Fns {
            batch_gemm_fn: batch_gemm_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_gemm_safe(a: &[f64], bt: &[f64], m: usize, k: usize, n: usize, p: u16, c: &mut [u16]) {
    // Safety: `detect_x86` only returns this pointer when AVX2 + FMA3
    // are both available at runtime.
    unsafe { crate::x86::fp_medium_f64::fp_medium_f64_gemm(a, bt, m, k, n, p, c) }
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

    fn scalar_gemm(a: &[u16], bt: &[u16], m: usize, k: usize, n: usize, p: u16) -> Vec<u16> {
        let mut out = vec![0u16; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut acc: u64 = 0;
                for t in 0..k {
                    acc += a[i * k + t] as u64 * bt[j * k + t] as u64;
                }
                out[i * n + j] = (acc % p as u64) as u16;
            }
        }
        out
    }

    fn u16_to_f64(xs: &[u16]) -> Vec<f64> {
        xs.iter().map(|&x| x as f64).collect()
    }

    #[test]
    fn safe_wrapper_matches_scalar_gemm() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[257u16, 1031, 4099, 32771, 65521] {
            for &(m, k, n) in &[
                (1usize, 1usize, 1usize),
                (1, 4, 4),
                (3, 5, 7),
                (4, 64, 12),
                (8, 64, 32),
                (16, 134, 16),
                (16, 134, 24),
                (4, 65, 25),
            ] {
                let a: Vec<u16> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u16)
                    .collect();
                let bt: Vec<u16> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u16)
                    .collect();
                let a_f = u16_to_f64(&a);
                let bt_f = u16_to_f64(&bt);
                let mut got = vec![0u16; m * n];
                (fns.batch_gemm_fn)(&a_f, &bt_f, m, k, n, p, &mut got);
                let expected = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, expected, "p={p} m={m} k={k} n={n}");
            }
        }
    }

    #[test]
    fn safe_wrapper_handles_zero_dims() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        let a: Vec<f64> = vec![];
        let bt: Vec<f64> = vec![];
        let mut out: Vec<u16> = vec![];
        (fns.batch_gemm_fn)(&a, &bt, 0, 0, 0, 65521, &mut out);
        assert!(out.is_empty());
    }
}
