//! # gf2 - High-Performance Bit String Manipulation
//!
//! This library provides efficient bit string operations with a focus on GF(2) arithmetic
//! and coding theory applications.
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
//! use gf2::BitVec;
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
mod bitvec;
pub mod kernels;
mod macros;
pub mod matrix;

pub use bitvec::BitVec;
