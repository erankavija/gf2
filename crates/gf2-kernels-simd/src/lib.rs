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

pub mod bipedal;
pub mod clmul_scalar;
pub mod fp65537;
pub mod fp_generic;
pub mod fp_medium;
pub mod fp_medium_f64;
pub mod fp_medium_ple;
pub mod fp_small;
pub mod fp_small_f32;
pub mod fp_small_panel;
pub mod fp_small_ple;
pub mod gf2m;
pub mod gf2m_batch;
pub mod gf2m_gemm;
pub mod gf2m_wide;
pub mod llr;
pub mod mersenne;
pub mod modem;
pub mod prefetch;
pub mod transpose;

pub use clmul_scalar::clmul_u64_scalar;
pub use prefetch::prefetch_read_l1;

/// Register-tiled M4RM 8×4 C-update function.
///
/// `c_block` contains eight contiguous output rows with `stride_words` words per
/// row. The function XORs four words beginning at `word_start` from each indexed
/// Gray-table entry into the matching output row.
///
/// # Panics
///
/// Panics if `c_block` does not contain eight full rows, if `word_start..word_start+4`
/// is outside the row stride, or if `table_buffer` does not cover every indexed
/// Gray-table row.
pub type M4rmTile8x4Fn = fn(&mut [u64], usize, usize, &[u64], &[usize; 8]);

/// Register-tiled M4RM full-row C-update function.
///
/// Processes a row block as repeated 8×4 YMM tiles across the full row width,
/// leaving any non-multiple-of-four tail to the safe caller.
///
/// # Panics
///
/// Panics if `c_block` does not contain eight full rows or if `table_buffer`
/// does not cover every indexed Gray-table row up to the largest processed
/// multiple-of-four word offset.
pub type M4rmTile8xNFn = fn(&mut [u64], usize, &[u64], &[usize; 8]);

/// Full Gray-code table builder for narrow rows.
///
/// Builds an entire `table_size`-entry Gray-code table into `buffer`, given the
/// `valid_rows` contiguous panel rows of B in `panel` (each `stride_words` wide).
/// `buffer` and `panel` are laid out row-major with `stride_words` words per
/// entry/row. Entry 0 is the zero vector; entry `g` holds the XOR of panel rows
/// whose bit is set in binary index `g`. Only the first `valid_rows` panel rows
/// contribute (the rest are treated as absent, matching the M4RM tail panel).
///
/// # Panics
///
/// Panics if `buffer` is smaller than `table_size * stride_words`, if `panel`
/// is smaller than `valid_rows * stride_words`, or if `stride_words` is not the
/// width this builder specializes for.
pub type M4rmGrayBuildFn = fn(&mut [u64], &[u64], usize, usize, usize);

/// Set of accelerated logical operations. Each function must have identical
/// semantics to the scalar implementation (in-place dst modification, slice length min).
#[derive(Copy, Clone)]
pub struct LogicalFns {
    pub and_fn: fn(&mut [u64], &[u64]),
    pub or_fn: fn(&mut [u64], &[u64]),
    pub xor_fn: fn(&mut [u64], &[u64]),
    pub m4rm_gray_xor16_fn: fn(&mut [[u64; 8]; 2], &[u64]),
    pub m4rm_gray_build4_fn: M4rmGrayBuildFn,
    pub m4rm_gray_build8_fn: M4rmGrayBuildFn,
    pub m4rm_tile8x4_fn: M4rmTile8x4Fn,
    pub m4rm_tile8xn_fn: M4rmTile8xNFn,
    pub not_fn: fn(&mut [u64]),
    pub popcnt_fn: fn(&[u64]) -> u64,
    pub and_popcnt_fn: fn(&[u64], &[u64]) -> u64,
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
