//! AVX2 + FMA3 (`_mm256_fmadd_pd`) f64-cascade GEMM kernel for medium
//! `Fp<P>` with `P ∈ (251, 65536)`.
//!
//! This is the **"Phase 6e f64 cascade"** filed under jit issue
//! `0749dbad` — analogous to Route A's f32-FMA cascade for fp_small
//! (`P ≤ 251`), but at f64 lane density so it covers the medium-prime
//! range `(251, 2^16)`. The structural model is documented in
//! `dev/bench_results/695350fd/2026-05-26-695350fd-fp-medium-blis.md` § 9.
//!
//! The unsafe AVX2 + FMA3 implementation lives in
//! [`crate::x86::fp_medium_f64`]; this module exposes only safe
//! function-pointer wrappers through the [`FpMediumF64Fns`] table
//! returned by [`detect`]. Callers without AVX2 + FMA3 receive `None`
//! and must fall back to the u16-lane fp_medium panel kernel (or
//! scalar).
//!
//! # Algorithm overview
//!
//! For canonical residues `a, b ∈ [0, p)` arriving as `f64` lanes (one
//! residue per lane), each gemm call:
//!
//! 1. **Pack pass.** Repack `bt: &[f64]` (n × k row-major) into N-major
//!    f64 panels of width `N_R = 12` (each panel `k × N_R` f64). `a` is
//!    consumed in place; no auxiliary `Vec<f64>` is allocated for A.
//!    Inputs arrive as f64 from the caller's outer pre-pack of
//!    `&[Fp<P>]` → `Vec<f64>`; the kernel's inner loop performs **no
//!    cvt instructions**, mirroring the fflas-ffpack `Modular<double>`
//!    micro-kernel structure.
//!
//! 2. **Inner micro-kernel.** A BLIS-class register-blocked dgemm
//!    micro-kernel with tile shape `m_R × n_R = 4 × 12` (12
//!    accumulator AVX2 registers + 1 broadcast + 3 B-tile registers
//!    = 16/16 register file). Each `_mm256_fmadd_pd(b_tile_j,
//!    a_broadcast_i, acc_ij)` issues at 0.5-cycle reciprocal
//!    throughput on Zen-3's two FMA execution ports.
//!
//! 3. **Reduction.** At the end of the k-axis (or each chunk for
//!    `k > 4096`), the 12 f64 accumulators carry an exact integer in
//!    `[0, 2^53]`. A vectorised f64 Barrett reduction
//!    (`r = x - p · round(x · (1/p))` + single conditional fix-up)
//!    brings each lane into `[0, p)`. The reduced lanes are then
//!    written to the caller's `&mut [u16]` output as canonical u16.
//!
//! 4. **Unpack pass.** Output canonical u16 cells are written directly
//!    to the caller's `&mut [u16]` storage; the caller is responsible
//!    for converting back to `Fp<P>` via `Fp::new`.
//!
//! # Throughput envelope
//!
//! Per Zen-3 micro-architecture: two FMA ports each retiring 4 f64
//! lanes per cycle = 16 ops/cycle in the bench's `2 m k n` op-count
//! metric. At a 4.4 GHz boost on the 5900X reference host the peak is
//! **70.4 Gop/s**, exactly matching the fflas-ffpack `Modular<double>`
//! peak (69.72 Gop/s observed at GF(65521)/n=4096).

/// Whole-gemm fast path for canonical-residue `Fp<P>` operands with
/// `P ∈ (251, 65535]`, dispatched on AVX2 + FMA3 hosts.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`, where `bt` is the row-major
/// transpose of the right operand (length `n * k`). Inputs are `f64`
/// lanes carrying canonical residues in `[0, p)`; outputs are
/// canonical u16 cells in `[0, p)`.
///
/// # Arguments
///
/// * `a` — left input in row-major, length `m * k`, canonical
///   residues stored as `f64` (one residue per lane).
/// * `bt` — right operand's row-major transpose, length `n * k`,
///   canonical residues stored as `f64`.
/// * `m`, `k`, `n` — matrix shapes.
/// * `p` — odd prime in `(251, 2^16)`.
/// * `c` — destination in row-major, length `m * n`. Caller-allocated,
///   written as canonical u16 cells.
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
pub type FpMediumF64GemmFn = fn(&[f64], &[f64], usize, usize, usize, u16, &mut [u16]);

/// Bundle of medium-prime f64-FMA SIMD batch operations.
///
/// Populated at runtime by [`detect`] when both AVX2 and FMA3 are
/// available. The function pointer takes the prime `p` as a runtime
/// argument so a single dispatch struct covers GF(257), GF(521),
/// GF(65521), and every other `P ∈ (251, 2^16)` consumer.
#[derive(Copy, Clone)]
pub struct FpMediumF64Fns {
    /// Whole-gemm `Fp<P>` AVX2 + FMA3 f64-cascade kernel for medium primes.
    pub batch_gemm_fn: FpMediumF64GemmFn,
}

/// Detect and return the best available medium-prime f64-FMA SIMD
/// function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// either AVX2 or FMA3. Callers receive `None` and must fall back to
/// the u16-lane `fp_medium` panel kernel ([`crate::fp_medium::detect`])
/// or scalar.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_medium_f64;
///
/// if let Some(fns) = fp_medium_f64::detect() {
///     // Compute `[1, 2, 3, 4] · diag([1, 1, 1, 1]) mod 65521 = [1, 2, 3, 4]`.
///     let a = [1.0f64, 2.0, 3.0, 4.0];
///     // 4×4 identity transpose stored row-major (it equals itself).
///     let bt = [
///         1.0f64, 0.0, 0.0, 0.0,
///         0.0, 1.0, 0.0, 0.0,
///         0.0, 0.0, 1.0, 0.0,
///         0.0, 0.0, 0.0, 1.0,
///     ];
///     let mut out = [0u16; 4];
///     (fns.batch_gemm_fn)(&a, &bt, 1, 4, 4, 65521, &mut out);
///     assert_eq!(out, [1, 2, 3, 4]);
/// }
/// ```
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
        // `m == 0` or `n == 0` → output is empty; kernel must not panic.
        let a: Vec<f64> = vec![];
        let bt: Vec<f64> = vec![];
        let mut out: Vec<u16> = vec![];
        (fns.batch_gemm_fn)(&a, &bt, 0, 0, 0, 65521, &mut out);
        assert!(out.is_empty());
    }
}
