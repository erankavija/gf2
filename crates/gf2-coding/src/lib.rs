//! Error-correcting codes built on `gf2-core` primitives.
//!
//! This crate provides implementations of error-correcting codes using the
//! [`BitVec`](gf2_core::BitVec) and [`BitMatrix`](gf2_core::BitMatrix) types from
//! the `gf2-core` library. It includes both block codes
//! and streaming (convolutional) codes.
//!
//! # Block Codes
//!
//! Block codes encode fixed-length messages into fixed-length codewords.
//! The main implementation is [`LinearBlockCode`], which supports:
//! - Systematic encoding using generator matrices
//! - Syndrome computation using parity-check matrices
//! - Standard Hamming codes via [`LinearBlockCode::hamming()`]
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
//! # Soft-Decision Decoding
//!
//! The [`llr`] module provides log-likelihood ratio (LLR) types for soft-decision
//! decoding, enabling superior performance over AWGN channels.
//!
//! # Examples
//!
//! ## Using Hamming codes
//!
//! ```
//! use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
//! use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
//! use gf2_core::BitVec;
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

pub mod bch;
pub mod channel;
pub mod convolutional;
pub mod ldpc;
pub mod linear;
pub mod llr;

// SIMD detection is now handled internally in llr.rs via once_cell::Lazy
pub mod simulation;
pub mod traits;

// Re-export main types
pub use bch::{BchCode, BchDecoder, BchEncoder, CodeRate};
pub use channel::{AwgnChannel, BpskModulator};
pub use convolutional::{ConvolutionalDecoder, ConvolutionalEncoder};
pub use ldpc::{CirculantMatrix, LdpcCode, LdpcDecoder, QuasiCyclicLdpc};
pub use linear::{LinearBlockCode, SyndromeTableDecoder};
pub use llr::Llr;
pub use traits::{DecoderResult, GeneratorMatrixAccess};
