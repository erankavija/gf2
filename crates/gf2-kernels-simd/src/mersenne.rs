//! SIMD batch kernels for Mersenne-prime arithmetic.
//!
//! Currently targets the `M31 = 2^31 - 1` prime field, with AVX2 kernels
//! that process 8 lanes of packed `u32` per 256-bit vector. The reduction
//! identity `2^31 ≡ 1 (mod M31)` reduces each product with a single fold
//! plus a branchless canonicalisation.
//!
//! All unsafe intrinsics are isolated in `x86/mersenne.rs`; this module
//! only exposes safe function-pointer wrappers through the [`MersenneFns`]
//! table returned by [`detect`]. Callers without AVX2 receive `None` and
//! should fall back to scalar loops.

/// Lane-wise batch multiply for `Fp<2^31 - 1>`.
///
/// Computes `out[i] = a[i] * b[i] mod (2^31 - 1)` for all `i < a.len()`.
/// Input values must already be canonical (`< 2^31 - 1`).
///
/// # Arguments
/// * `a`, `b` — input slices of canonical M31 values (same length)
/// * `out` — output slice (same length)
///
/// # Panics
/// Panics if the slices have different lengths.
pub type M31BatchMulFn = fn(&[u32], &[u32], &mut [u32]);

/// Lane-wise batch multiply-and-accumulate for `Fp<2^31 - 1>`.
///
/// Computes `acc[i] = (acc[i] + a[i] * b[i]) mod (2^31 - 1)`.
/// Inputs and accumulator must be canonical.
///
/// # Panics
/// Panics if the slices have different lengths.
pub type M31BatchMulAddFn = fn(&[u32], &[u32], &mut [u32]);

/// Batch dot product over `Fp<2^31 - 1>`.
///
/// Returns `sum_i (a[i] * b[i]) mod (2^31 - 1)` as a canonical `u32`.
/// Inputs must be canonical.
///
/// # Panics
/// Panics if `a.len() != b.len()`.
pub type M31BatchDotFn = fn(&[u32], &[u32]) -> u32;

/// Bundle of Mersenne-prime SIMD batch operations.
///
/// Populated at runtime by [`detect`] when AVX2 is available. All entries
/// are plain function pointers (not trait objects) so they remain usable
/// under a `#![deny(unsafe_code)]` regime in callers.
#[derive(Copy, Clone)]
pub struct MersenneFns {
    /// Lane-wise batch multiply for `M31 = 2^31 - 1`.
    pub m31_batch_mul_fn: M31BatchMulFn,
    /// Lane-wise batch multiply-and-accumulate for `M31`.
    pub m31_batch_mul_add_fn: M31BatchMulAddFn,
    /// Batch dot product reduced to scalar for `M31`.
    pub m31_batch_dot_fn: M31BatchDotFn,
}

/// Detect and return the best available Mersenne SIMD function bundle.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks AVX2.
pub fn detect() -> Option<MersenneFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<MersenneFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(MersenneFns {
            m31_batch_mul_fn: m31_batch_mul_safe,
            m31_batch_mul_add_fn: m31_batch_mul_add_safe,
            m31_batch_dot_fn: m31_batch_dot_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn m31_batch_mul_safe(a: &[u32], b: &[u32], out: &mut [u32]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::mersenne::mersenne31_batch_mul(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn m31_batch_mul_add_safe(a: &[u32], b: &[u32], acc: &mut [u32]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::mersenne::mersenne31_batch_mul_add(a, b, acc) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn m31_batch_dot_safe(a: &[u32], b: &[u32]) -> u32 {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::mersenne::mersenne31_batch_dot(a, b) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P31: u32 = (1u32 << 31) - 1;

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
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_mul() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        let a: Vec<u32> = (0..50u32).map(|i| (i * 12345) % P31).collect();
        let b: Vec<u32> = (0..50u32).map(|i| (i * 67890 + 7) % P31).collect();
        let mut out = vec![0u32; 50];
        (fns.m31_batch_mul_fn)(&a, &b, &mut out);
        for i in 0..50 {
            let expected = ((a[i] as u64 * b[i] as u64) % P31 as u64) as u32;
            assert_eq!(out[i], expected, "i={i}");
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_batch_dot() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &len in &[0usize, 1, 7, 8, 100, 1024] {
            let a: Vec<u32> = (0..len as u32).map(|i| (i * 17) % P31).collect();
            let b: Vec<u32> = (0..len as u32).map(|i| (i * 23 + 5) % P31).collect();
            let got = (fns.m31_batch_dot_fn)(&a, &b);
            let mut expected: u64 = 0;
            for i in 0..len {
                expected = (expected + (a[i] as u64 * b[i] as u64) % P31 as u64) % P31 as u64;
            }
            assert_eq!(got as u64, expected, "len={len}");
        }
    }
}
