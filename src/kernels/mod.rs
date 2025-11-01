//! Kernel module for runtime dispatch of optimized implementations.
//!
//! This module provides a trait-based interface for bulk operations on bit vectors,
//! with implementations for different CPU architectures and feature sets.
//!
//! Currently, only the scalar baseline is implemented. Future versions will add:
//! - AVX2/AVX-512 kernels for x86-64
//! - NEON kernels for AArch64
//! - Runtime feature detection and dispatch

pub mod ops;
pub mod scalar;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod x86;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

/// Trait defining bulk operations on bit vector buffers.
///
/// Implementations of this trait can provide optimized routines using
/// SIMD instructions or other architecture-specific features.
pub trait Kernel: Send + Sync {
    /// Performs bitwise AND on two buffers, storing the result in `dst`.
    fn and(&self, dst: &mut [u64], src: &[u64]);

    /// Performs bitwise OR on two buffers, storing the result in `dst`.
    fn or(&self, dst: &mut [u64], src: &[u64]);

    /// Performs bitwise XOR on two buffers, storing the result in `dst`.
    fn xor(&self, dst: &mut [u64], src: &[u64]);

    /// Performs bitwise NOT on a buffer.
    fn not(&self, buf: &mut [u64]);

    /// Counts the number of set bits (population count) in a buffer.
    fn popcount(&self, buf: &[u64]) -> u64;
}

/// Selects the best available kernel for the current CPU.
///
/// Currently returns the scalar baseline. Future versions will detect
/// CPU features and return optimized implementations when available.
pub fn select_kernel() -> &'static dyn Kernel {
    // TODO: Add runtime feature detection
    // - x86/x86_64: detect AVX2, AVX-512, PCLMULQDQ, BMI2
    // - aarch64: detect NEON
    &scalar::SCALAR_KERNEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_kernel_returns_valid() {
        let kernel = select_kernel();
        let mut a = vec![0xFFFFFFFFFFFFFFFFu64];
        let b = vec![0x0F0F0F0F0F0F0F0Fu64];
        kernel.xor(&mut a, &b);
        assert_eq!(a[0], 0xF0F0F0F0F0F0F0F0u64);
    }
}
