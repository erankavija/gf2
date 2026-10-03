//! AVX2 pure-integer Goto/BLIS-style panelized GEMM kernel for small `Fp<P>`
//! with `P <= 251`.
//!
//! Safe wrapper layer over `crate::x86::fp_small_panel`, operating on
//! canonical bytes (each element in `[0, p)`). [`detect`] returns `None`
//! without AVX2.
//!
//! Provenance: implemented from public Goto-vandeGeijn 2008 / BLIS 2015
//! framework and the AMD Zen 3 Software Optimization Guide; no
//! fflas-ffpack source, comments, or autotuning tables consulted.

/// Cache-blocking factor along the k-axis, chosen to fit L1d. The u32
/// accumulator bound `k ≤ 2³² / (p − 1)²` (`68 719` at `p = 251`) is not
/// binding. Measurement:
/// `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel-aggregate.csv`.
pub const KC: usize = 256;

/// Whole-GEMM panelized integer kernel signature for `Fp<P>` with
/// `P <= 251`.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`. Inputs are canonical bytes
/// (`< p`); outputs are canonical bytes (`< p`). `bt` is the
/// row-major transpose of B (length `n · k`, so row `j` holds
/// column `j` of B).
///
/// `p` is an odd prime in `[3, 251]`.
///
/// # Panics
///
/// Panics unless `a.len() == m * k`, `bt.len() == n * k` and
/// `c.len() == m * n`.
pub type SmallPrimePanelGemmFn = fn(&[u8], &[u8], usize, usize, usize, u8, &mut [u8]);

/// Bundle of small-prime panelized integer GEMM operations.
///
/// Populated at runtime by [`detect`] when AVX2 is available. The prime `p`
/// is a runtime argument.
#[derive(Copy, Clone)]
pub struct SmallPrimePanelFns {
    /// Goto/BLIS-style panelized whole-GEMM kernel for canonical-byte
    /// `Fp<P>` operands with `P ≤ 251`.
    pub batch_gemm_fn: SmallPrimePanelGemmFn,
}

/// Detect and return the best available small-prime panelized integer
/// GEMM kernel.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// AVX2.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_small_panel;
///
/// if let Some(fns) = fp_small_panel::detect() {
///     // 4×4 identity row-major; bt = row-major transpose of identity
///     // (which equals the identity in storage).
///     let a = [1u8, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
///     let bt = [1u8, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
///     let mut out = [0u8; 16];
///     (fns.batch_gemm_fn)(&a, &bt, 4, 4, 4, 7, &mut out);
///     // out is the 4×4 identity in canonical bytes.
///     assert_eq!(out, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
/// }
/// ```
pub fn detect() -> Option<SmallPrimePanelFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<SmallPrimePanelFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(SmallPrimePanelFns {
            batch_gemm_fn: batch_gemm_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_gemm_safe(a: &[u8], bt: &[u8], m: usize, k: usize, n: usize, p: u8, c: &mut [u8]) {
    // Safety: `detect_x86` only returns this pointer when AVX2 is
    // available at runtime.
    unsafe { crate::x86::fp_small_panel::fp_small_panel_gemm(a, bt, m, k, n, p, c) }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(fns.is_none());
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_gemm() {
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
                (4, 256, 256),
                (4, 257, 24),
                (4, 1024, 1024),
            ] {
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut got = vec![0u8; m * n];
                (fns.batch_gemm_fn)(&a, &bt, m, k, n, p, &mut got);
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
        let a: Vec<u8> = vec![];
        let bt: Vec<u8> = vec![];
        let mut out: Vec<u8> = vec![];
        (fns.batch_gemm_fn)(&a, &bt, 0, 0, 0, 7, &mut out);
        assert!(out.is_empty());
    }
}
