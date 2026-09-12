//! Scalar baseline backend implementation.
//!
//! This module provides a portable, pure Rust implementation of all kernel
//! operations. It serves as the baseline for correctness testing and the
//! fallback when specialized backends (SIMD, GPU, FPGA) are unavailable.
//!
//! # Design
//!
//! - **Safety**: No unsafe code
//! - **Portability**: Works on all platforms
//! - **Performance**: Hand-unrolled loops for bulk operations
//! - **Correctness**: Reference implementation for testing

use crate::kernels::Backend;

/// Scalar backend using simple loops with manual unrolling.
#[derive(Debug, Clone, Copy)]
pub struct ScalarBackend;

/// Global instance of the scalar backend.
pub static SCALAR_BACKEND: ScalarBackend = ScalarBackend;

impl Backend for ScalarBackend {
    fn name(&self) -> &'static str {
        "scalar"
    }
    #[inline]
    fn and(&self, dst: &mut [u64], src: &[u64]) {
        let len = dst.len().min(src.len());
        let mut i = 0usize;
        const UNROLL: usize = 4;
        let limit = len - (len % UNROLL);
        while i < limit {
            dst[i] &= src[i];
            dst[i + 1] &= src[i + 1];
            dst[i + 2] &= src[i + 2];
            dst[i + 3] &= src[i + 3];
            i += UNROLL;
        }
        while i < len {
            dst[i] &= src[i];
            i += 1;
        }
    }

    #[inline]
    fn or(&self, dst: &mut [u64], src: &[u64]) {
        let len = dst.len().min(src.len());
        let mut i = 0usize;
        const UNROLL: usize = 4;
        let limit = len - (len % UNROLL);
        while i < limit {
            dst[i] |= src[i];
            dst[i + 1] |= src[i + 1];
            dst[i + 2] |= src[i + 2];
            dst[i + 3] |= src[i + 3];
            i += UNROLL;
        }
        while i < len {
            dst[i] |= src[i];
            i += 1;
        }
    }

    #[inline]
    fn xor(&self, dst: &mut [u64], src: &[u64]) {
        let len = dst.len().min(src.len());
        let mut i = 0usize;
        const UNROLL: usize = 4;
        let limit = len - (len % UNROLL);
        while i < limit {
            dst[i] ^= src[i];
            dst[i + 1] ^= src[i + 1];
            dst[i + 2] ^= src[i + 2];
            dst[i + 3] ^= src[i + 3];
            i += UNROLL;
        }
        while i < len {
            dst[i] ^= src[i];
            i += 1;
        }
    }

    #[inline]
    fn not(&self, buf: &mut [u64]) {
        let len = buf.len();
        let mut i = 0usize;
        const UNROLL: usize = 4;
        let limit = len - (len % UNROLL);
        while i < limit {
            buf[i] = !buf[i];
            buf[i + 1] = !buf[i + 1];
            buf[i + 2] = !buf[i + 2];
            buf[i + 3] = !buf[i + 3];
            i += UNROLL;
        }
        while i < len {
            buf[i] = !buf[i];
            i += 1;
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
        SCALAR_BACKEND.and(&mut a, &b);
        assert_eq!(a, vec![0x0F, 0xF0]);
    }

    #[test]
    fn test_scalar_or() {
        let mut a = vec![0xF0, 0x0F];
        let b = vec![0x0F, 0xF0];
        SCALAR_BACKEND.or(&mut a, &b);
        assert_eq!(a, vec![0xFF, 0xFF]);
    }

    #[test]
    fn test_scalar_xor() {
        let mut a = vec![0xFF, 0xFF];
        let b = vec![0x0F, 0xF0];
        SCALAR_BACKEND.xor(&mut a, &b);
        assert_eq!(a, vec![0xF0, 0x0F]);
    }

    #[test]
    fn test_scalar_not() {
        let mut a = vec![0xFFFFFFFFFFFFFFFFu64, 0x0000000000000000u64];
        SCALAR_BACKEND.not(&mut a);
        assert_eq!(a, vec![0x0000000000000000u64, 0xFFFFFFFFFFFFFFFFu64]);
    }

    #[test]
    fn test_scalar_popcount() {
        let a = vec![0xFFu64, 0xF0F0F0F0F0F0F0F0u64];
        let count = SCALAR_BACKEND.popcount(&a);
        assert_eq!(count, 8 + 32);
    }
}
