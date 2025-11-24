//! DVB-T2 BCH Verification Tests
//!
//! These tests verify BCH encoding and decoding against official DVB-T2 test vectors.
//! Tests are marked with #[ignore] and only run when test vectors are available.

mod test_vectors;

use gf2_coding::bch::{BchCode, BchDecoder, BchEncoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use rand::Rng;
use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

/// Verify BCH encoding: TP04 → TP05
#[test]
#[ignore]
fn test_bch_encoding_tp04_to_tp05() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = BchCode::dvb_t2(vectors.config.frame_size.to_bch(), vectors.config.code_rate);
    let encoder = BchEncoder::new(bch);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let mut successes = 0;
    let mut failures = 0;

    // Test all blocks in first frame
    for (block_idx, input_block) in tp04.frame(0).iter().enumerate() {
        let expected_output = &tp05.frame(0)[block_idx];

        // Encode
        let encoded = encoder.encode(&input_block.data);

        // Compare
        if encoded == expected_output.data {
            successes += 1;
        } else {
            failures += 1;
            eprintln!(
                "Frame 1, Block {}: MISMATCH (expected {} bits, got {})",
                block_idx + 1,
                expected_output.data.len(),
                encoded.len()
            );

            // Show first few bit differences
            let mut diff_count = 0;
            for i in 0..encoded.len().min(expected_output.data.len()) {
                if encoded.get(i) != expected_output.data.get(i) {
                    diff_count += 1;
                    if diff_count <= 5 {
                        eprintln!(
                            "  Bit {} differs: expected {}, got {}",
                            i,
                            expected_output.data.get(i) as u8,
                            encoded.get(i) as u8
                        );
                    }
                }
            }
            if diff_count > 5 {
                eprintln!("  ... and {} more differences", diff_count - 5);
            }
        }
    }

    println!(
        "BCH Encoding (Frame 1): {} successes, {} failures",
        successes, failures
    );
    assert_eq!(
        failures, 0,
        "BCH encoding validation failed on {} blocks",
        failures
    );
}

/// Verify BCH decoding: TP05 → TP04 (error-free)
#[test]
#[ignore]
fn test_bch_decoding_tp05_to_tp04_error_free() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = BchCode::dvb_t2(vectors.config.frame_size.to_bch(), vectors.config.code_rate);
    let decoder = BchDecoder::new(bch);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let mut successes = 0;
    let mut failures = 0;

    // Test all blocks in first frame
    for (block_idx, codeword) in tp05.frame(0).iter().enumerate() {
        let expected_message = &tp04.frame(0)[block_idx];

        // Decode
        let decoded = decoder.decode(&codeword.data);

        // Compare
        if decoded == expected_message.data {
            successes += 1;
        } else {
            failures += 1;
            eprintln!(
                "Frame 1, Block {}: MISMATCH (expected {} bits, got {})",
                block_idx + 1,
                expected_message.data.len(),
                decoded.len()
            );
        }
    }

    println!(
        "BCH Decoding (Frame 1, error-free): {} successes, {} failures",
        successes, failures
    );
    assert_eq!(
        failures, 0,
        "BCH decoding validation failed on {} blocks",
        failures
    );
}

/// Verify BCH error correction capability with injected errors
#[test]
#[ignore]
fn test_bch_error_correction() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = BchCode::dvb_t2(vectors.config.frame_size.to_bch(), vectors.config.code_rate);
    let decoder = BchDecoder::new(bch.clone());

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let mut rng = rand::thread_rng();

    // Test correction capability (t=12 for DVB-T2)
    let max_errors = bch.t();
    println!("Testing error correction up to t={} errors", max_errors);

    let num_test_blocks = 10.min(tp05.frame(0).len());
    let trials_per_block = 5;

    for num_errors in 1..=max_errors {
        let mut successes = 0;
        let mut failures = 0;

        for block_idx in 0..num_test_blocks {
            let codeword = &tp05.frame(0)[block_idx];
            let expected_message = &tp04.frame(0)[block_idx];

            for _trial in 0..trials_per_block {
                // Inject random errors
                let mut corrupted = codeword.data.clone();
                let mut error_positions = Vec::new();

                while error_positions.len() < num_errors {
                    let pos = rng.gen_range(0..corrupted.len());
                    if !error_positions.contains(&pos) {
                        error_positions.push(pos);
                        corrupted.set(pos, !corrupted.get(pos));
                    }
                }

                // Decode
                let decoded = decoder.decode(&corrupted);

                // Check if corrected
                if decoded == expected_message.data {
                    successes += 1;
                } else {
                    failures += 1;
                }
            }
        }

        let total = num_test_blocks * trials_per_block;
        println!(
            "  {} errors: {}/{} corrected ({:.1}%)",
            num_errors,
            successes,
            total,
            100.0 * successes as f64 / total as f64
        );

        assert_eq!(
            failures, 0,
            "BCH failed to correct {} errors in {} trials",
            num_errors, failures
        );
    }
}

/// Verify BCH codeword structure: systematic encoding
#[test]
#[ignore]
fn test_bch_systematic_property() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = BchCode::dvb_t2(vectors.config.frame_size.to_bch(), vectors.config.code_rate);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let k = bch.k();
    let n = bch.n();
    let parity_bits = n - k;

    println!(
        "Verifying systematic encoding: k={}, n={}, parity={}",
        k, n, parity_bits
    );

    // Check first few blocks
    for block_idx in 0..5.min(tp04.frame(0).len()) {
        let message = &tp04.frame(0)[block_idx];
        let codeword = &tp05.frame(0)[block_idx];

        assert_eq!(message.data.len(), k, "Message length mismatch");
        assert_eq!(codeword.data.len(), n, "Codeword length mismatch");

        // Systematic property: first k bits of codeword should equal message
        for i in 0..k {
            assert_eq!(
                codeword.data.get(i),
                message.data.get(i),
                "Systematic property violated at block {}, bit {}",
                block_idx + 1,
                i
            );
        }
    }

    println!("✓ Systematic encoding property verified");
}

/// Test BCH encoding on a sample of blocks to verify consistency
#[test]
#[ignore]
fn test_bch_encoding_sample() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = BchCode::dvb_t2(vectors.config.frame_size.to_bch(), vectors.config.code_rate);
    let encoder = BchEncoder::new(bch);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    println!("Testing sample blocks from each frame...");

    for frame_idx in 0..tp04.num_frames() {
        let frame_tp04 = tp04.frame(frame_idx);
        let frame_tp05 = tp05.frame(frame_idx);

        // Test first, middle, and last block of each frame
        let test_indices = vec![0, frame_tp04.len() / 2, frame_tp04.len() - 1];

        for &block_idx in &test_indices {
            let message = &frame_tp04[block_idx];
            let expected_cw = &frame_tp05[block_idx];
            let encoded = encoder.encode(&message.data);

            assert_eq!(
                encoded,
                expected_cw.data,
                "Frame {}, Block {} encoding mismatch",
                frame_idx + 1,
                block_idx + 1
            );
        }

        println!(
            "  Frame {}: {} sample blocks verified",
            frame_idx + 1,
            test_indices.len()
        );
    }

    println!("✓ All sample blocks match");
}
