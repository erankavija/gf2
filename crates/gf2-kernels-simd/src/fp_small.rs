//! SIMD batch kernels for small `Fp<P>` with `P <= 251`.
//!
//! Targets the GF(7), GF(31), and GF(251) cells of the `cc5de315`
//! GF(p) parity story. Operates on **canonical** byte slices (each
//! element in `[0, P)`) and uses AVX2 byte-to-word lane expansion plus
//! Barrett reduction at 16-bit lane width to deliver a packed
//! lane-parallel multiply / add / sub / dot kernel.
//!
//! All unsafe intrinsics are isolated in `x86/fp_small.rs`; this module
//! exposes only safe function-pointer wrappers through the
//! [`SmallPrimeFns`] table returned by [`detect`]. Callers without AVX2
//! receive `None` and must fall back to scalar loops.
//!
//! # Algorithm
//!
//! For canonical inputs `a, b ∈ [0, P) ⊂ [0, 256)`, the byte-level
//! product is loaded as 16 packed bytes, zero-extended to 16-bit lanes
//! via `_mm256_cvtepu8_epi16` (or `unpacklo_epi8` against zero), then
//! multiplied lane-wise by `_mm256_mullo_epi16`. The product fits in
//! 16 bits because `(P - 1)² ≤ 250² = 62 500 < 65 536`.
//!
//! Reduction modulo `P` uses a single 16-bit Barrett step:
//!
//! ```text
//! μ = ⌊2¹⁶ / P⌋,
//! q = mulhi_u16(n, μ),
//! r = n − q · P,
//! r ← (r ≥ P) ? r − P : r.
//! ```
//!
//! Barrett's classical bound gives `r ∈ [0, 2P)` for any `n ∈ [0, 2¹⁶)`,
//! so a single conditional subtract canonicalises.
//!
//! For the dot-product entry point we accumulate into 32-bit lanes via
//! `_mm256_madd_epi16`, which fuses 16-bit lane-pair multiply + 32-bit
//! lane-pair add in one cycle on Zen 3. The 32-bit accumulator absorbs
//! `⌊2³² / (P − 1)²⌋ ≈ 6.87 × 10⁴` MACs without overflow at `P = 251`,
//! far in excess of any panel size that fits in L1d. We reduce by
//! scalar `% P` once at the end of the dot product.

/// Lane-wise batch multiply for a small prime `Fp<P>` with `P <= 251`.
///
/// Computes `out[i] = a[i] * b[i] mod p` for all `i < a.len()`.
/// Inputs and outputs are canonical bytes in `[0, p)`.
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical bytes (same length).
/// * `p` — odd prime in `[3, 251]`.
/// * `out` — output slice (same length as `a` and `b`).
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
/// be canonical (`< p`). The kernel accumulates into 32-bit lanes in
/// AVX2 and reduces modulo `p` once at the panel boundary, so the
/// per-element cost is dominated by the `_mm256_madd_epi16` lane-pair
/// MAC.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
pub type SmallPrimeBatchDotFn = fn(&[u8], &[u8], u8) -> u8;

/// Whole-row gemm panel for `Fp<P>` with `P <= 251`.
///
/// Computes `out[j] = (∑_t a[t] * bt[j*k + t]) mod p` for `j ∈ [0, n)`,
/// where `a` is one length-`k` row and `bt` is the `n × k` row-major
/// transpose of the right operand. The kernel reuses each loaded
/// 16-byte block of `a` against four B-transpose rows simultaneously,
/// amortising the AVX2 lane-broadcast and constant-table overhead
/// across four output cells per inner pass.
///
/// # Arguments
///
/// * `a` — left-row input slice of length `k`.
/// * `bt` — row-major B-transpose of length `n * k`.
/// * `k`, `n` — panel inner and outer dims.
/// * `p` — odd prime in `[3, 251]`.
/// * `out` — destination slice of length `n`.
///
/// # Panics
///
/// Panics if any slice length disagrees with `k`, `n`.
pub type SmallPrimeGemmRowPanelFn = fn(&[u8], &[u8], usize, usize, u8, &mut [u8]);

/// Sparse-times-dense row kernel for `Fp<P>` with `P <= 251`.
///
/// Writes `out[j] = (∑_h a_vals[h] * b[a_cols[h] * b_stride + j]) mod p`
/// for `j ∈ [0, n)`. The sparse left row is given as `(a_vals, a_cols)`
/// and `b` is a row-major dense byte matrix with row stride `b_stride`.
///
/// # Arguments
///
/// * `a_vals` — non-zero values of the sparse row, canonical bytes
///   `< p`. `a_vals.len() == a_cols.len()`.
/// * `a_cols` — column indices into `b` for each non-zero. Each must
///   satisfy `a_cols[h] * b_stride + n <= b.len()`.
/// * `b` — dense `b_rows × b_stride` byte matrix in row-major layout.
/// * `b_stride` — physical stride between consecutive rows of `b`.
/// * `n` — output column count (must be ≤ `b_stride`).
/// * `p` — odd prime in `[3, 251]`.
/// * `out` — destination slice of length `n`.
///
/// # Panics
///
/// Panics if `a_vals.len() != a_cols.len()` or `out.len() != n`.
pub type SmallPrimeSpmmRowFn = fn(&[u8], &[usize], &[u8], usize, usize, u8, &mut [u8]);

/// Bundle of small-prime SIMD batch operations.
///
/// Populated at runtime by [`detect`] when AVX2 is available. All
/// entries are plain function pointers (not trait objects) so they
/// remain usable under a `#![deny(unsafe_code)]` regime in callers.
///
/// The function pointers take the prime `p` as a runtime argument so
/// one dispatch struct covers GF(7), GF(31), GF(251), and any other
/// `P ≤ 251` consumers. The Barrett constant for the chosen prime is
/// loaded from a 256-entry table at the start of every kernel call.
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
}

/// Detect and return the best available small-prime SIMD function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// AVX2.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_small;
///
/// if let Some(fns) = fp_small::detect() {
///     let a = [1u8, 2, 3, 4];
///     let b = [5u8, 6, 0, 1];
///     let mut out = [0u8; 4];
///     (fns.batch_mul_fn)(&a, &b, 7, &mut out);
///     // out[i] = a[i] * b[i] mod 7
///     assert_eq!(out, [5, 5, 0, 4]);
/// }
/// ```
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
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_mul_safe(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_mul(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_add_safe(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_add(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_sub_safe(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_sub(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_dot_safe(a: &[u8], b: &[u8], p: u8) -> u8 {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_batch_dot(a, b, p) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn gemm_row_panel_safe(a: &[u8], bt: &[u8], k: usize, n: usize, p: u8, out: &mut [u8]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
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
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_small::fp_small_spmm_row(a_vals, a_cols, b, b_stride, n, p, out) }
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
}
