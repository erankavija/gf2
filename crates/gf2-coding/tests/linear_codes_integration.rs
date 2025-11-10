//! Integration tests for linear block codes.
//!
//! These tests verify the interaction between encoding, syndrome computation,
//! and decoding across different code types and parameters.

use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_core::BitVec;

#[test]
fn test_hamming_7_4_full_workflow() {
    let code = LinearBlockCode::hamming(3);
    let decoder = SyndromeTableDecoder::new(code);

    // Test multiple messages
    let test_cases = vec![
        vec![false, false, false, false],
        vec![true, true, true, true],
        vec![true, false, true, false],
        vec![false, true, false, true],
        vec![true, true, false, false],
    ];

    for msg_bits in test_cases {
        let mut msg = BitVec::new();
        for bit in msg_bits {
            msg.push_bit(bit);
        }

        // Encode
        let codeword = decoder.code().encode(&msg);
        assert_eq!(codeword.len(), 7);

        // Verify zero syndrome
        let syndrome = decoder.code().syndrome(&codeword).unwrap();
        assert_eq!(syndrome.count_ones(), 0);

        // Decode without error
        let decoded = decoder.decode(&codeword);
        assert_eq!(decoded, msg);

        // Test error correction at each position
        for err_pos in 0..7 {
            let mut corrupted = codeword.clone();
            corrupted.set(err_pos, !corrupted.get(err_pos));

            let corrected = decoder.decode(&corrupted);
            assert_eq!(
                corrected, msg,
                "Failed to correct error at position {}",
                err_pos
            );
        }
    }
}

#[test]
fn test_hamming_15_11_full_workflow() {
    let code = LinearBlockCode::hamming(4);
    let decoder = SyndromeTableDecoder::new(code);

    // Create a random-ish message
    let mut msg = BitVec::new();
    for i in 0..11 {
        msg.push_bit((i * 7 + 3) % 2 == 0);
    }

    let codeword = decoder.code().encode(&msg);
    assert_eq!(codeword.len(), 15);

    // Test error correction at strategic positions
    for err_pos in [0, 1, 7, 8, 14] {
        let mut corrupted = codeword.clone();
        corrupted.set(err_pos, !corrupted.get(err_pos));

        let decoded = decoder.decode(&corrupted);
        assert_eq!(
            decoded, msg,
            "Failed to correct error at position {}",
            err_pos
        );
    }
}

#[test]
fn test_hamming_31_26_encoding_correctness() {
    let code = LinearBlockCode::hamming(5);

    // Create message with pattern
    let mut msg = BitVec::new();
    for i in 0..26 {
        msg.push_bit(i % 3 == 0);
    }

    let codeword = code.encode(&msg);
    assert_eq!(codeword.len(), 31);

    // Verify it's a valid codeword (zero syndrome)
    let syndrome = code.syndrome(&codeword).unwrap();
    assert_eq!(syndrome.count_ones(), 0);

    // Verify systematic encoding
    let extracted = code.project_message(&codeword);
    assert_eq!(extracted, msg);
}

#[test]
fn test_multiple_errors_detection() {
    // Hamming codes can detect (but not always correct) 2 errors
    let code = LinearBlockCode::hamming(3);
    let decoder = SyndromeTableDecoder::new(code.clone());

    let mut msg = BitVec::new();
    for bit in [true, false, true, false] {
        msg.push_bit(bit);
    }

    let codeword = decoder.code().encode(&msg);

    // Introduce 2 errors at positions 1 and 3
    let mut corrupted = codeword.clone();
    corrupted.set(1, !corrupted.get(1));
    corrupted.set(3, !corrupted.get(3));

    // The syndrome should be non-zero (error detected)
    let syndrome = code.syndrome(&corrupted).unwrap();
    assert!(
        syndrome.count_ones() > 0,
        "Two errors should produce non-zero syndrome"
    );

    // Decoder will attempt correction, but result may be incorrect
    // We just verify it doesn't panic
    let _decoded = decoder.decode(&corrupted);
}

#[test]
fn test_code_linearity_property() {
    // XOR of two codewords is also a codeword
    let code = LinearBlockCode::hamming(3);

    let mut msg1 = BitVec::new();
    for bit in [true, false, true, false] {
        msg1.push_bit(bit);
    }

    let mut msg2 = BitVec::new();
    for bit in [false, true, true, false] {
        msg2.push_bit(bit);
    }

    let c1 = code.encode(&msg1);
    let c2 = code.encode(&msg2);

    let mut c_sum = c1.clone();
    c_sum.bit_xor_into(&c2);

    // The sum should have zero syndrome (valid codeword)
    let syndrome = code.syndrome(&c_sum).unwrap();
    assert_eq!(
        syndrome.count_ones(),
        0,
        "XOR of codewords should be a valid codeword"
    );
}

#[test]
fn test_zero_codeword_is_valid() {
    // All-zero message should encode to all-zero codeword
    for r in 2..=5 {
        let code = LinearBlockCode::hamming(r);

        let mut msg = BitVec::new();
        msg.resize(code.k(), false);

        let codeword = code.encode(&msg);

        assert_eq!(
            codeword.count_ones(),
            0,
            "All-zero message should produce all-zero codeword for r={}",
            r
        );

        let syndrome = code.syndrome(&codeword).unwrap();
        assert_eq!(
            syndrome.count_ones(),
            0,
            "All-zero codeword should have zero syndrome for r={}",
            r
        );
    }
}

#[test]
fn test_systematic_encoding_verification() {
    // Verify that Hamming codes use systematic encoding
    let code = LinearBlockCode::hamming(4);

    let mut msg = BitVec::new();
    for i in 0..11 {
        msg.push_bit(i % 2 == 1);
    }

    let codeword = code.encode(&msg);

    // Extract systematic positions and verify they match the message
    let extracted = code.project_message(&codeword);
    assert_eq!(
        extracted, msg,
        "Systematic encoding should preserve message bits at systematic positions"
    );
}

#[test]
fn test_parity_check_orthogonality() {
    // For systematic codes, G * H^T = 0
    for r in 2..=6 {
        let code = LinearBlockCode::hamming(r);

        if let Some(h) = code.parity_check() {
            let g = code.generator();
            let h_t = h.transpose();
            let product = g * &h_t;

            // Verify all entries are zero
            for row in 0..product.rows() {
                for col in 0..product.cols() {
                    assert!(
                        !product.get(row, col),
                        "G * H^T must be zero at ({}, {}) for Hamming(r={})",
                        row,
                        col,
                        r
                    );
                }
            }
        }
    }
}

#[test]
fn test_syndrome_uniqueness_for_single_errors() {
    // Each single-bit error should produce a unique syndrome
    let code = LinearBlockCode::hamming(3);

    let mut syndromes = std::collections::HashSet::new();

    for err_pos in 0..7 {
        let mut error_pattern = BitVec::new();
        error_pattern.resize(7, false);
        error_pattern.set(err_pos, true);

        let syndrome = code.syndrome(&error_pattern).unwrap();

        // Convert syndrome to a comparable form
        let syndrome_value: u64 = (0..syndrome.len())
            .map(|i| if syndrome.get(i) { 1u64 << i } else { 0 })
            .sum();

        assert!(
            syndromes.insert(syndrome_value),
            "Syndrome for error at position {} should be unique",
            err_pos
        );
    }

    assert_eq!(
        syndromes.len(),
        7,
        "Should have 7 unique non-zero syndromes for single-bit errors"
    );
}

#[test]
fn test_decoder_table_construction() {
    // Verify that syndrome table decoder properly constructs the lookup table
    let code = LinearBlockCode::hamming(3);
    let decoder = SyndromeTableDecoder::new(code);

    // The decoder should have entries for:
    // - Zero syndrome (no error)
    // - 7 single-bit error syndromes
    // Total: should have at least 8 entries (possibly more if syndromes collide)

    // We can't access the table directly, but we can verify behavior
    let mut msg = BitVec::new();
    for bit in [true, false, true, false] {
        msg.push_bit(bit);
    }

    let codeword = decoder.code().encode(&msg);

    // No error case
    let decoded = decoder.decode(&codeword);
    assert_eq!(decoded, msg);

    // Single error cases (all should be correctable)
    for err_pos in 0..7 {
        let mut corrupted = codeword.clone();
        corrupted.set(err_pos, !corrupted.get(err_pos));
        let corrected = decoder.decode(&corrupted);
        assert_eq!(
            corrected, msg,
            "Error at position {} should be corrected",
            err_pos
        );
    }
}

#[test]
fn test_word_boundary_handling() {
    // Test codes with parameters around word boundaries
    let code = LinearBlockCode::hamming(7); // n=127, k=120

    // Create message spanning multiple words
    let mut msg = BitVec::new();
    for i in 0..120 {
        msg.push_bit(i % 5 == 0);
    }

    let codeword = code.encode(&msg);
    assert_eq!(codeword.len(), 127);

    let syndrome = code.syndrome(&codeword).unwrap();
    assert_eq!(
        syndrome.count_ones(),
        0,
        "Valid codeword at word boundary should have zero syndrome"
    );

    let extracted = code.project_message(&codeword);
    assert_eq!(
        extracted, msg,
        "Should correctly extract message across word boundaries"
    );
}

#[test]
fn test_batch_encoding() {
    // Test encoding multiple messages in sequence
    let code = LinearBlockCode::hamming(3);

    let messages = vec![
        vec![true, false, true, false],
        vec![false, true, false, true],
        vec![true, true, false, false],
        vec![false, false, true, true],
    ];

    let mut codewords = Vec::new();

    for msg_bits in &messages {
        let mut msg = BitVec::new();
        for &bit in msg_bits {
            msg.push_bit(bit);
        }
        let codeword = code.encode(&msg);
        codewords.push(codeword);
    }

    // Verify all codewords are valid
    for codeword in &codewords {
        let syndrome = code.syndrome(codeword).unwrap();
        assert_eq!(syndrome.count_ones(), 0);
    }

    // Verify decoding
    let decoder = SyndromeTableDecoder::new(code.clone());
    for (i, codeword) in codewords.iter().enumerate() {
        let decoded = decoder.decode(codeword);
        let mut expected = BitVec::new();
        for &bit in &messages[i] {
            expected.push_bit(bit);
        }
        assert_eq!(decoded, expected);
    }
}
