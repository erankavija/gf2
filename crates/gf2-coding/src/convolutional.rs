//! Convolutional codes: feedforward encoding and hard-decision Viterbi decoding.

use crate::traits::{StreamingDecoder, StreamingEncoder};

/// A feedforward convolutional encoder.
#[derive(Debug, Clone)]
pub struct ConvolutionalEncoder {
    /// Constraint length (number of shift register stages)
    constraint_length: usize,
    /// Generator polynomials (one per output)
    generators: Vec<u32>,
    state: u32,
}

impl ConvolutionalEncoder {
    /// Creates a rate-1/n encoder with one generator mask per output; bit 0 of a
    /// mask taps the newest input bit.
    pub fn new(constraint_length: usize, generators: Vec<u32>) -> Self {
        Self {
            constraint_length,
            generators,
            state: 0,
        }
    }

    /// The shift register contents, newest input bit in bit 0.
    pub fn state(&self) -> u32 {
        self.state
    }

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
        self.state = (self.state << 1) | (input as u32);

        let mask = (1u32 << self.constraint_length) - 1;
        self.state &= mask;

        let mut outputs = Vec::new();
        for &gen in &self.generators {
            let product = self.state & gen;
            let output = product.count_ones() % 2 == 1;
            outputs.push(output);
        }

        outputs
    }

    fn reset(&mut self) {
        self.state = 0;
    }
}

/// Hard-decision Viterbi decoder with Hamming branch metrics. Traceback starts
/// from state 0, so the encoded stream must be terminated with K - 1 zero
/// bits. Each step tests every state pair: O(4^K * n) per input bit for `n`
/// generators.
#[derive(Debug, Clone)]
pub struct ConvolutionalDecoder {
    constraint_length: usize,
    generators: Vec<u32>,
    num_states: usize,
    metrics: Vec<u32>,
    prev_metrics: Vec<u32>,
    /// Survivor paths: decisions[time][state] = (input_bit, prev_state)
    decisions: Vec<Vec<(bool, usize)>>,
}

impl ConvolutionalDecoder {
    /// `constraint_length` and `generators` must match the encoder's.
    ///
    /// # Panics
    ///
    /// Panics if constraint_length is 0 or > 31.
    pub fn new(constraint_length: usize, generators: Vec<u32>) -> Self {
        assert!(constraint_length > 0 && constraint_length <= 31);

        let num_states = 1 << (constraint_length - 1);
        let mut metrics = vec![u32::MAX / 2; num_states];
        metrics[0] = 0;

        Self {
            constraint_length,
            generators,
            num_states,
            metrics,
            prev_metrics: vec![u32::MAX / 2; num_states],
            decisions: Vec::new(),
        }
    }

    fn compute_output(&self, prev_state: usize, input_bit: bool) -> Vec<bool> {
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

    fn hamming_distance(a: &[bool], b: &[bool]) -> u32 {
        a.iter().zip(b).filter(|(x, y)| x != y).count() as u32
    }

    fn viterbi_step(&mut self, received: &[bool]) {
        std::mem::swap(&mut self.metrics, &mut self.prev_metrics);
        self.metrics.fill(u32::MAX / 2);

        let mut current_decisions = vec![(false, 0); self.num_states];

        for (next_state, decision) in current_decisions.iter_mut().enumerate() {
            let mut best_metric = u32::MAX / 2;
            let mut best_input = false;
            let mut best_prev = 0;

            for prev_state in 0..self.num_states {
                for input_bit in [false, true] {
                    let resulting_state = (prev_state << 1 | (input_bit as usize))
                        & ((1 << (self.constraint_length - 1)) - 1);

                    if resulting_state != next_state {
                        continue;
                    }

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

    fn traceback(&self) -> Vec<bool> {
        if self.decisions.is_empty() {
            return Vec::new();
        }

        let mut decoded = Vec::with_capacity(self.decisions.len());
        let mut state = 0usize; // End at state 0 (terminated)

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
    /// Returns the traceback over every symbol received since the last reset.
    ///
    /// # Panics
    ///
    /// Panics if `symbols.len()` is not a multiple of the number of generators.
    fn decode_symbols(&mut self, symbols: &[bool]) -> Vec<bool> {
        let n = self.generators.len();
        assert_eq!(symbols.len() % n, 0, "Symbols must be multiple of {}", n);

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

        let output = encoder.encode_bit(false);
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_convolutional_encoder_basic() {
        let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
        encoder.reset();

        let output = encoder.encode_bit(true);
        assert_eq!(output.len(), 2);

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

        assert_eq!(&decoded[..message.len()], &message[..]);
    }
}
