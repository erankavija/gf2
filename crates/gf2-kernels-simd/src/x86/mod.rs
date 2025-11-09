#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::is_x86_feature_detected;

use crate::LogicalFns;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod avx2;

#[allow(dead_code)]
pub(crate) fn detect_x86() -> Option<LogicalFns> {
    // Prefer AVX2; add AVX-512F later when kernels are ready.
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) && is_x86_feature_detected!("avx2") {
        return Some(avx2::fns());
    }
    None
}
