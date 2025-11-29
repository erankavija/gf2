#![allow(clippy::missing_safety_doc)]
//! SIMD-accelerated logical kernels for `gf2-core`.
//!
//! This crate isolates unsafe and architecture-specific code. All public APIs are
//! safe and return plain function pointers that operate on `&mut [u64]` / `&[u64]`.
//! Runtime detection chooses the best available backend; if none match, callers
//! should fall back to scalar loops.
//!
//! Supported (x86_64): AVX2, AVX-512F (experimental).
//! AArch64 NEON planned.

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86;

pub mod gf2m;
pub mod llr;

/// Set of accelerated logical operations. Each function must have identical
/// semantics to the scalar implementation (in-place dst modification, slice length min).
pub struct LogicalFns {
    pub and_fn: fn(&mut [u64], &[u64]),
    pub or_fn: fn(&mut [u64], &[u64]),
    pub xor_fn: fn(&mut [u64], &[u64]),
    pub not_fn: fn(&mut [u64]),
    pub popcnt_fn: fn(&[u64]) -> u64,
    pub find_first_one_fn: fn(&[u64]) -> Option<usize>,
    pub find_first_zero_fn: fn(&[u64]) -> Option<usize>,
    pub shift_left_words_fn: fn(&mut [u64], usize),
    pub shift_right_words_fn: fn(&mut [u64], usize),
}

/// Detect and return the best available logical function bundle.
pub fn detect() -> Option<LogicalFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return x86::detect_x86();
    }
    #[allow(unreachable_code)]
    None
}
