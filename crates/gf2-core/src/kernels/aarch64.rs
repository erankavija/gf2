//! AArch64 CPU feature detection.

/// Always `true`: NEON is mandatory on AArch64.
#[cfg(target_arch = "aarch64")]
pub fn has_neon() -> bool {
    cfg!(target_feature = "neon") || true
}

/// Whether the AES extension is enabled at compile time.
#[cfg(target_arch = "aarch64")]
pub fn has_crypto() -> bool {
    cfg!(target_feature = "aes")
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "aarch64")]
    use super::*;

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_feature_detection() {
        assert!(has_neon());
        let _ = has_crypto();
    }
}
