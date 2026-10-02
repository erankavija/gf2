//! Error-correcting codes built on `gf2-core` primitives.
//!
//! This crate provides implementations of error-correcting codes over the
//! field abstractions of the `gf2-core` library: the canonical trait surface
//! is generic over the code symbol field and representation
//! ([`FieldVec`](gf2_core::field::FieldVec)/[`FieldMatrix`](gf2_core::field::matrix::FieldMatrix)),
//! with packed binary specializations using
//! [`BitVec`](gf2_core::BitVec) and [`BitMatrix`](gf2_core::BitMatrix).
//! It includes block codes and streaming (convolutional) codes. Besides the
//! families below it provides LDPC, GLDPC, product, CRC, and Reed-Muller
//! subcodes; BCJR, GRAND, and ordered-statistics decoders; modems, channels,
//! and Monte Carlo simulation. The module index lists each.

#![deny(unsafe_code)]
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
//! # BCH Codes
//!
//! The [`bch`] module constructs BCH codes over any supported base field from
//! independent inputs, encodes them systematically in a declared coordinate
//! layout, decodes binary codes, and materializes their generator and
//! parity-check matrices. Its module documentation is the researcher's
//! guide: the workflow, distance terminology, coordinate conventions, error
//! contracts, decoder guarantees, complexity, and performance-path
//! selection, with the runnable examples that exercise them.
//!
//! # Streaming Codes
//!
//! [`ConvolutionalEncoder`] is a feedforward shift-register encoder and
//! [`ConvolutionalDecoder`] a hard-decision Viterbi decoder.
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
pub mod bcjr;
pub mod channel;
pub mod convolutional;
pub mod crc;
pub mod drm;
pub mod dvb_t2_bicm_harness;
pub mod error;
pub mod fading;
pub mod gldpc;
pub mod grand;
pub mod info_theory;
pub mod ldpc;
pub mod linear;
pub mod llr;
pub mod modem;
pub mod osd;
pub mod product;

pub mod simulation;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod traits;
pub mod transform;
pub mod tuning;

// Re-export main types
pub use bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
pub use bch::CodeRate;
pub use channel::AwgnChannel;
pub use convolutional::{ConvolutionalDecoder, ConvolutionalEncoder};
pub use error::{Capability, CodeCapability, CodeError, RepresentationId};
pub use ldpc::{
    CirculantMatrix, DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder, QuasiCyclicLdpc,
};
pub use linear::{LinearBlockCode, SyndromeTableDecoder};
pub use llr::Llr;
pub use traits::{DecoderResult, GeneratorMatrixAccess};
