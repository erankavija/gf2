//! Traits for error-correcting codes.
//!
//! This module defines the core traits for encoding and decoding operations
//! in error-correcting codes, supporting both block codes and streaming codes.

use crate::llr::Llr;
use gf2_core::BitVec;

/// Result of a soft-decision decoding operation.
///
/// Contains the decoded bits along with metadata about the decoding process,
/// particularly useful for iterative decoders like LDPC and turbo codes.
#[derive(Debug, Clone, PartialEq)]
pub struct DecoderResult {
    /// The decoded message bits
    pub decoded_bits: BitVec,

    /// Number of iterations performed (for iterative decoders)
    pub iterations: usize,

    /// Whether the decoder converged to a valid codeword
    pub converged: bool,

    /// Whether the syndrome check passed (for linear codes)
    pub syndrome_check_passed: bool,
}

impl DecoderResult {
    /// Creates a new decoder result.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::traits::DecoderResult;
    /// use gf2_core::BitVec;
    ///
    /// let decoded = BitVec::from_bytes_le(&[0b1011]);
    /// let result = DecoderResult::new(decoded, 5, true, true);
    /// assert_eq!(result.iterations, 5);
    /// assert!(result.converged);
    /// ```
    pub fn new(
        decoded_bits: BitVec,
        iterations: usize,
        converged: bool,
        syndrome_check_passed: bool,
    ) -> Self {
        Self {
            decoded_bits,
            iterations,
            converged,
            syndrome_check_passed,
        }
    }

    /// Creates a result for a successful single-shot decode (non-iterative).
    pub fn success(decoded_bits: BitVec) -> Self {
        Self {
            decoded_bits,
            iterations: 1,
            converged: true,
            syndrome_check_passed: true,
        }
    }

    /// Creates a result for a failed decode.
    pub fn failure(decoded_bits: BitVec, iterations: usize) -> Self {
        Self {
            decoded_bits,
            iterations,
            converged: false,
            syndrome_check_passed: false,
        }
    }
}

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

/// Soft-decision decoder for block codes.
///
/// A soft-decision decoder uses log-likelihood ratios (LLRs) to make better
/// decoding decisions than hard-decision decoders. This trait supports both
/// single-shot and iterative decoding algorithms.
///
/// # LLR Convention
///
/// LLR values follow the convention:
/// - Positive LLR → bit is more likely 0
/// - Negative LLR → bit is more likely 1
/// - Magnitude represents confidence
pub trait SoftDecoder {
    /// Returns the number of message bits (dimension).
    fn k(&self) -> usize;

    /// Returns the number of codeword bits (length).
    fn n(&self) -> usize;

    /// Decodes using soft information (LLRs).
    ///
    /// This is the primary decoding method for soft-decision decoders.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Log-likelihood ratios for each codeword bit position
    ///
    /// # Returns
    ///
    /// Decoded message bits
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n()`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::traits::SoftDecoder;
    ///
    /// let llrs: Vec<Llr> = received_symbols.iter()
    ///     .map(|&s| Llr::from_bpsk_symbol(s, noise_variance))
    ///     .collect();
    /// let decoded = decoder.decode_soft(&llrs);
    /// ```
    fn decode_soft(&self, llrs: &[Llr]) -> BitVec;

    /// Decodes and returns detailed result information.
    ///
    /// Similar to `decode_soft` but returns additional metadata useful for
    /// analysis and debugging.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Log-likelihood ratios for each codeword bit position
    ///
    /// # Returns
    ///
    /// A `DecoderResult` containing decoded bits and metadata
    fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
        let decoded = self.decode_soft(llrs);
        DecoderResult::success(decoded)
    }
}

/// Iterative soft-decision decoder for LDPC and turbo codes.
///
/// Extends `SoftDecoder` with iteration control and early stopping criteria.
/// Iterative decoders repeatedly refine LLR estimates until convergence or
/// a maximum iteration count is reached.
///
/// # Typical Usage Pattern
///
/// ```ignore
/// let mut decoder = LdpcDecoder::new(code);
/// let result = decoder.decode_iterative(&channel_llrs, 50); // max 50 iterations
///
/// if result.converged {
///     println!("Converged in {} iterations", result.iterations);
/// } else {
///     println!("Failed to converge after {} iterations", result.iterations);
/// }
/// ```
pub trait IterativeSoftDecoder: SoftDecoder {
    /// Decodes with iteration control.
    ///
    /// Performs iterative belief propagation or similar algorithm until
    /// convergence or maximum iterations reached.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Initial log-likelihood ratios from channel
    /// * `max_iterations` - Maximum number of iterations to perform
    ///
    /// # Returns
    ///
    /// A `DecoderResult` containing decoded bits and convergence information
    ///
    /// # Early Stopping
    ///
    /// The decoder should stop early if:
    /// - Syndrome check passes (for linear codes)
    /// - LLR updates fall below threshold (converged)
    /// - Maximum iterations reached
    fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult;

    /// Returns the number of iterations used in the last decode.
    ///
    /// Useful for tracking decoder performance without full `DecoderResult`.
    fn last_iteration_count(&self) -> usize;

    /// Resets internal decoder state.
    ///
    /// Should be called between decoding different codewords to ensure
    /// no state leaks between frames.
    fn reset(&mut self);
}

/// Soft-decision decoder for block codes (deprecated - use `SoftDecoder`).
///
/// This trait is deprecated in favor of the more comprehensive `SoftDecoder` trait.
#[deprecated(since = "0.2.0", note = "Use SoftDecoder trait instead")]
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
    /// Deprecated: Use `SoftDecoder::decode_soft()` instead.
    fn decode_soft(&self, soft_bits: &[f64]) -> BitVec;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_result_new() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result = DecoderResult::new(bits.clone(), 10, true, true);

        assert_eq!(result.decoded_bits, bits);
        assert_eq!(result.iterations, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
    }

    #[test]
    fn test_decoder_result_success() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result = DecoderResult::success(bits.clone());

        assert_eq!(result.decoded_bits, bits);
        assert_eq!(result.iterations, 1);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
    }

    #[test]
    fn test_decoder_result_failure() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result = DecoderResult::failure(bits.clone(), 50);

        assert_eq!(result.decoded_bits, bits);
        assert_eq!(result.iterations, 50);
        assert!(!result.converged);
        assert!(!result.syndrome_check_passed);
    }

    #[test]
    fn test_decoder_result_clone() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result1 = DecoderResult::new(bits.clone(), 5, true, false);
        let result2 = result1.clone();

        assert_eq!(result1, result2);
    }

    // Mock implementations for testing trait contracts

    struct MockSoftDecoder {
        k: usize,
        n: usize,
    }

    impl SoftDecoder for MockSoftDecoder {
        fn k(&self) -> usize {
            self.k
        }

        fn n(&self) -> usize {
            self.n
        }

        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), self.n);
            // Simple hard decision for testing
            let mut result = BitVec::new();
            for &llr in llrs.iter().take(self.k) {
                result.push_bit(llr.hard_decision());
            }
            result
        }
    }

    struct MockIterativeDecoder {
        k: usize,
        n: usize,
        last_iterations: usize,
    }

    impl SoftDecoder for MockIterativeDecoder {
        fn k(&self) -> usize {
            self.k
        }

        fn n(&self) -> usize {
            self.n
        }

        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), self.n);
            let mut result = BitVec::new();
            for &llr in llrs.iter().take(self.k) {
                result.push_bit(llr.hard_decision());
            }
            result
        }
    }

    impl IterativeSoftDecoder for MockIterativeDecoder {
        fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
            assert_eq!(llrs.len(), self.n);

            // Simulate convergence after 5 iterations
            let iterations = max_iterations.min(5);
            self.last_iterations = iterations;

            let decoded = self.decode_soft(llrs);
            let converged = iterations < max_iterations;

            DecoderResult::new(decoded, iterations, converged, converged)
        }

        fn last_iteration_count(&self) -> usize {
            self.last_iterations
        }

        fn reset(&mut self) {
            self.last_iterations = 0;
        }
    }

    #[test]
    fn test_soft_decoder_trait() {
        let decoder = MockSoftDecoder { k: 4, n: 7 };

        assert_eq!(decoder.k(), 4);
        assert_eq!(decoder.n(), 7);

        let llrs = vec![
            Llr::new(3.0),
            Llr::new(-2.0),
            Llr::new(1.0),
            Llr::new(-0.5),
            Llr::new(2.0),
            Llr::new(1.5),
            Llr::new(-1.0),
        ];

        let decoded = decoder.decode_soft(&llrs);
        assert_eq!(decoded.len(), 4);

        // Check hard decisions
        assert!(!decoded.get(0)); // 3.0 → 0
        assert!(decoded.get(1)); // -2.0 → 1
        assert!(!decoded.get(2)); // 1.0 → 0
        assert!(decoded.get(3)); // -0.5 → 1
    }

    #[test]
    fn test_soft_decoder_with_result() {
        let decoder = MockSoftDecoder { k: 4, n: 7 };

        let llrs = vec![
            Llr::new(3.0),
            Llr::new(-2.0),
            Llr::new(1.0),
            Llr::new(-0.5),
            Llr::new(2.0),
            Llr::new(1.5),
            Llr::new(-1.0),
        ];

        let result = decoder.decode_soft_with_result(&llrs);

        assert_eq!(result.decoded_bits.len(), 4);
        assert_eq!(result.iterations, 1);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
    }

    #[test]
    fn test_iterative_decoder_converges() {
        let mut decoder = MockIterativeDecoder {
            k: 4,
            n: 7,
            last_iterations: 0,
        };

        let llrs = vec![Llr::new(1.0); 7];

        let result = decoder.decode_iterative(&llrs, 50);

        assert_eq!(result.iterations, 5); // Converges at 5
        assert!(result.converged);
        assert_eq!(decoder.last_iteration_count(), 5);
    }

    #[test]
    fn test_iterative_decoder_max_iterations() {
        let mut decoder = MockIterativeDecoder {
            k: 4,
            n: 7,
            last_iterations: 0,
        };

        let llrs = vec![Llr::new(1.0); 7];

        let result = decoder.decode_iterative(&llrs, 3); // Less than convergence point

        assert_eq!(result.iterations, 3);
        assert!(!result.converged); // Didn't converge
        assert_eq!(decoder.last_iteration_count(), 3);
    }

    #[test]
    fn test_iterative_decoder_reset() {
        let mut decoder = MockIterativeDecoder {
            k: 4,
            n: 7,
            last_iterations: 0,
        };

        let llrs = vec![Llr::new(1.0); 7];
        decoder.decode_iterative(&llrs, 10);
        assert_eq!(decoder.last_iteration_count(), 5);

        decoder.reset();
        assert_eq!(decoder.last_iteration_count(), 0);
    }

    #[test]
    #[should_panic(expected = "left == right")]
    fn test_soft_decoder_wrong_length_panics() {
        let decoder = MockSoftDecoder { k: 4, n: 7 };
        let llrs = vec![Llr::new(1.0); 5]; // Wrong length
        decoder.decode_soft(&llrs);
    }
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
