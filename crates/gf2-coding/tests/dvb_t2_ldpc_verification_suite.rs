//! DVB-T2 LDPC Verification Test Suite
//!
//! Comprehensive validation of LDPC encoding and decoding against official DVB-T2
//! test vectors, following the same approach as BCH verification.
//!
//! Test structure:
//! - TP05 (BCH output) → TP06 (LDPC output) encoding validation
//! - TP06 → TP05 decoding validation (error-free)
//! - TP06 + errors → TP05 decoding (error correction)
//! - Systematic encoding property verification
//! - Multi-frame consistency checks
//!
//! Prerequisites:
//! - DVB-T2 test vectors at $DVB_TEST_VECTORS_PATH or ~/dvb_test_vectors
//! - Pre-computed LDPC cache at data/ldpc/dvb_t2/ (optional but recommended)
//!
//! Run with: cargo test --test dvb_t2_ldpc_verification_suite -- --ignored --nocapture

mod test_vectors;

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_coding::CodeRate;
use rand::Rng;
use std::path::PathBuf;
use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

/// Helper: Load cache from standard location if available
fn try_load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        eprintln!("Note: No pre-computed cache found at {:?}", cache_dir);
        eprintln!("      Encoding will compute generator matrices on first use (slow)");
        eprintln!("      Run: cargo run --release --bin generate_ldpc_cache");
        None
    }
}

/// Helper: Create encoder with optional cache
fn create_encoder(code: LdpcCode, cache: Option<&EncodingCache>) -> LdpcEncoder {
    match cache {
        Some(c) => {
            eprintln!("Creating encoder with cache...");
            LdpcEncoder::with_cache(code, c)
        }
        None => {
            eprintln!("Creating encoder without cache (this may take 2-10 seconds)...");
            LdpcEncoder::new(code)
        }
    }
}

/// Test 1: Verify LDPC encoding TP05 → TP06
///
/// Validates that systematic LDPC encoding produces exact match with
/// reference test vectors.
#[test]
#[ignore]
fn test_ldpc_encoding_tp05_to_tp06() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let cache = try_load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let encoder = create_encoder(code, cache.as_ref());

    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    let mut successes = 0;
    let mut failures = 0;

    println!("Testing LDPC encoding on Frame 1 (202 blocks)...");
    let start = std::time::Instant::now();

    // Test all blocks in first frame
    for (block_idx, input_block) in tp05.frame(0).iter().enumerate() {
        let expected_output = &tp06.frame(0)[block_idx];

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

    let elapsed = start.elapsed();
    println!(
        "LDPC Encoding (Frame 1): {} successes, {} failures in {:.2}s",
        successes,
        failures,
        elapsed.as_secs_f64()
    );
    
    if successes > 0 {
        let throughput = (successes * 38880) as f64 / elapsed.as_secs_f64() / 1_000_000.0;
        println!("  Throughput: {:.2} Mbps", throughput);
    }

    assert_eq!(
        failures, 0,
        "LDPC encoding validation failed on {} blocks",
        failures
    );
}

/// Test 2: Verify LDPC decoding TP06 → TP05 (error-free)
///
/// Validates that hard-decision decoding of valid codewords recovers
/// the original message bits.
#[test]
#[ignore]
fn test_ldpc_decoding_tp06_to_tp05_error_free() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let mut decoder = LdpcDecoder::new(code.clone());

    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    let mut successes = 0;
    let mut failures = 0;
    let mut total_iterations = 0;

    println!("Testing LDPC decoding on Frame 1 (202 blocks, error-free)...");
    let start = std::time::Instant::now();

    // Test all blocks in first frame
    for (block_idx, codeword) in tp06.frame(0).iter().enumerate() {
        let expected_message = &tp05.frame(0)[block_idx];

        // Convert to soft LLRs (high confidence for error-free)
        let mut llrs = Vec::with_capacity(codeword.data.len());
        for i in 0..codeword.data.len() {
            let bit = codeword.data.get(i);
            llrs.push(if bit {
                Llr::new(-10.0) // Strong belief in bit 1
            } else {
                Llr::new(10.0) // Strong belief in bit 0
            });
        }

        // Decode
        let result = decoder.decode_iterative(&llrs, 50);
        total_iterations += result.iterations;

        // Compare
        if result.decoded_bits == expected_message.data {
            successes += 1;
        } else {
            failures += 1;
            eprintln!(
                "Frame 1, Block {}: MISMATCH (converged={}, iterations={})",
                block_idx + 1,
                result.converged,
                result.iterations
            );
        }
    }

    let elapsed = start.elapsed();
    let avg_iterations = total_iterations as f64 / tp06.frame(0).len() as f64;
    
    println!(
        "LDPC Decoding (Frame 1, error-free): {} successes, {} failures in {:.2}s",
        successes, failures, elapsed.as_secs_f64()
    );
    println!("  Average iterations: {:.1}", avg_iterations);
    
    if successes > 0 {
        let throughput = (successes * 38880) as f64 / elapsed.as_secs_f64() / 1_000_000.0;
        println!("  Throughput: {:.2} Mbps", throughput);
    }

    assert_eq!(
        failures, 0,
        "LDPC decoding validation failed on {} blocks",
        failures
    );
}

/// Test 3: Verify LDPC error correction capability with injected errors
///
/// Tests the decoder's ability to correct random bit errors in codewords.
#[test]
#[ignore]
fn test_ldpc_error_correction() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let mut decoder = LdpcDecoder::new(code.clone());

    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    let mut rng = rand::thread_rng();

    println!("Testing LDPC error correction capability...");
    println!("Code: DVB-T2 Normal, Rate 3/5 (n=64800, k=38880)");

    let num_test_blocks = 10.min(tp06.frame(0).len());
    let trials_per_block = 5;

    // Test different error rates
    let error_rates = vec![0.001, 0.005, 0.01, 0.02]; // Fraction of bits flipped

    for error_rate in error_rates {
        let mut successes = 0;
        let mut failures = 0;
        let mut total_iterations = 0;

        for block_idx in 0..num_test_blocks {
            let codeword = &tp06.frame(0)[block_idx];
            let expected_message = &tp05.frame(0)[block_idx];

            for _trial in 0..trials_per_block {
                // Inject random errors
                let mut corrupted = codeword.data.clone();
                let num_errors = (corrupted.len() as f64 * error_rate).round() as usize;
                let mut error_positions = Vec::new();

                while error_positions.len() < num_errors {
                    let pos = rng.gen_range(0..corrupted.len());
                    if !error_positions.contains(&pos) {
                        error_positions.push(pos);
                        corrupted.set(pos, !corrupted.get(pos));
                    }
                }

                // Convert to soft LLRs (moderate confidence)
                let mut llrs = Vec::with_capacity(corrupted.len());
                for i in 0..corrupted.len() {
                    let bit = corrupted.get(i);
                    llrs.push(if bit {
                        Llr::new(-3.0) // Moderate belief in bit 1
                    } else {
                        Llr::new(3.0) // Moderate belief in bit 0
                    });
                }

                // Decode
                let result = decoder.decode_iterative(&llrs, 50);
                total_iterations += result.iterations;

                // Check if corrected
                if result.decoded_bits == expected_message.data {
                    successes += 1;
                } else {
                    failures += 1;
                }
            }
        }

        let total = num_test_blocks * trials_per_block;
        let avg_iterations = total_iterations as f64 / total as f64;
        
        println!(
            "  Error rate {:.3} ({} errors/block): {}/{} corrected ({:.1}%), {} failed, avg iter: {:.1}",
            error_rate,
            (64800.0 * error_rate).round() as usize,
            successes,
            total,
            100.0 * successes as f64 / total as f64,
            failures,
            avg_iterations
        );
    }
}

/// Test 4: Verify LDPC systematic encoding property
///
/// Checks that the first k bits of each codeword match the message bits.
#[test]
#[ignore]
fn test_ldpc_systematic_property() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    let k = code.k();
    let n = code.n();
    let parity_bits = n - k;

    println!(
        "Verifying systematic encoding: k={}, n={}, parity={}",
        k, n, parity_bits
    );

    // Check first few blocks
    for block_idx in 0..5.min(tp05.frame(0).len()) {
        let message = &tp05.frame(0)[block_idx];
        let codeword = &tp06.frame(0)[block_idx];

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

/// Test 5: Sample blocks across multiple frames
///
/// Spot-checks encoding consistency across all frames.
#[test]
#[ignore]
fn test_ldpc_encoding_sample() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let cache = try_load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let encoder = create_encoder(code, cache.as_ref());

    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    println!("Testing sample blocks from each frame...");

    for frame_idx in 0..tp05.num_frames() {
        let frame_tp05 = tp05.frame(frame_idx);
        let frame_tp06 = tp06.frame(frame_idx);

        // Test first, middle, and last block of each frame
        let test_indices = vec![0, frame_tp05.len() / 2, frame_tp05.len() - 1];

        for &block_idx in &test_indices {
            let message = &frame_tp05[block_idx];
            let expected_cw = &frame_tp06[block_idx];
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

/// Test 6: Validate DVB-T2 LDPC parameters
///
/// Ensures that the LDPC code parameters match DVB-T2 specification.
#[test]
#[ignore]
fn test_ldpc_parameter_validation() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    println!("Validating DVB-T2 LDPC parameters...");

    // Check dimensions
    assert_eq!(code.n(), 64800, "LDPC codeword length mismatch");
    assert_eq!(code.k(), 38880, "LDPC message length mismatch");
    assert_eq!(code.m(), 64800 - 38880, "LDPC parity check count mismatch");

    // Check rate
    let rate = code.k() as f64 / code.n() as f64;
    let expected_rate = 3.0 / 5.0;
    assert!(
        (rate - expected_rate).abs() < 0.01,
        "Code rate mismatch: expected {:.3}, got {:.3}",
        expected_rate,
        rate
    );

    // Check test vector consistency
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    assert_eq!(
        tp05.frame(0)[0].data.len(),
        code.k(),
        "TP05 block length mismatch"
    );
    assert_eq!(
        tp06.frame(0)[0].data.len(),
        code.n(),
        "TP06 block length mismatch"
    );

    println!("✓ All parameters validated");
}

/// Test 7: Parity check validation
///
/// Verifies that all codewords in TP06 satisfy H·c = 0.
#[test]
#[ignore]
fn test_ldpc_parity_check() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    println!("Verifying parity check property H·c = 0...");

    let num_test_blocks = 10.min(tp06.frame(0).len());
    let mut failures = Vec::new();

    for (block_idx, codeword) in tp06.frame(0).iter().take(num_test_blocks).enumerate() {
        if !code.is_valid_codeword(&codeword.data) {
            let syndrome = code.syndrome(&codeword.data);
            let weight = syndrome.count_ones();
            failures.push((block_idx + 1, weight));
            println!("  Block {}: FAILED (syndrome weight = {})", block_idx + 1, weight);
        } else {
            println!("  Block {}: PASS", block_idx + 1);
        }
    }

    if !failures.is_empty() {
        println!("\nFailed blocks: {:?}", failures);
        panic!("Parity check failed for {} blocks", failures.len());
    }
    
    println!("✓ Parity check validated for {} blocks", num_test_blocks);
}

/// Test 8: Encode/decode roundtrip
///
/// Full roundtrip: message → encode → decode → message
#[test]
#[ignore]
fn test_ldpc_roundtrip() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let cache = try_load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let encoder = create_encoder(code.clone(), cache.as_ref());
    let mut decoder = LdpcDecoder::new(code);

    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    println!("Testing encode/decode roundtrip...");

    let num_test_blocks = 10.min(tp05.frame(0).len());
    let mut successes = 0;
    let mut failures = 0;

    for (block_idx, message_block) in tp05.frame(0).iter().take(num_test_blocks).enumerate() {
        // Encode
        let codeword = encoder.encode(&message_block.data);

        // Convert to soft LLRs
        let mut llrs = Vec::with_capacity(codeword.len());
        for i in 0..codeword.len() {
            let bit = codeword.get(i);
            llrs.push(if bit {
                Llr::new(-10.0) // Strong belief in bit 1
            } else {
                Llr::new(10.0) // Strong belief in bit 0
            });
        }

        // Decode
        let result = decoder.decode_iterative(&llrs, 50);

        // Check roundtrip
        if result.decoded_bits == message_block.data {
            successes += 1;
        } else {
            failures += 1;
            eprintln!("Block {} roundtrip failed", block_idx + 1);
        }
    }

    println!(
        "Roundtrip: {} successes, {} failures",
        successes, failures
    );
    assert_eq!(failures, 0, "Roundtrip validation failed on {} blocks", failures);
}
