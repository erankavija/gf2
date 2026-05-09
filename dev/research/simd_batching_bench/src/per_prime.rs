//! Per-prime hand-rolled bipedal F_3 AVX2 kernel.
//!
//! Each function operates on `__m256i` pairs `(mag, sgn)` representing 256
//! lanes of F_3 (4 u64 lanes × 64 bit positions). The arithmetic uses the
//! Scheinerman 2024 §2.2 formulas verbatim, expressed directly in AVX2
//! intrinsics with no abstraction layer between the formula and the silicon.
//!
//! All `pub unsafe fn` here carry a top-of-function `// SAFETY:` comment
//! per the amended CLAUDE.md `dev/research/<crate>/` exemption.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Bipedal F_3 add on a 256-bit lane (paper §2.2, 6 ops with CSE).
///
/// `t = m1 ^ s1 ^ s2; u = m2 & t; m_+ = u | (m1 ^ m2); s_+ = u ^ s1`.
///
/// # Arguments
///
/// * `m1`, `s1` — first operand `(mag, sgn)`.
/// * `m2`, `s2` — second operand `(mag, sgn)`.
///
/// Returns `(m_plus, s_plus)`, the lane-wise sum.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime (the `#[target_feature]`
/// attribute is the static contract; runtime detection is the dynamic one).
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::per_prime::bipedal3_avx2_add;
/// // Caller must runtime-check AVX2 first.
/// if is_x86_feature_detected!("avx2") {
///     // SAFETY: AVX2 just verified above.
///     unsafe {
///         use core::arch::x86_64::_mm256_set1_epi64x;
///         let z = _mm256_set1_epi64x(0);
///         let _ = bipedal3_avx2_add(z, z, z, z); // 0 + 0 = 0
///     }
/// }
/// ```
///
/// # Complexity
///
/// `O(1)`: six AVX2 logical ops per 256-lane batch.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn bipedal3_avx2_add(
    m1: __m256i,
    s1: __m256i,
    m2: __m256i,
    s2: __m256i,
) -> (__m256i, __m256i) {
    // SAFETY: AVX2 availability is the caller's precondition; the
    // intrinsics themselves only require typed __m256i operands.
    unsafe {
        let t = _mm256_xor_si256(_mm256_xor_si256(m1, s1), s2);
        let u = _mm256_and_si256(m2, t);
        let m_plus = _mm256_or_si256(u, _mm256_xor_si256(m1, m2));
        let s_plus = _mm256_xor_si256(u, s1);
        (m_plus, s_plus)
    }
}

/// Bipedal F_3 sub on a 256-bit lane (paper §2.2, 6 ops with CSE).
///
/// `t = s1 ^ s2; u = m1 & t; m_- = u | (m1 ^ m2); s_- = u ^ (m2 ^ s2)`.
///
/// # Arguments
///
/// * `m1`, `s1` — first operand `(mag, sgn)`.
/// * `m2`, `s2` — second operand `(mag, sgn)`.
///
/// Returns `(m_minus, s_minus)`, the lane-wise difference `(self - rhs)`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime.
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::per_prime::bipedal3_avx2_sub;
/// if is_x86_feature_detected!("avx2") {
///     // SAFETY: AVX2 just verified above.
///     unsafe {
///         use core::arch::x86_64::_mm256_set1_epi64x;
///         let z = _mm256_set1_epi64x(0);
///         let _ = bipedal3_avx2_sub(z, z, z, z); // 0 - 0 = 0
///     }
/// }
/// ```
///
/// # Complexity
///
/// `O(1)`: six AVX2 logical ops per 256-lane batch.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn bipedal3_avx2_sub(
    m1: __m256i,
    s1: __m256i,
    m2: __m256i,
    s2: __m256i,
) -> (__m256i, __m256i) {
    // SAFETY: AVX2 availability is the caller's precondition.
    unsafe {
        let t = _mm256_xor_si256(s1, s2);
        let u = _mm256_and_si256(m1, t);
        let m_minus = _mm256_or_si256(u, _mm256_xor_si256(m1, m2));
        let s_minus = _mm256_xor_si256(u, _mm256_xor_si256(m2, s2));
        (m_minus, s_minus)
    }
}

/// Bipedal F_3 mul on a 256-bit lane (paper §2.2, 2 ops).
///
/// `m_x = m1 & m2; s_x = s1 ^ s2`.
///
/// # Arguments
///
/// * `m1`, `s1` — first operand `(mag, sgn)`.
/// * `m2`, `s2` — second operand `(mag, sgn)`.
///
/// Returns `(m_x, s_x)`, the lane-wise product.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime.
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::per_prime::bipedal3_avx2_mul;
/// if is_x86_feature_detected!("avx2") {
///     // SAFETY: AVX2 just verified above.
///     unsafe {
///         use core::arch::x86_64::_mm256_set1_epi64x;
///         let z = _mm256_set1_epi64x(0);
///         let _ = bipedal3_avx2_mul(z, z, z, z); // 0 * 0 = 0
///     }
/// }
/// ```
///
/// # Complexity
///
/// `O(1)`: two AVX2 logical ops per 256-lane batch.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn bipedal3_avx2_mul(
    m1: __m256i,
    s1: __m256i,
    m2: __m256i,
    s2: __m256i,
) -> (__m256i, __m256i) {
    // SAFETY: AVX2 availability is the caller's precondition.
    unsafe {
        let m_x = _mm256_and_si256(m1, m2);
        let s_x = _mm256_xor_si256(s1, s2);
        (m_x, s_x)
    }
}

/// Apply `bipedal3_avx2_add` to two slices of `(mag, sgn)` u64 words.
///
/// Slices must be the same length and a multiple of 4 (one AVX2 lane = 4 u64).
///
/// # Arguments
///
/// * `mag1`, `sgn1` — first operand vectors, each `n` u64s.
/// * `mag2`, `sgn2` — second operand vectors, each `n` u64s.
/// * `out_mag`, `out_sgn` — output vectors, each `n` u64s.
///
/// # Safety
///
/// AVX2 must be available; all six slices share length `n` divisible by 4.
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::per_prime::run_add_batch;
/// if is_x86_feature_detected!("avx2") {
///     let mag1 = vec![0u64; 4];
///     let sgn1 = vec![0u64; 4];
///     let mag2 = vec![0u64; 4];
///     let sgn2 = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices are length 4 (one AVX2 lane).
///     unsafe {
///         run_add_batch(&mag1, &sgn1, &mag2, &sgn2, &mut out_m, &mut out_s);
///     }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)`: one bipedal3_avx2_add per AVX2 lane.
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
        let mut i = 0;
        while i < n {
            let v_m1 = _mm256_loadu_si256(mag1.as_ptr().add(i) as *const __m256i);
            let v_s1 = _mm256_loadu_si256(sgn1.as_ptr().add(i) as *const __m256i);
            let v_m2 = _mm256_loadu_si256(mag2.as_ptr().add(i) as *const __m256i);
            let v_s2 = _mm256_loadu_si256(sgn2.as_ptr().add(i) as *const __m256i);
            let (m, s) = bipedal3_avx2_add(v_m1, v_s1, v_m2, v_s2);
            _mm256_storeu_si256(out_mag.as_mut_ptr().add(i) as *mut __m256i, m);
            _mm256_storeu_si256(out_sgn.as_mut_ptr().add(i) as *mut __m256i, s);
            i += 4;
        }
    }
}

/// Apply `bipedal3_avx2_sub` to two slices of `(mag, sgn)` u64 words.
///
/// See [`run_add_batch`] for argument shape.
///
/// # Safety
///
/// AVX2 must be available; all six slices share length `n` divisible by 4.
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::per_prime::run_sub_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices are length 4.
///     unsafe { run_sub_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)`.
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
        let mut i = 0;
        while i < n {
            let v_m1 = _mm256_loadu_si256(mag1.as_ptr().add(i) as *const __m256i);
            let v_s1 = _mm256_loadu_si256(sgn1.as_ptr().add(i) as *const __m256i);
            let v_m2 = _mm256_loadu_si256(mag2.as_ptr().add(i) as *const __m256i);
            let v_s2 = _mm256_loadu_si256(sgn2.as_ptr().add(i) as *const __m256i);
            let (m, s) = bipedal3_avx2_sub(v_m1, v_s1, v_m2, v_s2);
            _mm256_storeu_si256(out_mag.as_mut_ptr().add(i) as *mut __m256i, m);
            _mm256_storeu_si256(out_sgn.as_mut_ptr().add(i) as *mut __m256i, s);
            i += 4;
        }
    }
}

/// Apply `bipedal3_avx2_mul` to two slices of `(mag, sgn)` u64 words.
///
/// See [`run_add_batch`] for argument shape.
///
/// # Safety
///
/// AVX2 must be available; all six slices share length `n` divisible by 4.
///
/// # Examples
///
/// ```no_run
/// use simd_batching_bench::per_prime::run_mul_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices are length 4.
///     unsafe { run_mul_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)`.
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
        let mut i = 0;
        while i < n {
            let v_m1 = _mm256_loadu_si256(mag1.as_ptr().add(i) as *const __m256i);
            let v_s1 = _mm256_loadu_si256(sgn1.as_ptr().add(i) as *const __m256i);
            let v_m2 = _mm256_loadu_si256(mag2.as_ptr().add(i) as *const __m256i);
            let v_s2 = _mm256_loadu_si256(sgn2.as_ptr().add(i) as *const __m256i);
            let (m, s) = bipedal3_avx2_mul(v_m1, v_s1, v_m2, v_s2);
            _mm256_storeu_si256(out_mag.as_mut_ptr().add(i) as *mut __m256i, m);
            _mm256_storeu_si256(out_sgn.as_mut_ptr().add(i) as *mut __m256i, s);
            i += 4;
        }
    }
}
