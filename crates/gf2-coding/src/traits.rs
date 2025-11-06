//! Traits for error-correcting codes.
//!
//! This module defines the core traits for encoding and decoding operations
//! in error-correcting codes, supporting both block codes and streaming codes.

use gf2_core::BitVec;

/// Encoder for block codes.
///
/// A block encoder transforms fixed-length message blocks into fixed-length codewords.
/// The encoder is characterized by:
/// - `k`: the number of message bits
/// - `n`: the number of codeword bits
/// - The code rate is `k/n`
pub trait BlockEncoder {
    /// Returns the number of message bits (dimension).
    fn k(&self) -> usize;

    /// Returns the number of codeword bits (length).
    fn n(&self) -> usize;

    /// Encodes a message into a codeword.
    ///
    /// # Arguments
    ///
    /// * `message` - A bit vector of length `k` containing the message bits
    ///
    /// # Returns
    ///
    /// A bit vector of length `n` containing the encoded codeword
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != k()`
    fn encode(&self, message: &BitVec) -> BitVec;
}

/// Hard-decision decoder for block codes.
///
/// A hard-decision decoder takes a received codeword (where each bit is a hard 0 or 1 decision)
/// and attempts to recover the original message bits, potentially correcting errors.
pub trait HardDecisionDecoder {
    /// Decodes a received codeword and returns the estimated message bits.
    ///
    /// # Arguments
    ///
    /// * `received` - The received bit vector (potentially with errors)
    ///
    /// # Returns
    ///
    /// A bit vector containing the decoded message bits
    ///
    /// # Panics
    ///
    /// Panics if the received vector has incorrect length
    fn decode(&self, received: &BitVec) -> BitVec;
}

/// Soft-decision decoder for block codes (placeholder for future LLR-based decoding).
///
/// A soft-decision decoder uses soft information (e.g., log-likelihood ratios)
/// to make better decoding decisions than hard-decision decoders.
/// This trait is currently a placeholder for future implementations.
pub trait SoftDecisionDecoder {
    /// Decodes using soft information (e.g., LLRs).
    ///
    /// # Arguments
    ///
    /// * `soft_bits` - Soft information for each bit position
    ///
    /// # Returns
    ///
    /// Decoded message bits
    ///
    /// # Note
    ///
    /// This is a placeholder trait. The exact signature and soft bit representation
    /// will be refined in future implementations.
    fn decode_soft(&self, soft_bits: &[f64]) -> BitVec;
}

/// Streaming encoder for convolutional codes.
///
/// A streaming encoder processes bits one at a time, maintaining internal state
/// across multiple encode operations. This is used for convolutional codes.
pub trait StreamingEncoder {
    /// Encodes a single input bit and returns the output symbol(s).
    ///
    /// # Arguments
    ///
    /// * `input` - The input bit to encode
    ///
    /// # Returns
    ///
    /// A vector of output bits (the encoded symbols)
    fn encode_bit(&mut self, input: bool) -> Vec<bool>;

    /// Resets the encoder state to initial conditions.
    fn reset(&mut self);
}

/// Streaming decoder for convolutional codes.
///
/// A streaming decoder processes received symbols and maintains internal state
/// across multiple decode operations.
pub trait StreamingDecoder {
    /// Decodes received symbol(s) and potentially outputs decoded bit(s).
    ///
    /// # Arguments
    ///
    /// * `symbols` - The received symbols to decode
    ///
    /// # Returns
    ///
    /// Decoded bits (may be empty if more symbols are needed)
    fn decode_symbols(&mut self, symbols: &[bool]) -> Vec<bool>;

    /// Resets the decoder state to initial conditions.
    fn reset(&mut self);
}
