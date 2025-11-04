//! Error-correcting codes built on gf2 primitives.
//!
//! This module provides implementations of error-correcting codes using the
//! BitVec and BitMatrix types from the gf2 library. It includes both block codes
//! and streaming (convolutional) codes.
//!
//! # Block Codes
//!
//! Block codes encode fixed-length messages into fixed-length codewords.
//! The main implementation is [`LinearBlockCode`], which supports:
//! - Systematic encoding using generator matrices
//! - Syndrome computation using parity-check matrices
//! - Standard Hamming codes via [`LinearBlockCode::hamming_7_4()`] and [`LinearBlockCode::hamming()`]
//!
//! Decoding is provided by [`SyndromeTableDecoder`], which uses a precomputed
//! syndrome table for efficient single-error correction.
//!
//! # Streaming Codes
//!
//! Convolutional codes process bits in a streaming fashion. The module provides
//! skeleton implementations in [`ConvolutionalEncoder`] and [`ConvolutionalDecoder`]
//! for future expansion.
//!
//! # Examples
//!
//! ## Using Hamming codes
//!
//! ```
//! use gf2::coding::{LinearBlockCode, SyndromeTableDecoder};
//! use gf2::coding::traits::{BlockEncoder, HardDecisionDecoder};
//! use gf2::BitVec;
//!
//! // Create a Hamming(15,11) code with r=4
//! let code = LinearBlockCode::hamming(4);
//! assert_eq!(code.k(), 11);
//! assert_eq!(code.n(), 15);
//!
//! let decoder = SyndromeTableDecoder::new(code);
//!
//! // Encode a message
//! let mut msg = BitVec::new();
//! for i in 0..11 {
//!     msg.push_bit(i % 2 == 0);
//! }
//! let codeword = decoder.code().encode(&msg);
//!
//! // Decode (with or without errors)
//! let decoded = decoder.decode(&codeword);
//! assert_eq!(decoded, msg);
//! ```

pub mod convolutional;
pub mod linear;
pub mod traits;

// Re-export main types
pub use convolutional::{ConvolutionalDecoder, ConvolutionalEncoder};
pub use linear::{LinearBlockCode, SyndromeTableDecoder};
