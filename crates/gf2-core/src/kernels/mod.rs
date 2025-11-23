//! Kernel module for runtime dispatch of optimized implementations.
//!
//! This module provides a unified interface for different execution backends:
//! - **Scalar**: Pure Rust baseline (always available)
//! - **SIMD**: AVX2/AVX-512/NEON acceleration (optional, runtime detected)
//! - **GPU**: Future CUDA/OpenCL/Vulkan compute (planned)
//! - **FPGA**: Future hardware acceleration (planned)
//!
//! # Architecture
//!
//! - `Backend` trait: Defines operations all backends must implement
//! - `ops` module: High-level operations with smart dispatch
//! - `scalar` module: Pure Rust implementations
//! - Backend-specific modules: SIMD, GPU, FPGA implementations

pub mod backend;
pub mod ops;
pub mod scalar;

#[cfg(feature = "simd")]
pub mod simd;

#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod x86;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

// Re-export core types
pub use backend::{select_backend_for_size, Backend, SelectedBackend};
pub use scalar::ScalarBackend;

// Legacy compatibility: Keep old Kernel trait until migration is complete
/// Legacy trait for kernel operations.
///
/// **Deprecated**: Use [`Backend`] trait instead for new code.
#[deprecated(since = "0.1.0", note = "Use Backend trait instead")]
pub trait Kernel: Send + Sync {
    /// Performs bitwise AND.
    fn and(&self, dst: &mut [u64], src: &[u64]);
    /// Performs bitwise OR.
    fn or(&self, dst: &mut [u64], src: &[u64]);
    /// Performs bitwise XOR.
    fn xor(&self, dst: &mut [u64], src: &[u64]);
    /// Performs bitwise NOT.
    fn not(&self, buf: &mut [u64]);
    /// Counts set bits.
    fn popcount(&self, buf: &[u64]) -> u64;
}

// Bridge: ScalarBackend implements old Kernel trait for compatibility
#[allow(deprecated)]
impl Kernel for ScalarBackend {
    fn and(&self, dst: &mut [u64], src: &[u64]) {
        Backend::and(self, dst, src);
    }

    fn or(&self, dst: &mut [u64], src: &[u64]) {
        Backend::or(self, dst, src);
    }

    fn xor(&self, dst: &mut [u64], src: &[u64]) {
        Backend::xor(self, dst, src);
    }

    fn not(&self, buf: &mut [u64]) {
        Backend::not(self, buf);
    }

    fn popcount(&self, buf: &[u64]) -> u64 {
        Backend::popcount(self, buf)
    }
}

/// Legacy function for selecting kernel.
///
/// **Deprecated**: Use backend selection functions instead.
#[deprecated(since = "0.1.0", note = "Use backend selection instead")]
#[allow(deprecated)]
pub fn select_kernel() -> &'static dyn Kernel {
    &scalar::SCALAR_BACKEND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn test_select_kernel_returns_valid() {
        let kernel = select_kernel();
        let mut a = vec![0xFFFFFFFFFFFFFFFFu64];
        let b = vec![0x0F0F0F0F0F0F0F0Fu64];
        kernel.xor(&mut a, &b);
        assert_eq!(a[0], 0xF0F0F0F0F0F0F0F0u64);
    }
}
