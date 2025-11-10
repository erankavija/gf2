//! Convolutional codes (skeleton for future implementation).
//!
//! This module provides skeleton types for convolutional codes, which process
//! bits in a streaming fashion while maintaining internal state.

use crate::traits::{StreamingDecoder, StreamingEncoder};

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
/// use gf2_coding::ConvolutionalEncoder;
/// use gf2_coding::traits::StreamingEncoder;
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
    /// use gf2_coding::ConvolutionalEncoder;
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

    /// Returns the current encoder state.
    pub fn state(&self) -> u32 {
        self.state
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

/// A Viterbi decoder for convolutional codes.
///
/// Implements the Viterbi algorithm for maximum-likelihood decoding of convolutional codes.
/// Uses hard-decision decoding with Hamming distance metrics.
///
/// # Examples
///
/// ```
/// use gf2_coding::{ConvolutionalEncoder, ConvolutionalDecoder};
/// use gf2_coding::traits::{StreamingEncoder, StreamingDecoder};
///
/// let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
/// let mut decoder = ConvolutionalDecoder::new(3, vec![0b111, 0b101]);
///
/// encoder.reset();
/// decoder.reset();
///
/// let message = vec![true, false, true];
/// let mut codeword = Vec::new();
/// for &bit in &message {
///     codeword.extend(encoder.encode_bit(bit));
/// }
///
/// // Terminate
/// for _ in 0..2 {
///     codeword.extend(encoder.encode_bit(false));
/// }
///
/// let decoded = decoder.decode_symbols(&codeword);
/// // First 3 bits should match
/// assert_eq!(&decoded[..3], &message[..]);
/// ```
#[derive(Debug, Clone)]
pub struct ConvolutionalDecoder {
    constraint_length: usize,
    generators: Vec<u32>,
    num_states: usize,
    /// Current path metrics
    metrics: Vec<u32>,
    /// Previous path metrics
    prev_metrics: Vec<u32>,
    /// Survivor paths: decisions[time][state] = (input_bit, prev_state)
    decisions: Vec<Vec<(bool, usize)>>,
}

impl ConvolutionalDecoder {
    /// Creates a new Viterbi decoder.
    ///
    /// # Arguments
    ///
    /// * `constraint_length` - Must match encoder's K
    /// * `generators` - Must match encoder's generator polynomials
    ///
    /// # Panics
    ///
    /// Panics if constraint_length is 0 or > 31.
    pub fn new(constraint_length: usize, generators: Vec<u32>) -> Self {
        assert!(constraint_length > 0 && constraint_length <= 31);

        let num_states = 1 << (constraint_length - 1);
        let mut metrics = vec![u32::MAX / 2; num_states];
        metrics[0] = 0; // Start at state 0

        Self {
            constraint_length,
            generators,
            num_states,
            metrics,
            prev_metrics: vec![u32::MAX / 2; num_states],
            decisions: Vec::new(),
        }
    }

    /// Computes output for a transition from prev_state with input_bit.
    fn compute_output(&self, prev_state: usize, input_bit: bool) -> Vec<bool> {
        // Full register after shifting in input_bit
        let full_state = (prev_state << 1) | (input_bit as usize);
        let masked_state = full_state & ((1 << self.constraint_length) - 1);

        self.generators
            .iter()
            .map(|&gen| {
                let product = masked_state & (gen as usize);
                product.count_ones() % 2 == 1
            })
            .collect()
    }

    /// Hamming distance between two bit vectors.
    fn hamming_distance(a: &[bool], b: &[bool]) -> u32 {
        a.iter().zip(b).filter(|(x, y)| x != y).count() as u32
    }

    /// One step of Viterbi forward pass.
    fn viterbi_step(&mut self, received: &[bool]) {
        std::mem::swap(&mut self.metrics, &mut self.prev_metrics);
        self.metrics.fill(u32::MAX / 2);

        let mut current_decisions = vec![(false, 0); self.num_states];

        // For each possible next state
        for (next_state, decision) in current_decisions.iter_mut().enumerate() {
            let mut best_metric = u32::MAX / 2;
            let mut best_input = false;
            let mut best_prev = 0;

            // Enumerate all possible (prev_state, input) pairs that lead to next_state
            for prev_state in 0..self.num_states {
                for input_bit in [false, true] {
                    // Check if this transition leads to next_state
                    let resulting_state = (prev_state << 1 | (input_bit as usize))
                        & ((1 << (self.constraint_length - 1)) - 1);

                    if resulting_state != next_state {
                        continue;
                    }

                    // Valid transition
                    let expected = self.compute_output(prev_state, input_bit);
                    let branch_metric = Self::hamming_distance(&expected, received);
                    let path_metric = self.prev_metrics[prev_state].saturating_add(branch_metric);

                    if path_metric < best_metric {
                        best_metric = path_metric;
                        best_input = input_bit;
                        best_prev = prev_state;
                    }
                }
            }

            self.metrics[next_state] = best_metric;
            *decision = (best_input, best_prev);
        }

        self.decisions.push(current_decisions);
    }

    /// Traceback to recover decoded sequence.
    fn traceback(&self) -> Vec<bool> {
        if self.decisions.is_empty() {
            return Vec::new();
        }

        let mut decoded = Vec::with_capacity(self.decisions.len());
        let mut state = 0usize; // End at state 0 (terminated)

        // Traceback from end to start
        for t in (0..self.decisions.len()).rev() {
            let (input_bit, prev_state) = self.decisions[t][state];
            decoded.push(input_bit);
            state = prev_state;
        }

        decoded.reverse();
        decoded
    }
}

impl Default for ConvolutionalDecoder {
    fn default() -> Self {
        Self::new(3, vec![0b111, 0b101])
    }
}

impl StreamingDecoder for ConvolutionalDecoder {
    fn decode_symbols(&mut self, symbols: &[bool]) -> Vec<bool> {
        let n = self.generators.len();
        assert_eq!(symbols.len() % n, 0, "Symbols must be multiple of {}", n);

        // Process each n-bit chunk
        for chunk in symbols.chunks(n) {
            self.viterbi_step(chunk);
        }

        self.traceback()
    }

    fn reset(&mut self) {
        self.metrics.fill(u32::MAX / 2);
        self.metrics[0] = 0;
        self.prev_metrics.fill(u32::MAX / 2);
        self.decisions.clear();
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
        assert!(output[0]);
        assert!(output[1]);
    }

    #[test]
    fn test_convolutional_decoder_creation() {
        let decoder = ConvolutionalDecoder::new(3, vec![0b111, 0b101]);
        assert_eq!(decoder.num_states, 4);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
        let mut decoder = ConvolutionalDecoder::new(3, vec![0b111, 0b101]);

        encoder.reset();
        decoder.reset();

        let message = vec![true, false, true];
        let mut codeword = Vec::new();

        for &bit in &message {
            codeword.extend(encoder.encode_bit(bit));
        }

        // Terminate with K-1 zeros
        for _ in 0..2 {
            codeword.extend(encoder.encode_bit(false));
        }

        let decoded = decoder.decode_symbols(&codeword);

        // Should decode the message correctly
        assert_eq!(&decoded[..message.len()], &message[..]);
    }
}
