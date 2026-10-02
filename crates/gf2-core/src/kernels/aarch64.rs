//! AArch64 (ARM64) CPU feature detection. NEON and PMULL kernels are
//! `@/issue/194b902a`.

/// Checks if NEON is available on the current CPU.
///
/// On AArch64, NEON is mandatory, so this always returns true.
#[cfg(target_arch = "aarch64")]
pub fn has_neon() -> bool {
    // NEON is mandatory on AArch64
    cfg!(target_feature = "neon") || true
}

/// Checks if AES/crypto extensions are available.
#[cfg(target_arch = "aarch64")]
pub fn has_crypto() -> bool {
    // Compile-time only; runtime detection belongs to `@/issue/194b902a`.
    cfg!(target_feature = "aes")
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "aarch64")]
    use super::*;

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_feature_detection() {
        // Just verify that feature detection doesn't panic
        assert!(has_neon()); // Should always be true on AArch64
        let _ = has_crypto();
    }
}
