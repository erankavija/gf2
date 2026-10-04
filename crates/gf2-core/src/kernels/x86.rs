//! x86/x86-64 CPU feature detection; the kernels live in `gf2-kernels-simd`.

/// Checks if AVX2 is available on the current CPU.
#[cfg(target_arch = "x86_64")]
pub fn has_avx2() -> bool {
    is_x86_feature_detected!("avx2")
}

#[cfg(target_arch = "x86")]
pub fn has_avx2() -> bool {
    is_x86_feature_detected!("avx2")
}

/// Checks if AVX-512F is available on the current CPU.
#[cfg(target_arch = "x86_64")]
pub fn has_avx512f() -> bool {
    is_x86_feature_detected!("avx512f")
}

#[cfg(target_arch = "x86")]
pub fn has_avx512f() -> bool {
    false // AVX-512 not available on 32-bit x86
}

/// Checks if PCLMULQDQ is available on the current CPU.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn has_pclmulqdq() -> bool {
    is_x86_feature_detected!("pclmulqdq")
}

/// Checks if BMI2 is available on the current CPU.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn has_bmi2() -> bool {
    is_x86_feature_detected!("bmi2")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_detection() {
        let _ = has_avx2();
        let _ = has_avx512f();
        let _ = has_pclmulqdq();
        let _ = has_bmi2();
    }
}
