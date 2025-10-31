//! Scalar baseline kernel implementation.
//!
//! This module provides a portable, scalar implementation of bulk operations
//! that works on all platforms. It serves as the fallback when SIMD
//! implementations are not available.

use super::Kernel;

/// Scalar kernel implementation using simple loops.
pub struct ScalarKernel;

/// Global instance of the scalar kernel.
pub static SCALAR_KERNEL: ScalarKernel = ScalarKernel;

impl Kernel for ScalarKernel {
    #[inline]
    fn and(&self, dst: &mut [u64], src: &[u64]) {
        let len = dst.len().min(src.len());
        for i in 0..len {
            dst[i] &= src[i];
        }
    }

    #[inline]
    fn or(&self, dst: &mut [u64], src: &[u64]) {
        let len = dst.len().min(src.len());
        for i in 0..len {
            dst[i] |= src[i];
        }
    }

    #[inline]
    fn xor(&self, dst: &mut [u64], src: &[u64]) {
        let len = dst.len().min(src.len());
        for i in 0..len {
            dst[i] ^= src[i];
        }
    }

    #[inline]
    fn not(&self, buf: &mut [u64]) {
        for word in buf.iter_mut() {
            *word = !*word;
        }
    }

    #[inline]
    fn popcount(&self, buf: &[u64]) -> u64 {
        buf.iter().map(|w| w.count_ones() as u64).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_and() {
        let mut a = vec![0xFF, 0xFF];
        let b = vec![0x0F, 0xF0];
        SCALAR_KERNEL.and(&mut a, &b);
        assert_eq!(a, vec![0x0F, 0xF0]);
    }

    #[test]
    fn test_scalar_or() {
        let mut a = vec![0xF0, 0x0F];
        let b = vec![0x0F, 0xF0];
        SCALAR_KERNEL.or(&mut a, &b);
        assert_eq!(a, vec![0xFF, 0xFF]);
    }

    #[test]
    fn test_scalar_xor() {
        let mut a = vec![0xFF, 0xFF];
        let b = vec![0x0F, 0xF0];
        SCALAR_KERNEL.xor(&mut a, &b);
        assert_eq!(a, vec![0xF0, 0x0F]);
    }

    #[test]
    fn test_scalar_not() {
        let mut a = vec![0xFFFFFFFFFFFFFFFFu64, 0x0000000000000000u64];
        SCALAR_KERNEL.not(&mut a);
        assert_eq!(a, vec![0x0000000000000000u64, 0xFFFFFFFFFFFFFFFFu64]);
    }

    #[test]
    fn test_scalar_popcount() {
        let a = vec![0xFFu64, 0xF0F0F0F0F0F0F0F0u64];
        let count = SCALAR_KERNEL.popcount(&a);
        assert_eq!(count, 8 + 32);
    }
}
