//! # gf2-core - High-Performance GF(2) Primitives
//!
//! This crate provides efficient bit string and matrix operations with a focus on GF(2)
//! arithmetic. It powers higher-level coding theory and compression tooling provided by
//! the companion `gf2-coding` crate.
//!
//! ## Core Types
//!
//! - [`BitVec`]: An owning, growable bit string backed by `Vec<u64>`.
//! - [`matrix::BitMatrix`]: A row-major, bit-packed boolean matrix for GF(2) linear algebra.
//!
//! ## Design Invariants
//!
//! - **Storage**: Dense contiguous `u64` words in little-endian bit order.
//! - **Bit Numbering**: Within each word, bit `i` maps to `word = i >> 6`, `mask = 1u64 << (i & 63)`.
//! - **Tail Masking**: Padding bits beyond `len_bits` in the last word are always zeroed.
//!
//! ## Examples
//!
//! ```
//! use gf2_core::BitVec;
//!
//! let mut bv = BitVec::new();
//! bv.push_bit(true);
//! bv.push_bit(false);
//! bv.push_bit(true);
//!
//! assert_eq!(bv.len(), 3);
//! assert_eq!(bv.get(0), true);
//! assert_eq!(bv.get(1), false);
//! assert_eq!(bv.count_ones(), 2);
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod alg;
mod bitslice;
mod bitvec;
pub mod gf2m;
pub mod kernels;
mod macros;
pub mod matrix;
pub mod sparse;

pub use bitslice::{BitSlice, BitSliceMut};
pub use bitvec::BitVec;

// Optional SIMD accessor: compiled only when the "simd" feature is enabled.
// This module contains no unsafe code; unsafe is isolated in the separate
// gf2-kernels-simd crate.
#[cfg(feature = "simd")]
pub(crate) mod simd {
    use gf2_kernels_simd::LogicalFns;
    use std::sync::OnceLock;

    static FNS: OnceLock<Option<LogicalFns>> = OnceLock::new();

    #[inline]
    pub fn maybe_simd() -> Option<&'static LogicalFns> {
        FNS.get_or_init(gf2_kernels_simd::detect).as_ref()
    }
}

#[cfg(not(feature = "simd"))]
pub(crate) mod simd {
    #[allow(dead_code)]
    #[inline]
    pub fn maybe_simd() -> Option<()> {
        None
    }
}
