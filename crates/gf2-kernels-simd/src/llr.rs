//! SIMD-accelerated LLR (Log-Likelihood Ratio) operations for soft-decision decoding.
//!
//! Provides horizontal min/max operations over f32 slices, used in:
//! - LDPC belief propagation (min-sum approximation)
//! - Viterbi decoder (ACS operations)
//! - Turbo codes (MAP/BCJR algorithm)
//!
//! # LLR Sign Convention
//!
//! LLRs represent log(P(bit=0) / P(bit=1)):
//! - Positive LLR → likely bit=0
//! - Negative LLR → likely bit=1
//! - Magnitude = confidence

/// LLR operation function bundle for f32.
pub struct LlrFnsF32 {
    /// Compute sign-preserving horizontal minimum: sign_product * min(|inputs|)
    pub minsum_fn: fn(&[f32]) -> f32,
    /// Compute maximum of absolute values.
    pub maxabs_fn: fn(&[f32]) -> f32,
}

/// LLR operation function bundle for f64.
pub struct LlrFnsF64 {
    /// Compute sign-preserving horizontal minimum: sign_product * min(|inputs|)
    pub minsum_fn: fn(&[f64]) -> f64,
    /// Compute maximum of absolute values.
    pub maxabs_fn: fn(&[f64]) -> f64,
}

// Legacy type alias for backward compatibility
pub type LlrFns = LlrFnsF32;

/// Detect and return the best available f32 LLR function bundle.
pub fn detect_f32() -> Option<LlrFnsF32> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86_f32();
    }
    #[allow(unreachable_code)]
    None
}

/// Detect and return the best available f64 LLR function bundle.
pub fn detect_f64() -> Option<LlrFnsF64> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86_f64();
    }
    #[allow(unreachable_code)]
    None
}

/// Legacy detect function (returns f32 version).
pub fn detect() -> Option<LlrFns> {
    detect_f32()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86_f32() -> Option<LlrFnsF32> {
    use std::arch::is_x86_feature_detected;

    if is_x86_feature_detected!("avx2") {
        Some(LlrFnsF32 {
            minsum_fn: minsum_avx2_f32_safe,
            maxabs_fn: maxabs_avx2_f32_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86_f64() -> Option<LlrFnsF64> {
    use std::arch::is_x86_feature_detected;

    if is_x86_feature_detected!("avx2") {
        Some(LlrFnsF64 {
            minsum_fn: minsum_avx2_f64_safe,
            maxabs_fn: maxabs_avx2_f64_safe,
        })
    } else {
        None
    }
}

/// Safe wrapper for AVX2 min-sum (f32).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn minsum_avx2_f32_safe(inputs: &[f32]) -> f32 {
    unsafe { minsum_avx2_f32(inputs) }
}

/// Safe wrapper for AVX2 max absolute value (f32).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn maxabs_avx2_f32_safe(inputs: &[f32]) -> f32 {
    unsafe { maxabs_avx2_f32(inputs) }
}

/// Safe wrapper for AVX2 min-sum (f64).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn minsum_avx2_f64_safe(inputs: &[f64]) -> f64 {
    unsafe { minsum_avx2_f64(inputs) }
}

/// Safe wrapper for AVX2 max absolute value (f64).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn maxabs_avx2_f64_safe(inputs: &[f64]) -> f64 {
    unsafe { maxabs_avx2_f64(inputs) }
}

/// Compute sign-preserving horizontal minimum using AVX2 (f32).
///
/// Returns: sign_product * min(|inputs|)
///
/// # Safety
/// Requires AVX2 CPU feature.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn minsum_avx2_f32(inputs: &[f32]) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    if inputs.is_empty() {
        return 0.0;
    }

    let n = inputs.len();

    // Sign mask: 0x80000000 in each lane (sign bit of f32)
    let sign_mask = _mm256_set1_ps(-0.0f32);

    // Initialize accumulators
    let mut vec_min = _mm256_set1_ps(f32::INFINITY);
    let mut vec_sign = _mm256_setzero_ps(); // Accumulate XOR of sign bits

    // Process 8 floats at a time
    let chunks = n / 8;
    for i in 0..chunks {
        let ptr = inputs.as_ptr().add(i * 8);
        let vals = _mm256_loadu_ps(ptr);

        // Extract absolute values: vals & ~sign_mask
        let abs_vals = _mm256_andnot_ps(sign_mask, vals);
        vec_min = _mm256_min_ps(vec_min, abs_vals);

        // Extract sign bits and XOR accumulate
        let signs = _mm256_and_ps(vals, sign_mask);
        vec_sign = _mm256_xor_ps(vec_sign, signs);
    }

    // Horizontal reduction: extract scalar minimum from vector
    let mut temp = [0.0f32; 8];
    _mm256_storeu_ps(temp.as_mut_ptr(), vec_min);
    let mut min_abs = f32::INFINITY;
    for &val in &temp {
        min_abs = min_abs.min(val);
    }

    // Extract final sign: count negative signs in vector
    _mm256_storeu_ps(temp.as_mut_ptr(), vec_sign);
    let mut sign_product = 1.0f32;
    for &s in &temp {
        // Check if sign bit is set (negative)
        if s.to_bits() & 0x8000_0000 != 0 {
            sign_product = -sign_product;
        }
    }

    // Handle remainder scalarly
    for &val in &inputs[chunks * 8..] {
        min_abs = min_abs.min(val.abs());
        if val < 0.0 {
            sign_product = -sign_product;
        }
    }

    sign_product * min_abs
}

/// Compute maximum absolute value using AVX2 (f32).
///
/// # Safety
/// Requires AVX2 CPU feature.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn maxabs_avx2_f32(inputs: &[f32]) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    if inputs.is_empty() {
        return 0.0;
    }

    let n = inputs.len();
    let sign_mask = _mm256_set1_ps(-0.0f32);
    let mut vec_max = _mm256_setzero_ps();

    // Process 8 floats at a time
    let chunks = n / 8;
    for i in 0..chunks {
        let ptr = inputs.as_ptr().add(i * 8);
        let vals = _mm256_loadu_ps(ptr);
        let abs_vals = _mm256_andnot_ps(sign_mask, vals);
        vec_max = _mm256_max_ps(vec_max, abs_vals);
    }

    // Horizontal reduction
    let mut temp = [0.0f32; 8];
    _mm256_storeu_ps(temp.as_mut_ptr(), vec_max);
    let mut max_val = 0.0f32;
    for &val in &temp {
        max_val = max_val.max(val);
    }

    // Handle remainder scalarly
    for &val in &inputs[chunks * 8..] {
        max_val = max_val.max(val.abs());
    }

    max_val
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference scalar implementation for testing.
    fn scalar_minsum(inputs: &[f32]) -> f32 {
        if inputs.is_empty() {
            return 0.0;
        }

        let mut min_abs = f32::INFINITY;
        let mut sign_product = 1.0f32;

        for &val in inputs {
            min_abs = min_abs.min(val.abs());
            if val < 0.0 {
                sign_product = -sign_product;
            }
        }

        sign_product * min_abs
    }

    /// Reference scalar implementation for testing.
    fn scalar_maxabs(inputs: &[f32]) -> f32 {
        inputs.iter().map(|x| x.abs()).fold(0.0f32, f32::max)
    }

    #[test]
    fn test_detection() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("avx2") {
                assert!(fns.is_some(), "AVX2 available but detection failed");
            } else {
                eprintln!("AVX2 not available, SIMD will use fallback");
            }
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_empty() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            eprintln!("Skipping: AVX2 not available");
            return;
        }

        let fns = detect().unwrap();
        let result = (fns.minsum_fn)(&[]);
        assert_eq!(result, 0.0, "Empty input should return 0.0");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_single_positive() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![5.0];
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "Single positive value");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_single_negative() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![-3.0];
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "Single negative value");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_all_positive() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![1.0, 2.0, 3.0, 4.0];
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "All positive: min=1.0, sign=+1");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_all_negative() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![-1.0, -2.0, -3.0, -4.0];
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "All negative: 4 negatives → +1.0");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_mixed_signs() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![1.0, -2.0, 3.0, -4.0];
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "Mixed signs: 2 negatives → +1.0");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_odd_negatives() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![2.0, -3.0, 4.0];
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "Odd number of negatives → -2.0");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_large_vector() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        // Test with > 8 elements to exercise AVX2 vector path + remainder
        let inputs: Vec<f32> = (1..=20)
            .map(|i| if i % 3 == 0 { -(i as f32) } else { i as f32 })
            .collect();
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "Large vector (20 elements)");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_exact_8_elements() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0];
        let result = (fns.minsum_fn)(&inputs);
        let expected = scalar_minsum(&inputs);
        assert_eq!(result, expected, "Exactly 8 elements (one AVX2 vector)");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_maxabs_empty() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let result = (fns.maxabs_fn)(&[]);
        assert_eq!(result, 0.0, "Empty input should return 0.0");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_maxabs_single() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![-7.5];
        let result = (fns.maxabs_fn)(&inputs);
        let expected = scalar_maxabs(&inputs);
        assert_eq!(result, expected, "Single value: |-7.5| = 7.5");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_maxabs_multiple() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs = vec![1.0, -5.0, 3.0, -2.0];
        let result = (fns.maxabs_fn)(&inputs);
        let expected = scalar_maxabs(&inputs);
        assert_eq!(result, expected, "Max of [1.0, -5.0, 3.0, -2.0] = 5.0");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_maxabs_large_vector() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();
        let inputs: Vec<f32> = (1..=25)
            .map(|i| if i == 17 { 100.0 } else { i as f32 })
            .collect();
        let result = (fns.maxabs_fn)(&inputs);
        let expected = scalar_maxabs(&inputs);
        assert_eq!(result, expected, "Max should be 100.0");
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_minsum_vs_scalar_random() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();

        // Test multiple random-ish patterns
        let test_cases = vec![
            vec![1.5, -2.3, 4.7, -0.8, 9.2],
            vec![-1.1, -2.2, -3.3, -4.4, -5.5, -6.6],
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9],
            (1..=30)
                .map(|i| (i as f32) * if i % 2 == 0 { -1.0 } else { 1.0 })
                .collect(),
        ];

        for inputs in test_cases {
            let result = (fns.minsum_fn)(&inputs);
            let expected = scalar_minsum(&inputs);
            assert!(
                (result - expected).abs() < 1e-6,
                "SIMD result {} differs from scalar {} for inputs {:?}",
                result,
                expected,
                inputs
            );
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_maxabs_vs_scalar_random() {
        use std::arch::is_x86_feature_detected;
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        let fns = detect().unwrap();

        let test_cases = vec![
            vec![1.5, -2.3, 4.7, -0.8, 9.2],
            vec![-10.5, 3.2, -7.8, 2.1],
            (1..=15).map(|i| (i as f32) * 0.5).collect(),
        ];

        for inputs in test_cases {
            let result = (fns.maxabs_fn)(&inputs);
            let expected = scalar_maxabs(&inputs);
            assert!(
                (result - expected).abs() < 1e-6,
                "SIMD result {} differs from scalar {} for inputs {:?}",
                result,
                expected,
                inputs
            );
        }
    }
}

/// Compute sign-preserving horizontal minimum using AVX2 (f64).
///
/// Returns: sign_product * min(|inputs|)
///
/// # Safety
/// Requires AVX2 CPU feature.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn minsum_avx2_f64(inputs: &[f64]) -> f64 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    if inputs.is_empty() {
        return 0.0;
    }

    let n = inputs.len();
    let sign_mask = _mm256_set1_pd(-0.0f64);
    let mut vec_min = _mm256_set1_pd(f64::INFINITY);
    let mut vec_sign = _mm256_setzero_pd();

    // Process 4 f64 at a time with AVX2
    let chunks = n / 4;
    for i in 0..chunks {
        let ptr = inputs.as_ptr().add(i * 4);
        let vals = _mm256_loadu_pd(ptr);
        let abs_vals = _mm256_andnot_pd(sign_mask, vals);
        vec_min = _mm256_min_pd(vec_min, abs_vals);
        let signs = _mm256_and_pd(vals, sign_mask);
        vec_sign = _mm256_xor_pd(vec_sign, signs);
    }

    // Horizontal reduction
    let mut temp = [0.0f64; 4];
    _mm256_storeu_pd(temp.as_mut_ptr(), vec_min);
    let mut min_abs = f64::INFINITY;
    for &val in &temp {
        min_abs = min_abs.min(val);
    }

    _mm256_storeu_pd(temp.as_mut_ptr(), vec_sign);
    let mut sign_product = 1.0f64;
    for &s in &temp {
        if s.to_bits() & 0x8000_0000_0000_0000 != 0 {
            sign_product = -sign_product;
        }
    }

    // Handle remainder
    for &val in &inputs[chunks * 4..] {
        min_abs = min_abs.min(val.abs());
        if val < 0.0 {
            sign_product = -sign_product;
        }
    }

    sign_product * min_abs
}

/// Compute maximum absolute value using AVX2 (f64).
///
/// # Safety
/// Requires AVX2 CPU feature.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn maxabs_avx2_f64(inputs: &[f64]) -> f64 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    if inputs.is_empty() {
        return 0.0;
    }

    let n = inputs.len();
    let sign_mask = _mm256_set1_pd(-0.0f64);
    let mut vec_max = _mm256_setzero_pd();

    let chunks = n / 4;
    for i in 0..chunks {
        let ptr = inputs.as_ptr().add(i * 4);
        let vals = _mm256_loadu_pd(ptr);
        let abs_vals = _mm256_andnot_pd(sign_mask, vals);
        vec_max = _mm256_max_pd(vec_max, abs_vals);
    }

    let mut temp = [0.0f64; 4];
    _mm256_storeu_pd(temp.as_mut_ptr(), vec_max);
    let mut max_val = 0.0f64;
    for &val in &temp {
        max_val = max_val.max(val);
    }

    for &val in &inputs[chunks * 4..] {
        max_val = max_val.max(val.abs());
    }

    max_val
}
