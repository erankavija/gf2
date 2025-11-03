//! Convolutional codes (skeleton for future implementation).
//!
//! This module provides skeleton types for convolutional codes, which process
//! bits in a streaming fashion while maintaining internal state.

use crate::coding::traits::{StreamingEncoder, StreamingDecoder};

/// A convolutional encoder (skeleton).
///
/// Convolutional encoders maintain a shift register state and produce
/// output symbols based on the current input and state.
///
/// # Note
///
/// This is a skeleton implementation. Full convolutional encoding will be
/// implemented in a future update.
///
/// # Examples
///
/// ```
/// use gf2::coding::ConvolutionalEncoder;
/// use gf2::coding::traits::StreamingEncoder;
///
/// let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
/// encoder.reset();
/// let _output = encoder.encode_bit(true);
/// ```
#[derive(Debug, Clone)]
pub struct ConvolutionalEncoder {
    /// Constraint length (number of shift register stages)
    constraint_length: usize,
    /// Generator polynomials (one per output)
    generators: Vec<u32>,
    /// Current state of the shift register
    state: u32,
}

impl ConvolutionalEncoder {
    /// Creates a new convolutional encoder.
    ///
    /// # Arguments
    ///
    /// * `constraint_length` - The number of shift register stages (K)
    /// * `generators` - Generator polynomials for each output
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::coding::ConvolutionalEncoder;
    ///
    /// // Create a rate-1/2, K=3 encoder
    /// let encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
    /// ```
    pub fn new(constraint_length: usize, generators: Vec<u32>) -> Self {
        Self {
            constraint_length,
            generators,
            state: 0,
        }
    }

    /// Returns the constraint length.
    pub fn constraint_length(&self) -> usize {
        self.constraint_length
    }

    /// Returns the code rate as (1, n) where n is the number of generators.
    pub fn rate(&self) -> (usize, usize) {
        (1, self.generators.len())
    }
}

impl StreamingEncoder for ConvolutionalEncoder {
    fn encode_bit(&mut self, input: bool) -> Vec<bool> {
        // Shift input into the register
        self.state = (self.state << 1) | (input as u32);
        
        // Keep only constraint_length bits
        let mask = (1u32 << self.constraint_length) - 1;
        self.state &= mask;

        // Generate outputs by XORing the state with each generator polynomial
        let mut outputs = Vec::new();
        for &gen in &self.generators {
            let product = self.state & gen;
            // XOR all bits in the product to get output bit
            let output = product.count_ones() % 2 == 1;
            outputs.push(output);
        }

        outputs
    }

    fn reset(&mut self) {
        self.state = 0;
    }
}

/// A convolutional decoder (skeleton/stub).
///
/// Convolutional decoders typically use algorithms like Viterbi decoding
/// to find the most likely transmitted sequence.
///
/// # Note
///
/// This is a stub implementation. Full decoding functionality (e.g., Viterbi algorithm)
/// will be implemented in a future update.
///
/// # Examples
///
/// ```
/// use gf2::coding::ConvolutionalDecoder;
/// use gf2::coding::traits::StreamingDecoder;
///
/// let mut decoder = ConvolutionalDecoder::new();
/// decoder.reset();
/// ```
#[derive(Debug, Clone)]
pub struct ConvolutionalDecoder {
    // Placeholder fields for future implementation
    _placeholder: (),
}

impl ConvolutionalDecoder {
    /// Creates a new convolutional decoder.
    ///
    /// # Note
    ///
    /// This is a stub. Parameters for encoder structure will be added
    /// in future implementations.
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl Default for ConvolutionalDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingDecoder for ConvolutionalDecoder {
    fn decode_symbols(&mut self, _symbols: &[bool]) -> Vec<bool> {
        // Stub: returns empty vector
        // Future implementation will use Viterbi or other decoding algorithms
        Vec::new()
    }

    fn reset(&mut self) {
        // Stub: nothing to reset yet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convolutional_encoder_creation() {
        let encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
        assert_eq!(encoder.constraint_length(), 3);
        assert_eq!(encoder.rate(), (1, 2));
    }

    #[test]
    fn test_convolutional_encoder_reset() {
        let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
        encoder.encode_bit(true);
        encoder.encode_bit(true);
        encoder.reset();
        
        // After reset, state should be 0
        let output = encoder.encode_bit(false);
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_convolutional_encoder_basic() {
        let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
        encoder.reset();
        
        // Encode a single bit
        let output = encoder.encode_bit(true);
        assert_eq!(output.len(), 2);
        
        // Both generators should produce output
        // With state = 001 (binary), gen1 = 111, gen2 = 101
        // output1 = 001 & 111 = 001 -> XOR = 1
        // output2 = 001 & 101 = 001 -> XOR = 1
        assert_eq!(output[0], true);
        assert_eq!(output[1], true);
    }

    #[test]
    fn test_convolutional_decoder_creation() {
        let decoder = ConvolutionalDecoder::new();
        // Just verify it can be created
        let _ = decoder;
    }

    #[test]
    fn test_convolutional_decoder_stub() {
        let mut decoder = ConvolutionalDecoder::new();
        let result = decoder.decode_symbols(&[true, false, true]);
        // Stub returns empty vector
        assert_eq!(result.len(), 0);
    }
}
