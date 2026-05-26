//! SIMD batch kernels for medium primes `Fp<P>` with `P < 2^16`.
//!
//! This module covers the `word-fits-in-u16` family of prime fields,
//! whose canonical residues fit in a single 16-bit lane. The reference
//! prime is `P = 65521` (the largest prime below `2^16`); the kernel
//! accepts any odd prime in `(251, 65535]` — the upper boundary of
//! `Fp<P>` primes that the small-prime kernel (issue `662f7a15`) does
//! not already cover.
//!
//! The unsafe AVX2 implementation lives in [`crate::x86::fp_medium`];
//! this module exposes only safe function-pointer wrappers through the
//! [`MediumPrimeFns`] table returned by [`detect`]. Callers without AVX2
//! receive `None` and must fall back to scalar loops.

/// Computes the Barrett magic constant `m = floor(2^32 / p)` for a
/// medium prime `p ∈ (1, 2^16)`.
///
/// This `const fn` lets callers compute the constant at compile time
/// for use with [`MediumPrimeBatchMulFn`] without depending on the
/// architecture-specific module.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_medium::barrett_m32;
///
/// // m = floor(2^32 / 65521) = 65551.
/// assert_eq!(barrett_m32(65521), 65551);
/// ```
#[inline]
pub const fn barrett_m32(p: u16) -> u32 {
    ((1u64 << 32) / p as u64) as u32
}

/// Lane-wise batch multiply for medium-prime `Fp<P>`.
///
/// Computes `out[i] = (a[i] * b[i]) mod p` for all `i`. Inputs must be
/// canonical (`< p`); the Barrett magic constant `barrett_m =
/// floor(2^32 / p)` must be supplied by the caller (typically computed
/// at compile time via [`barrett_m32`]).
pub type MediumPrimeBatchMulFn = fn(&[u16], &[u16], u16, u32, &mut [u16]);

/// Lane-wise batch addition for medium-prime `Fp<P>`.
pub type MediumPrimeBatchAddFn = fn(&[u16], &[u16], u16, &mut [u16]);

/// Lane-wise batch subtraction for medium-prime `Fp<P>`.
pub type MediumPrimeBatchSubFn = fn(&[u16], &[u16], u16, &mut [u16]);

/// Batch dot product for medium-prime `Fp<P>`, returning the canonical
/// reduced sum.
pub type MediumPrimeBatchDotFn = fn(&[u16], &[u16], u16) -> u32;

/// Sparse-times-dense row kernel for medium-prime `Fp<P>` with
/// `P ∈ (251, 65535]`.
///
/// Writes `out[j] = (∑_h a_vals[h] * b[a_cols[h] * b_stride + j]) mod p`
/// for `j ∈ [0, n)`. The sparse left row is given as `(a_vals,
/// a_cols)` with canonical u16 lanes; `b` is a row-major dense u16
/// matrix with row stride `b_stride`. `out` is the dense output row of
/// length `n`.
pub type MediumPrimeSpmmRowFn = fn(&[u16], &[usize], &[u16], usize, usize, u16, &mut [u16]);

/// Whole-GEMM panel kernel for medium-prime `Fp<P>` with `P ∈ (251,
/// 65535]`. Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p`
/// for every `(i, j) ∈ [0, m) × [0, n)`. Inputs are canonical u16
/// residues. The transpose `bt` is `n × k` row-major.
///
/// Closes the per-cell `MediumPrimeBatchDotFn` dispatch overhead at
/// large `n` (issue `74ba1cdc`): pre-packs B once per gemm into
/// `NR = 16` u16-wide N-major panels, then sweeps each `MR = 2` row
/// block of A against every panel with 8 u64-lane accumulators
/// resident across the full k axis.
pub type MediumPrimeGemmPanelFn = fn(&[u16], &[u16], usize, usize, usize, u16, &mut [u16]);

/// Bundle of AVX2 batch operations for medium-prime `Fp<P>`.
///
/// Populated at runtime by [`detect`] when AVX2 is available. All
/// entries are plain function pointers, usable from `#![deny(unsafe_code)]`
/// callers.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_medium;
///
/// if let Some(fns) = fp_medium::detect() {
///     let p = 65521u16;
///     let m = fp_medium::barrett_m32(p);
///     let a: Vec<u16> = (0..16u16).collect();
///     let b: Vec<u16> = (0..16u16).map(|i| i + 1).collect();
///     let mut out = vec![0u16; 16];
///     (fns.batch_mul_fn)(&a, &b, p, m, &mut out);
/// }
/// ```
#[derive(Copy, Clone)]
pub struct MediumPrimeFns {
    /// Lane-wise batch multiply.
    pub batch_mul_fn: MediumPrimeBatchMulFn,
    /// Lane-wise batch addition.
    pub batch_add_fn: MediumPrimeBatchAddFn,
    /// Lane-wise batch subtraction.
    pub batch_sub_fn: MediumPrimeBatchSubFn,
    /// Batch dot product reduced to a canonical scalar.
    pub batch_dot_fn: MediumPrimeBatchDotFn,
    /// Sparse-times-dense row kernel.
    pub spmm_row_fn: MediumPrimeSpmmRowFn,
    /// Whole-GEMM panel kernel (`jit:74ba1cdc` — replaces per-cell
    /// `batch_dot_fn` dispatch in the GEMM caller).
    pub gemm_panel_fn: MediumPrimeGemmPanelFn,
}

/// Detect and return the best available medium-prime SIMD bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// AVX2. Callers must then fall back to scalar arithmetic.
pub fn detect() -> Option<MediumPrimeFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<MediumPrimeFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(MediumPrimeFns {
            batch_mul_fn: batch_mul_safe,
            batch_add_fn: batch_add_safe,
            batch_sub_fn: batch_sub_safe,
            batch_dot_fn: batch_dot_safe,
            spmm_row_fn: spmm_row_safe,
            gemm_panel_fn: gemm_panel_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_mul_safe(a: &[u16], b: &[u16], p: u16, barrett_m: u32, out: &mut [u16]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_medium::fp_medium_batch_mul(a, b, p, barrett_m, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_add_safe(a: &[u16], b: &[u16], p: u16, out: &mut [u16]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_medium::fp_medium_batch_add(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_sub_safe(a: &[u16], b: &[u16], p: u16, out: &mut [u16]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_medium::fp_medium_batch_sub(a, b, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_dot_safe(a: &[u16], b: &[u16], p: u16) -> u32 {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_medium::fp_medium_batch_dot(a, b, p) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn spmm_row_safe(
    a_vals: &[u16],
    a_cols: &[usize],
    b: &[u16],
    b_stride: usize,
    n: usize,
    p: u16,
    out: &mut [u16],
) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_medium::fp_medium_spmm_row(a_vals, a_cols, b, b_stride, n, p, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn gemm_panel_safe(
    a: &[u16],
    bt: &[u16],
    m: usize,
    k: usize,
    n: usize,
    p: u16,
    c: &mut [u16],
) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_medium::fp_medium_gemm_panel(a, bt, m, k, n, p, c) }
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

    #[test]
    fn safe_wrapper_matches_scalar_batch_mul() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        let p = 65521u16;
        let m = barrett_m32(p);
        let a: Vec<u16> = (0..200)
            .map(|i| ((i as u32 * 12345) % p as u32) as u16)
            .collect();
        let b: Vec<u16> = (0..200)
            .map(|i| ((i as u32 * 67890 + 7) % p as u32) as u16)
            .collect();
        let mut out = vec![0u16; 200];
        (fns.batch_mul_fn)(&a, &b, p, m, &mut out);
        for i in 0..200 {
            let expected = ((a[i] as u32 * b[i] as u32) % p as u32) as u16;
            assert_eq!(out[i], expected, "i={i}");
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_dot() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        let p = 65521u16;
        for &len in &[0usize, 1, 7, 15, 16, 17, 100, 1024] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 17) % p as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 23 + 5) % p as u32) as u16)
                .collect();
            let got = (fns.batch_dot_fn)(&a, &b, p);
            let mut expected: u64 = 0;
            for i in 0..len {
                expected += (a[i] as u64) * (b[i] as u64);
            }
            assert_eq!(got as u64, expected % p as u64, "len={len}");
        }
    }
}
