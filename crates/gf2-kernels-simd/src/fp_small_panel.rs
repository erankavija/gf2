//! AVX2 pure-integer Goto/BLIS-style panelized GEMM kernel for small
//! `Fp<P>` with `P <= 251` — **Route C** of the jit:615db3b9 Phase 1
//! plan.
//!
//! This is the safe wrapper layer; the unsafe AVX2 intrinsics live in
//! `crate::x86::fp_small_panel`. The kernel is one of three prototype
//! routes the plan explores for GF(251) at n ∈ {256, 1024}:
//!
//! - **Route A** (`crate::fp_small_f32`, jit:68cdf4c8) — in-Rust
//!   f32/FMA cascade. Closed; PASS at n=1024, SHORTFALL at n=256.
//! - **Route B** (`dev/research/blas_sgemm_gf251/`, jit:91429c1c) —
//!   OpenBLAS sgemm cascade. Closed; SHORTFALL at both cells.
//! - **Route C** (this kernel, jit:fc182ed5) — pure-integer
//!   Goto/BLIS panelized micro-kernel. See
//!   `dev/active/fc182ed5/fc182ed5-route-c-design.md` for the panel
//!   dimension derivation (`MR × NR × KC = 4 × 24 × 256`) and the
//!   `_mm256_madd_epi16` inner-loop structure.
//!
//! Provenance: implemented from public Goto-vandeGeijn 2008 / BLIS 2015
//! framework and the AMD Zen 3 Software Optimization Guide; no
//! fflas-ffpack source, comments, or autotuning tables consulted.
//!
//! # Algorithm
//!
//! Operates on **canonical bytes** (each element in `[0, p)`), same
//! representation as the Candidate C kernel (`crate::fp_small`). The
//! inner kernel uses `_mm256_madd_epi16` lane-pair MAC against
//! pre-packed A/B panels:
//!
//! 1. **Pack pass.** A is repacked into MR-row horizontal panels with
//!    pair-of-bytes interleaving (one `Vec<u32>` of size
//!    `MR · ceil(k/2)` per MR-row block). B is repacked into NR-column
//!    vertical panels with pair-of-rows interleaving (one
//!    `Vec<u8>` of size `n_panels · (k_padded · NR)` shared across all
//!    MR-row blocks).
//! 2. **Inner kernel.** 12 u32 SIMD accumulators × `kc / 2` t-pair
//!    steps per cache chunk. Each step issues 3 b-pair loads + 4
//!    a-pair broadcasts + 12 `_mm256_madd_epi16` + 12 `_mm256_add_epi32`.
//! 3. **Reduction.** At panel boundary, the 12 u32 vectors are
//!    Barrett-reduced mod p via the SSOT
//!    `crate::x86::fp_small::barrett_reduce_lane32` reducer
//!    (same reducer route A delegates to and Candidate C's SpMM
//!    row reducer uses) and packed to u8 bytes via the SSOT
//!    `_mm256_packus_epi32` + `_mm256_permute4x64_epi64` +
//!    `_mm256_packus_epi16` sequence.
//!
//! # Bound for `_mm256_madd_epi16` accumulation
//!
//! Each `_mm256_madd_epi16` lane sums two u16 products, each
//! `≤ (p − 1)² ≤ 250² = 62 500` at `p = 251`. Across the full k axis
//! the lane sum is `≤ k · (p − 1)²`. For u32 lanes (`2³² ≈ 4.29 · 10⁹`)
//! this bounds `k ≤ 2³² / (p − 1)²`; at `p = 251` the cap is
//! `k ≤ 68 719`, far above any in-scope cell. KC = 256 is therefore
//! L1d-fit-bound (not arithmetic-bound).
//!
//! # Safety contract
//!
//! All public functions here are safe; they dispatch to the
//! `unsafe` AVX2 intrinsics in `crate::x86::fp_small_panel` only
//! when [`detect`] has returned `Some(_)` (i.e. AVX2 is available
//! at runtime). Callers without AVX2 receive `None` and must fall
//! back to Candidate C or scalar.

/// Whole-GEMM panelized integer kernel signature for `Fp<P>` with
/// `P <= 251`.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`. Inputs are canonical bytes
/// (`< p`); outputs are canonical bytes (`< p`). `bt` is the
/// row-major transpose of B (length `n · k`, so row `j` holds
/// column `j` of B).
///
/// # Arguments
///
/// * `a` — left input row-major, length `m · k`, canonical bytes.
/// * `bt` — right operand's row-major transpose, length `n · k`,
///   canonical bytes.
/// * `m`, `k`, `n` — matrix shapes.
/// * `p` — odd prime in `[3, 251]`.
/// * `c` — destination row-major, length `m · n`, canonical bytes.
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
pub type SmallPrimePanelGemmFn = fn(&[u8], &[u8], usize, usize, usize, u8, &mut [u8]);

/// Bundle of small-prime panelized integer GEMM operations
/// (route C, jit:fc182ed5).
///
/// Populated at runtime by [`detect`] when AVX2 is available. The
/// function pointer takes the prime `p` as a runtime argument so
/// one dispatch struct covers GF(7), GF(31), GF(251), and any other
/// `P ≤ 251` consumer.
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
/// AVX2. Callers receive `None` and must fall back to
/// [`crate::fp_small::detect`]'s row-panel kernel (Candidate C) or
/// scalar.
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
