//! AVX2 + FMA3 (`_mm256_fmadd_ps`) f32-cascade GEMM kernel for small
//! `Fp<P>` with `P <= 251`.
//!
//! This is **Candidate F** from `dev/plans/small_prime_kernel_strategy.md`
//! § 4.5 / § 5.5 — the in-Rust f32-FMA cascade selected by the Wave-6B
//! amendment (§ 6.1) as the FMA3-host primary path. The Wave-6B
//! Candidate C kernel in [`crate::fp_small`] remains compiled in as
//! the AVX2-only-no-FMA3 runtime fallback per the same amendment.
//!
//! All unsafe intrinsics are isolated in `x86/fp_small_f32.rs`; this
//! module exposes only safe function-pointer wrappers through the
//! [`SmallPrimeF32Fns`] table returned by [`detect`]. Callers without
//! AVX2 + FMA3 receive `None` and must fall back to Candidate C
//! (or scalar).
//!
//! # Algorithm
//!
//! For canonical inputs `a, b ∈ [0, P) ⊂ [0, 256)`, each gemm call:
//!
//! 1. **Pack pass.** Convert `&[u8]` (canonical-form residues) into
//!    `Vec<f32>` row-major buffers `a_packed` (`m × k`) and
//!    column-major `b_packed` (`k × n`). The per-element conversion
//!    is `(*v as f32)` — exact for `p ≤ 251` because every value
//!    fits in 8 bits, comfortably below f32's 24-bit mantissa.
//!
//! 2. **Inner micro-kernel.** A BLIS-class register-blocked sgemm
//!    micro-kernel with tile shape `m_R × n_R = 4 × 24` (12
//!    accumulator AVX2 registers + 1 broadcast + 3 A-column registers
//!    = 16/16 register file). Each `_mm256_fmadd_ps(b_tile_j,
//!    a_broadcast_i, acc_ij)` issues at 0.5-cycle reciprocal
//!    throughput on Zen-3's two FMA execution ports — twice
//!    Candidate C's `_mm256_madd_epi16` rate.
//!
//!    The k-loop is split into chunks of `k_C` per outer iteration,
//!    where `k_C` is the per-prime `k_max` — the largest number of
//!    `(p-1)²`-magnitude products that f32 can absorb without
//!    rounding loss. For `p ∈ {7, 31, 251}`, `k_max ∈ {4096, 1024, 64}`
//!    respectively. The current implementation uses a uniform
//!    `K_CHUNK = 64` so a single code path covers all in-scope primes
//!    safely; the ILP from FMA-port pipelining is preserved by the
//!    inner-tile fan-out.
//!
//! 3. **Reduction.** At the end of every `k_C` chunk, the 12
//!    accumulator vectors are rounded to nearest integer via
//!    `_mm256_round_ps` and converted to `__m256i` via
//!    `_mm256_cvtps_epi32`, then reduced modulo `p` via scalar `% p`
//!    per lane. Both the round-and-cast cost (one `vroundps` plus
//!    one `vcvtps2dq` per accumulator, ~6 cycles each on Zen-3) and
//!    the scalar reduction cost (`32 lanes / tile × ~5 cycles/div`)
//!    are paid once per `k_C` chunk, not once per FMA — so they
//!    amortise across the inner loop.
//!
//! 4. **Unpack pass.** Copy the canonical-byte output buffer back to
//!    the caller's `&mut [u8]` storage.
//!
//! # Throughput envelope
//!
//! Per Zen-3 micro-architecture (§ 5.5 (b) of the design): two FMA
//! ports each retiring 8 f32 lanes per cycle = 32 ops/cycle in the
//! bench's `2 m k n` op-count metric. At a 5 GHz boost on the 5900X
//! reference host the peak is **160 Gop/s**, exactly twice
//! Candidate C's `_mm256_madd_epi16` peak of 80 Gop/s. The
//! pack-amortisation derivation in § 6.1 shows F overtakes C at
//! `n ≥ 32` at the issue's empirical pack-cost factor.
//!
//! # Soundness for `p ≤ 251`
//!
//! After `k_C = 64` FMAs of pairs in `[0, 250]`, every accumulator
//! lane holds a non-negative integer ≤ `64 · 250² = 4 000 000`,
//! comfortably below f32's exact-integer range `[0, 2^24] = [0,
//! 16 777 216]`. Each `_mm256_fmadd_ps` therefore produces an exact
//! integer result — no rounding occurs in the inner loop — and the
//! `_mm256_round_ps(_, _MM_FROUND_TO_NEAREST_INT)` + `cvtps_epi32`
//! at chunk end is a no-op semantically (the value is already an
//! integer). Reductions are computed in `i32` lanes via scalar
//! `% p`, and the `i32` accumulator further absorbs the cross-chunk
//! sum without overflow because `n / 64 · 4 000 000 < 2^31` for
//! every `n ≤ 1024`.

#![allow(clippy::missing_safety_doc)]

/// Whole-gemm fast path for canonical-byte `Fp<P>` operands with
/// `P <= 251`, dispatched on AVX2 + FMA3 hosts.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`, where `bt` is the row-major
/// transpose of the right operand (length `n * k`). Inputs and
/// outputs are canonical bytes in `[0, p)`.
///
/// # Arguments
///
/// * `a` — left input in row-major, length `m * k`.
/// * `bt` — right operand's row-major transpose, length `n * k`.
/// * `m`, `k`, `n` — matrix shapes.
/// * `p` — odd prime in `[3, 251]`.
/// * `c` — destination in row-major, length `m * n`. Caller-allocated.
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
pub type SmallPrimeF32GemmFn = fn(&[u8], &[u8], usize, usize, usize, u8, &mut [u8]);

/// Bundle of small-prime f32-FMA SIMD batch operations.
///
/// Populated at runtime by [`detect`] when both AVX2 and FMA3 are
/// available. The function pointer takes the prime `p` as a runtime
/// argument so a single dispatch struct covers GF(7), GF(31),
/// GF(251), and any other `P ≤ 251` consumers.
#[derive(Copy, Clone)]
pub struct SmallPrimeF32Fns {
    /// Whole-gemm `Fp<P>` AVX2 + FMA3 f32-cascade kernel for `P <= 251`.
    pub batch_gemm_fn: SmallPrimeF32GemmFn,
}

/// Detect and return the best available small-prime f32-FMA SIMD
/// function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// either AVX2 or FMA3. Callers receive `None` and must fall back to
/// Candidate C ([`crate::fp_small::detect`]) or scalar.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_small_f32;
///
/// if let Some(fns) = fp_small_f32::detect() {
///     // Compute `[1, 2, 3, 4] · diag([1, 1, 1, 1]) mod 7 = [1, 2, 3, 4]`.
///     let a = [1u8, 2, 3, 4];
///     // 4×4 identity transpose stored row-major (it equals itself).
///     let bt = [
///         1u8, 0, 0, 0,
///         0, 1, 0, 0,
///         0, 0, 1, 0,
///         0, 0, 0, 1,
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
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_gemm_safe(a: &[u8], bt: &[u8], m: usize, k: usize, n: usize, p: u8, c: &mut [u8]) {
    // Safety: `detect_x86` only returns this pointer when AVX2 + FMA3
    // are both available at runtime.
    unsafe { crate::x86::fp_small_f32::fp_small_f32_gemm(a, bt, m, k, n, p, c) }
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
        // `m == 0` or `n == 0` → output is empty; kernel must not panic.
        let a: Vec<u8> = vec![];
        let bt: Vec<u8> = vec![];
        let mut out: Vec<u8> = vec![];
        (fns.batch_gemm_fn)(&a, &bt, 0, 0, 0, 7, &mut out);
        assert!(out.is_empty());
    }
}
