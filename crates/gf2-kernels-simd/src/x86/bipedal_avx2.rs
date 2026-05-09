//! AVX2 batch entry points for the F_3 instantiation
//! [`crate::bipedal::Bipedal3x4`] of the generic
//! [`crate::bipedal::framework::BatchedBipedalLike`] framework.
//!
//! The functions in this file carry `#[target_feature(enable = "avx2")]`,
//! which is the static contract required by rustc to fully inline the
//! trait-method-emitted AVX2 intrinsics. R4 §4.1 documents the 12-34x
//! regression that occurs without this discipline.
//!
//! All `pub unsafe fn` here carry a top-of-function `// SAFETY:` comment.
//! AVX2 availability is the dynamic precondition every caller must
//! runtime-detect via `is_x86_feature_detected!("avx2")` before invoking.
//!
//! These are the only files in the bipedal stack that actually emit AVX2
//! instructions; everything in `crate::bipedal::*` is plumbing that
//! inlines into them.

use crate::bipedal::framework::BatchedBipedalLike;
use crate::bipedal::lanes::{Avx2Lane, BipedalLogicalLanes};
use crate::bipedal::Config3;

/// Apply F_3 add over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// One AVX2 lane consumes 4 × `u64` (256 bits = 256 logical F_3 lanes).
/// All six slices must be the same length and a multiple of 4. An empty
/// input (length 0) is allowed and is a no-op.
///
/// # Arguments
///
/// * `mag1`, `sgn1` — first operand `(mag, sgn)` streams.
/// * `mag2`, `sgn2` — second operand `(mag, sgn)` streams.
/// * `out_mag`, `out_sgn` — output buffers.
///
/// # Safety
///
/// AVX2 must be available at runtime (verify via
/// `is_x86_feature_detected!("avx2")`). All six slices share length
/// divisible by 4; behaviour is undefined otherwise.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_add_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4 (= one AVX2 lane).
///     unsafe { run_add_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops, where `n = mag1.len()`.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_add_batch(
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    debug_assert_eq!(mag1.len() % 4, 0);
    debug_assert_eq!(mag1.len(), sgn1.len());
    debug_assert_eq!(mag1.len(), mag2.len());
    debug_assert_eq!(mag1.len(), sgn2.len());
    debug_assert_eq!(mag1.len(), out_mag.len());
    debug_assert_eq!(mag1.len(), out_sgn.len());
    let n = mag1.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m1 = Avx2Lane::loadu(mag1, i);
            let v_s1 = Avx2Lane::loadu(sgn1, i);
            let v_m2 = Avx2Lane::loadu(mag2, i);
            let v_s2 = Avx2Lane::loadu(sgn2, i);
            let (m, s) =
                BatchedBipedalLike::<Config3, Avx2Lane, Avx2Lane>::add(v_m1, v_s1, v_m2, v_s2);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}

/// Apply F_3 sub over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// See [`run_add_batch`] for the slice-shape contract.
///
/// # Arguments
///
/// Same shape as [`run_add_batch`]; `(mag1, sgn1) - (mag2, sgn2)`.
///
/// # Safety
///
/// AVX2 must be available at runtime. All six slices share length
/// divisible by 4.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_sub_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4.
///     unsafe { run_sub_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_sub_batch(
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    debug_assert_eq!(mag1.len() % 4, 0);
    debug_assert_eq!(mag1.len(), sgn1.len());
    debug_assert_eq!(mag1.len(), mag2.len());
    debug_assert_eq!(mag1.len(), sgn2.len());
    debug_assert_eq!(mag1.len(), out_mag.len());
    debug_assert_eq!(mag1.len(), out_sgn.len());
    let n = mag1.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m1 = Avx2Lane::loadu(mag1, i);
            let v_s1 = Avx2Lane::loadu(sgn1, i);
            let v_m2 = Avx2Lane::loadu(mag2, i);
            let v_s2 = Avx2Lane::loadu(sgn2, i);
            let (m, s) =
                BatchedBipedalLike::<Config3, Avx2Lane, Avx2Lane>::sub(v_m1, v_s1, v_m2, v_s2);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}

/// Apply F_3 mul over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// See [`run_add_batch`] for the slice-shape contract.
///
/// # Arguments
///
/// Same shape as [`run_add_batch`]; `(mag1, sgn1) * (mag2, sgn2)`.
///
/// # Safety
///
/// AVX2 must be available at runtime. All six slices share length
/// divisible by 4.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_mul_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4.
///     unsafe { run_mul_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_mul_batch(
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    debug_assert_eq!(mag1.len() % 4, 0);
    debug_assert_eq!(mag1.len(), sgn1.len());
    debug_assert_eq!(mag1.len(), mag2.len());
    debug_assert_eq!(mag1.len(), sgn2.len());
    debug_assert_eq!(mag1.len(), out_mag.len());
    debug_assert_eq!(mag1.len(), out_sgn.len());
    let n = mag1.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m1 = Avx2Lane::loadu(mag1, i);
            let v_s1 = Avx2Lane::loadu(sgn1, i);
            let v_m2 = Avx2Lane::loadu(mag2, i);
            let v_s2 = Avx2Lane::loadu(sgn2, i);
            let (m, s) =
                BatchedBipedalLike::<Config3, Avx2Lane, Avx2Lane>::mul(v_m1, v_s1, v_m2, v_s2);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}

/// Apply F_3 neg over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// Two input slices and two output slices, all the same length and a
/// multiple of 4.
///
/// # Arguments
///
/// * `mag`, `sgn` — input `(mag, sgn)` streams.
/// * `out_mag`, `out_sgn` — output buffers.
///
/// # Safety
///
/// AVX2 must be available at runtime. All four slices share length
/// divisible by 4.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_neg_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4.
///     unsafe { run_neg_batch(&v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_neg_batch(mag: &[u64], sgn: &[u64], out_mag: &mut [u64], out_sgn: &mut [u64]) {
    debug_assert_eq!(mag.len() % 4, 0);
    debug_assert_eq!(mag.len(), sgn.len());
    debug_assert_eq!(mag.len(), out_mag.len());
    debug_assert_eq!(mag.len(), out_sgn.len());
    let n = mag.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m = Avx2Lane::loadu(mag, i);
            let v_s = Avx2Lane::loadu(sgn, i);
            let (m, s) = BatchedBipedalLike::<Config3, Avx2Lane, Avx2Lane>::neg(v_m, v_s);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}
