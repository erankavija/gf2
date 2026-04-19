//! SIMD batch kernels for `Fp<65537>` arithmetic.
//!
//! `P = 65537 = 2^16 + 1` is the fifth Fermat prime. Its algebraic shape
//! (`2^16 ≡ -1 (mod P)`) makes the modular reduction exceptionally tight
//! on SIMD: a packed 33-bit product splits on the 16-bit boundary, one
//! subtract folds the halves, and a single conditional subtract
//! canonicalises. The result is a per-element cost dominated by the
//! multiply itself — and because AVX2 processes eight u32 lanes per
//! 256-bit vector, the amortised cost beats a sequential Montgomery mul
//! by a large factor on Zen 3 and newer.
//!
//! All unsafe intrinsics are isolated in `x86/fp65537.rs`; this module
//! exposes only safe function-pointer wrappers through the
//! [`Fp65537Fns`] table returned by [`detect`]. Callers without AVX2
//! receive `None` and must fall back to scalar loops.

/// Lane-wise batch multiply for `Fp<65537>`.
///
/// Computes `out[i] = a[i] * b[i] mod 65537` for all `i < a.len()`.
/// Input values must already be canonical (`< 65537`).
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical `Fp<65537>` values (same length).
/// * `out` — output slice (same length).
///
/// # Panics
///
/// Panics if the slice lengths differ.
pub type Fp65537BatchMulFn = fn(&[u32], &[u32], &mut [u32]);

/// Lane-wise batch addition for `Fp<65537>`.
///
/// Computes `out[i] = (a[i] + b[i]) mod 65537`.
///
/// # Panics
///
/// Panics if the slice lengths differ.
pub type Fp65537BatchAddFn = fn(&[u32], &[u32], &mut [u32]);

/// Lane-wise batch subtraction for `Fp<65537>`.
///
/// Computes `out[i] = (a[i] - b[i]) mod 65537` with the result in
/// canonical form `[0, 65537)`.
///
/// # Panics
///
/// Panics if the slice lengths differ.
pub type Fp65537BatchSubFn = fn(&[u32], &[u32], &mut [u32]);

/// Fused Karatsuba combine for `GF(p²)` element-wise multiplication over
/// `Fp<65537>`.
///
/// Computes, for every `i`:
///
/// ```text
/// v0_i    = a0[i] * b0[i]
/// v1_i    = a1[i] * b1[i]
/// cross_i = (a0[i] + a1[i]) * (b0[i] + b1[i])
/// out_c0[i] = v0_i + beta · v1_i
/// out_c1[i] = cross_i - v0_i - v1_i
/// ```
///
/// with all intermediates kept in AVX2 registers, eliminating the
/// intermediate heap buffers that a `batch_mul_fn` + `batch_add_fn`
/// composition would otherwise need.
///
/// # Panics
///
/// Panics if any input or output slice has a different length from `a0`.
pub type Fp65537BatchKaratsubaFn = fn(&[u32], &[u32], &[u32], &[u32], u32, &mut [u32], &mut [u32]);

/// Bundle of `Fp<65537>` SIMD batch operations.
///
/// Populated at runtime by [`detect`] when AVX2 is available. All entries
/// are plain function pointers (not trait objects) so they remain usable
/// under a `#![deny(unsafe_code)]` regime in callers.
///
/// # Examples
///
/// ```
/// # use gf2_kernels_simd::fp65537;
/// if let Some(fns) = fp65537::detect() {
///     let a: Vec<u32> = (0..16u32).collect();
///     let b: Vec<u32> = (0..16u32).map(|i| i + 1).collect();
///     let mut out = vec![0u32; 16];
///     (fns.batch_mul_fn)(&a, &b, &mut out);
/// }
/// ```
#[derive(Copy, Clone)]
pub struct Fp65537Fns {
    /// Lane-wise batch multiply for `Fp<65537>`.
    pub batch_mul_fn: Fp65537BatchMulFn,
    /// Lane-wise batch addition for `Fp<65537>`.
    pub batch_add_fn: Fp65537BatchAddFn,
    /// Lane-wise batch subtraction for `Fp<65537>`.
    pub batch_sub_fn: Fp65537BatchSubFn,
    /// Fused Karatsuba combine for `GF(p²) / Fp<65537>`. This is the
    /// hot-loop entry point used by
    /// [`gf2_core::gfpn::BatchExtField::batch_mul_quadratic`] — keeping
    /// all intermediates in AVX2 registers avoids the nine heap
    /// allocations that a pass-per-op composition would otherwise need,
    /// and is the main source of the measured throughput win.
    pub batch_karatsuba_fn: Fp65537BatchKaratsubaFn,
}

/// Detect and return the best available `Fp<65537>` SIMD function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// AVX2. Callers must then fall back to scalar arithmetic.
///
/// # Examples
///
/// ```
/// # use gf2_kernels_simd::fp65537;
/// let maybe_fns = fp65537::detect();
/// // `maybe_fns.is_some()` on any AVX2-capable x86_64 host.
/// # let _ = maybe_fns;
/// ```
pub fn detect() -> Option<Fp65537Fns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<Fp65537Fns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(Fp65537Fns {
            batch_mul_fn: batch_mul_safe,
            batch_add_fn: batch_add_safe,
            batch_sub_fn: batch_sub_safe,
            batch_karatsuba_fn: batch_karatsuba_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_mul_safe(a: &[u32], b: &[u32], out: &mut [u32]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp65537::fp65537_batch_mul(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_add_safe(a: &[u32], b: &[u32], out: &mut [u32]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp65537::fp65537_batch_add(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_sub_safe(a: &[u32], b: &[u32], out: &mut [u32]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp65537::fp65537_batch_sub(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_karatsuba_safe(
    a0: &[u32],
    a1: &[u32],
    b0: &[u32],
    b1: &[u32],
    beta: u32,
    out_c0: &mut [u32],
    out_c1: &mut [u32],
) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp65537::fp65537_batch_karatsuba(a0, a1, b0, b1, beta, out_c0, out_c1) }
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
        let a: Vec<u32> = (0..50u32).map(|i| (i * 12345) % 65537).collect();
        let b: Vec<u32> = (0..50u32).map(|i| (i * 67890 + 7) % 65537).collect();
        let mut out = vec![0u32; 50];
        (fns.batch_mul_fn)(&a, &b, &mut out);
        for i in 0..50 {
            let expected = ((a[i] as u64 * b[i] as u64) % 65537u64) as u32;
            assert_eq!(out[i], expected, "i={i}");
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_add() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        let a: Vec<u32> = (0..50u32).map(|i| (i * 12345) % 65537).collect();
        let b: Vec<u32> = (0..50u32).map(|i| (i * 67890 + 7) % 65537).collect();
        let mut out = vec![0u32; 50];
        (fns.batch_add_fn)(&a, &b, &mut out);
        for i in 0..50 {
            let expected = (a[i] + b[i]) % 65537;
            assert_eq!(out[i], expected, "i={i}");
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_sub() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        let a: Vec<u32> = (0..50u32).map(|i| (i * 12345) % 65537).collect();
        let b: Vec<u32> = (0..50u32).map(|i| (i * 67890 + 7) % 65537).collect();
        let mut out = vec![0u32; 50];
        (fns.batch_sub_fn)(&a, &b, &mut out);
        for i in 0..50 {
            let expected = (a[i] + 65537 - b[i]) % 65537;
            assert_eq!(out[i], expected, "i={i}");
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_karatsuba() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &n in &[0usize, 1, 7, 8, 9, 16, 17, 100, 1000] {
            let a0: Vec<u32> = (0..n as u32).map(|i| (i * 17) % 65537).collect();
            let a1: Vec<u32> = (0..n as u32).map(|i| (i * 23 + 5) % 65537).collect();
            let b0: Vec<u32> = (0..n as u32).map(|i| (i * 29 + 7) % 65537).collect();
            let b1: Vec<u32> = (0..n as u32).map(|i| (i * 31 + 11) % 65537).collect();
            let beta = 3u32;

            let mut out_c0 = vec![0u32; n];
            let mut out_c1 = vec![0u32; n];
            (fns.batch_karatsuba_fn)(&a0, &a1, &b0, &b1, beta, &mut out_c0, &mut out_c1);

            for i in 0..n {
                let v0 = ((a0[i] as u64 * b0[i] as u64) % 65537) as u32;
                let v1 = ((a1[i] as u64 * b1[i] as u64) % 65537) as u32;
                let sum_a = (a0[i] + a1[i]) % 65537;
                let sum_b = (b0[i] + b1[i]) % 65537;
                let cross = ((sum_a as u64 * sum_b as u64) % 65537) as u32;
                let beta_v1 = ((beta as u64 * v1 as u64) % 65537) as u32;
                let expected_c0 = (v0 + beta_v1) % 65537;
                let expected_c1 = ((cross + 65537 - v0) % 65537 + 65537 - v1) % 65537;
                assert_eq!(out_c0[i], expected_c0, "c0 mismatch n={n} i={i}");
                assert_eq!(out_c1[i], expected_c1, "c1 mismatch n={n} i={i}");
            }
        }
    }

    #[test]
    fn batch_mul_lengths_cover_tail() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &len in &[0usize, 1, 7, 8, 9, 15, 16, 17, 256, 1024] {
            let a: Vec<u32> = (0..len as u32).map(|i| (i * 17) % 65537).collect();
            let b: Vec<u32> = (0..len as u32).map(|i| (i * 23 + 5) % 65537).collect();
            let mut out = vec![0u32; len];
            (fns.batch_mul_fn)(&a, &b, &mut out);
            for i in 0..len {
                let expected = ((a[i] as u64 * b[i] as u64) % 65537) as u32;
                assert_eq!(out[i], expected, "len={len} i={i}");
            }
        }
    }
}
