//! Educational example: NASA rate-1/2, K=3 convolutional code with Viterbi decoding.
//!
//! # Convolutional Codes: A Comprehensive Tutorial
//!
//! This example demonstrates convolutional encoding and Viterbi decoding using
//! the industry-standard NASA rate-1/2, K=3 code (a simplified version of the
//! NASA/CCSDS K=7 standard used in space communications).
//!
//! ## Mathematical Foundation
//!
//! ### Encoder Structure
//!
//! A convolutional encoder is defined by:
//! - **Constraint length K**: Number of shift register stages (K=3 here)
//! - **Code rate 1/n**: Ratio of input bits to output bits (rate 1/2 means 2 outputs per input)
//! - **Generator polynomials G**: Define which register taps contribute to each output
//!
//! For our K=3, rate-1/2 encoder:
//! ```text
//! G_1 = 111_2 = 7_8  (octal 7: all three taps)
//! G_2 = 101_2 = 5_8  (octal 5: first and third taps)
//! ```
//!
//! ### Encoding Process
//!
//! The encoder maintains a 3-bit shift register `[s_2, s_1, s_0]`. For each input bit `u_t`:
//!
//! 1. Shift `u_t` into the register: `[s_2, s_1, s_0] <- [s_1, s_0, u_t]`
//! 2. Compute outputs:
//!    ```text
//!    v_1 = s_2 XOR s_1 XOR s_0  (G_1 = 111)
//!    v_2 = s_2 XOR s_0          (G_2 = 101)
//!    ```
//! 3. Output the symbol `[v_1, v_2]`
//!
//! where XOR denotes addition in GF(2).
//!
//! ### State Transition Diagram
//!
//! With K=3, there are `2^(K-1) = 4` states, labeled by `[s_2, s_1]`:
//!
//! ```text
//! State 00: ──0/00──> 00    ──1/11──> 10
//! State 01: ──0/11──> 00    ──1/00──> 10
//! State 10: ──0/10──> 01    ──1/01──> 11
//! State 11: ──0/01──> 01    ──1/10──> 11
//!
//! Notation: input/output_1 output_2
//! ```
//!
//! ### Example Encoding Trace
//!
//! Input sequence: `1011`
//!
//! | Time | Input | State | Register | G₁ (111) | G₂ (101) | Output |
//! |------|-------|-------|----------|----------|----------|--------|
//! | 0    | -     | 00    | 000      | -        | -        | -      |
//! | 1    | 1     | 00→10 | 001      | 1        | 1        | 11     |
//! | 2    | 0     | 10→01 | 010      | 1        | 0        | 10     |
//! | 3    | 1     | 01→10 | 101      | 0        | 0        | 00     |
//! | 4    | 1     | 10→11 | 111      | 1        | 0        | 10     |
//!
//! Encoded output: `11 10 00 10` (8 bits from 4 input bits)
//!
//! ### Viterbi Decoding Algorithm
//!
//! The Viterbi algorithm finds the most likely transmitted sequence by:
//!
//! 1. **Initialization**: Start with metric 0 for state 00, infinity for all other states
//! 2. **Forward Pass**: For each received symbol:
//!    - For each state, compute metrics for both possible transitions
//!    - Keep the path with lower accumulated Hamming distance
//!    - Store which input bit led to this state (survivor path)
//! 3. **Traceback**: Starting from state 00 at the end, follow survivor paths backward
//!
//! #### Branch Metric Computation
//!
//! For transition from state s' to state s with input bit u:
//! ```text
//! branch_metric = d_H(received_symbol, expected_output(s', u))
//! ```
//!
//! where `d_H` is Hamming distance.
//!
//! #### Path Metric Update
//!
//! ```text
//! M[s, t] = min_{s'} { M[s', t-1] + branch_metric(s' -> s) }
//! ```
//!
//! ### Error Correction Capability
//!
//! The free distance of this code is `d_free = 5`, meaning:
//! - Guaranteed correction of `t = floor((d_free - 1)/2) = 2` errors (per constraint length)
//! - Can detect up to `d_free - 1 = 4` errors
//!
//! Performance improves with longer constraint lengths:
//! - K=3: `d_free = 5`
//! - K=5: `d_free = 7`
//! - K=7: `d_free = 10` (NASA standard)
//!
//! ## Practical Considerations
//!
//! ### Termination
//!
//! To ensure the encoder returns to state 00 (required for optimal Viterbi decoding),
//! append `K-1 = 2` zero bits to the message. This is called "tail-biting" or "termination".
//!
//! ### Applications
//!
//! Convolutional codes are used in:
//! - **Deep space communications** (NASA/ESA spacecraft)
//! - **Satellite communications** (DVB-S, GPS)
//! - **Mobile telephony** (GSM, CDMA)
//! - **WiFi** (802.11, often with puncturing)
//!
//! Modern systems often use turbo codes or LDPC codes, but convolutional codes
//! remain important for low-latency applications and as building blocks for
//! turbo codes.

use gf2_coding::traits::{StreamingDecoder, StreamingEncoder};
use gf2_coding::{ConvolutionalDecoder, ConvolutionalEncoder};

fn main() {
    // Create encoder and decoder with generators [111, 101] (octal [7, 5])
    let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
    let mut decoder = ConvolutionalDecoder::new(3, vec![0b111, 0b101]);

    // Example message
    let message = vec![true, false, true, true];

    // Encode with termination
    let codeword = encode_with_trace(&mut encoder, &message);

    // Decode without errors
    let _decoded_clean = decode_message(&mut decoder, &codeword, &message);

    // Test error correction with 1, 2, and 3 errors
    test_error_correction(&mut decoder, &codeword, &message);

    print_performance_summary();
}

fn encode_with_trace(encoder: &mut ConvolutionalEncoder, message: &[bool]) -> Vec<bool> {
    encoder.reset();
    let mut codeword = Vec::new();

    print_encoder_header(encoder);
    print_message_info(message);
    print_encoding_table_header();

    // Encode message bits
    for (t, &bit) in message.iter().enumerate() {
        let old_state = encoder.state();
        let output = encoder.encode_bit(bit);
        let new_state = encoder.state();
        codeword.extend(output.iter());

        print_encoding_step(t + 1, bit, old_state, new_state, &output);
    }

    // Termination: Add K-1 zero bits
    print_encoding_separator();
    for t in 0..(encoder.constraint_length() - 1) {
        let old_state = encoder.state();
        let output = encoder.encode_bit(false);
        let new_state = encoder.state();
        codeword.extend(output.iter());

        print_encoding_step(message.len() + t + 1, false, old_state, new_state, &output);
    }

    print_encoding_table_footer();
    print_codeword_info(&codeword, message.len());

    codeword
}

fn decode_message(
    decoder: &mut ConvolutionalDecoder,
    codeword: &[bool],
    message: &[bool],
) -> Vec<bool> {
    decoder.reset();
    let decoded = decoder.decode_symbols(codeword);

    print_decoding_result(
        "without errors",
        codeword,
        &decoded[..message.len()],
        message,
    );

    decoded
}

fn test_error_correction(decoder: &mut ConvolutionalDecoder, codeword: &[bool], message: &[bool]) {
    for num_errors in 1..=3 {
        let error_positions: Vec<usize> = match num_errors {
            1 => vec![1],
            2 => vec![1, 5],
            _ => vec![1, 5, 9],
        };

        let corrupted = introduce_errors(codeword, &error_positions);
        decoder.reset();
        let decoded = decoder.decode_symbols(&corrupted);

        print_error_correction_result(
            &corrupted,
            &decoded[..message.len()],
            message,
            num_errors,
            &error_positions,
        );
    }
}

fn introduce_errors(codeword: &[bool], positions: &[usize]) -> Vec<bool> {
    let mut corrupted = codeword.to_vec();
    for &pos in positions {
        if pos < corrupted.len() {
            corrupted[pos] = !corrupted[pos];
        }
    }
    corrupted
}

fn bits_to_string(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

// === Printing Functions ===

fn print_encoder_header(encoder: &ConvolutionalEncoder) {
    println!("=== NASA Rate-1/2, K=3 Convolutional Code Example ===\n");
    println!("Encoder parameters:");
    println!("  Constraint length K = {}", encoder.constraint_length());
    println!("  Code rate = 1/{}", encoder.rate().1);
    println!("  Generator polynomials:");
    println!("    G_1 = 111_2 = 7_8 (octal)");
    println!("    G_2 = 101_2 = 5_8 (octal)");
    println!();
}

fn print_message_info(message: &[bool]) {
    println!("Message to encode: {:?}", bits_to_string(message));
    println!("Message length: {} bits", message.len());
    println!();
}

fn print_encoding_table_header() {
    println!("Encoding trace:");
    println!("┌──────┬───────┬──────────┬──────────────┬────────┐");
    println!("│ Time │ Input │  State   │   Register   │ Output │");
    println!("├──────┼───────┼──────────┼──────────────┼────────┤");
}

fn print_encoding_step(time: usize, input: bool, old_state: u32, new_state: u32, output: &[bool]) {
    println!(
        "│  {:2}  │   {}   │ {:02b} → {:02b} │ {:03b} ({:3}) │   {}{}   │",
        time,
        if input { '1' } else { '0' },
        old_state >> 1,
        new_state >> 1,
        new_state,
        new_state,
        if output[0] { '1' } else { '0' },
        if output[1] { '1' } else { '0' },
    );
}

fn print_encoding_separator() {
    println!("├──────┼───────┼──────────┼──────────────┼────────┤");
}

fn print_encoding_table_footer() {
    println!("└──────┴───────┴──────────┴──────────────┴────────┘");
    println!();
}

fn print_codeword_info(codeword: &[bool], message_len: usize) {
    println!("Encoded codeword: {}", bits_to_string(codeword));
    println!(
        "Codeword length: {} bits (rate = {}/{})",
        codeword.len(),
        message_len,
        codeword.len(),
    );
    println!();
}

fn print_decoding_result(description: &str, received: &[bool], decoded: &[bool], message: &[bool]) {
    println!("Decoding {}:", description);
    println!("  Received: {}", bits_to_string(received));
    println!(
        "  Decoded:  {} (first {} bits)",
        bits_to_string(decoded),
        message.len()
    );
    println!(
        "  Match: {}",
        if decoded == message {
            "✓ Correct!"
        } else {
            "✗ Error"
        }
    );
    println!();
}

fn print_error_correction_result(
    corrupted: &[bool],
    decoded: &[bool],
    message: &[bool],
    num_errors: usize,
    error_positions: &[usize],
) {
    let num_correct = decoded.iter().zip(message).filter(|(a, b)| a == b).count();

    println!(
        "Decoding with {} error(s) at position(s) {:?}:",
        num_errors, error_positions
    );
    println!("  Received: {}", bits_to_string(corrupted));
    println!(
        "  Decoded:  {} (first {} bits)",
        bits_to_string(decoded),
        message.len()
    );
    println!(
        "  Correct bits: {}/{} ({:.1}%)",
        num_correct,
        message.len(),
        100.0 * num_correct as f64 / message.len() as f64,
    );
    println!(
        "  Status: {}",
        if num_correct == message.len() {
            "✓ All errors corrected!"
        } else {
            "⚠ Some errors remain"
        }
    );
    println!();
}

fn print_performance_summary() {
    println!("=== Performance Characteristics ===\n");
    println!("Free distance d_free ≈ 5 for K=3, rate-1/2");
    println!("Guaranteed error correction: t = floor((d_free-1)/2) = 2 errors");
    println!("Error detection: up to d_free - 1 = 4 errors");
    println!();
    println!("For comparison with industry standards:");
    println!("  - NASA K=3: d_free ≈ 5  (educational)");
    println!("  - GSM K=5:  d_free ≈ 7  (mobile phones)");
    println!("  - NASA K=7: d_free = 10 (deep space, GPS)");
    println!();
    println!("=== Further Reading ===\n");
    println!("• Viterbi, A. J. (1967). \"Error bounds for convolutional codes\"");
    println!("• Lin, S., & Costello, D. J. (2004). \"Error Control Coding\" (Chapter 11-12)");
    println!("• NASA/CCSDS TM Synchronization and Channel Coding (CCSDS 131.0-B-3)");
}
