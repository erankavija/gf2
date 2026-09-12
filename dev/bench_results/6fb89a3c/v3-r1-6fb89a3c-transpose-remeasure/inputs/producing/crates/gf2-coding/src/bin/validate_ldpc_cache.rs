//! Validate LDPC cache with error correction tests

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_coding::CodeRate;
use std::path::Path;

fn main() {
    println!("=== LDPC Cache Validation with Error Correction ===\n");

    // Load cache
    println!("Loading cache from data/ldpc/dvb_t2...");
    let cache =
        EncodingCache::from_directory(Path::new("data/ldpc/dvb_t2")).expect("Failed to load cache");

    println!("✓ Cache loaded: {} entries\n", cache.stats().entries);

    // Create encoder/decoder
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate3_5);
    let encoder = LdpcEncoder::with_cache(code.clone(), &cache);
    let mut decoder = LdpcDecoder::new(code);

    println!("Testing DVB-T2 Short Rate 3/5:");
    println!("  n = {} (codeword length)", encoder.n());
    println!("  k = {} (message length)", encoder.k());
    println!("  r = {} (parity length)\n", encoder.n() - encoder.k());

    // Test 1: Error-free roundtrip
    println!("Test 1: Error-free roundtrip");
    test_roundtrip(&encoder, &mut decoder, 0);

    // Test 2: Increasing error counts
    println!("\nTest 2: Error correction capability");
    for num_errors in [1, 5, 10, 20, 50, 100, 200, 500, 1000] {
        test_roundtrip(&encoder, &mut decoder, num_errors);
    }

    println!("\n=== Validation Complete ===");
}

fn test_roundtrip(encoder: &LdpcEncoder, decoder: &mut LdpcDecoder, num_errors: usize) {
    use rand::Rng;

    // Create random message
    let mut rng = rand::thread_rng();
    let mut message = gf2_core::BitVec::zeros(encoder.k());
    for i in 0..encoder.k() {
        if rng.gen_bool(0.5) {
            message.set(i, true);
        }
    }

    // Encode
    let codeword = encoder.encode(&message);
    assert_eq!(codeword.len(), encoder.n());

    // Add errors
    let mut received = codeword.clone();
    if num_errors > 0 {
        let mut error_positions = Vec::new();
        while error_positions.len() < num_errors {
            let pos = rng.gen_range(0..encoder.n());
            if !error_positions.contains(&pos) {
                error_positions.push(pos);
            }
        }

        for &pos in &error_positions {
            received.set(pos, !received.get(pos));
        }
    }

    // Convert to LLRs (hard decision: +∞ for 0, -∞ for 1)
    // Use finite values for numerical stability
    let llrs: Vec<Llr> = (0..encoder.n())
        .map(|i| Llr::new(if received.get(i) { -10.0 } else { 10.0 }))
        .collect();

    // Decode with soft decoder
    let result = decoder.decode_iterative(&llrs, 50); // Max 50 iterations

    if result.converged {
        let decoded_cw = result.decoded_bits;

        // Extract message bits (first k bits for systematic code)
        let mut decoded_msg = gf2_core::BitVec::zeros(encoder.k());
        for i in 0..encoder.k() {
            decoded_msg.set(i, decoded_cw.get(i));
        }

        // Check if decoded message matches original
        let success = decoded_msg == message;

        if success {
            if num_errors == 0 {
                println!(
                    "  ✓ Error-free: PASS (converged in {} iters)",
                    result.iterations
                );
            } else {
                println!(
                    "  ✓ {:4} errors: CORRECTED (converged in {} iters)",
                    num_errors, result.iterations
                );
            }
        } else {
            println!("  ✗ {:4} errors: INCORRECT DECODING", num_errors);

            // Count bit errors in decoded vs original
            let mut bit_errors = 0;
            for i in 0..message.len() {
                if decoded_msg.get(i) != message.get(i) {
                    bit_errors += 1;
                }
            }
            println!("     {} bit errors in decoded message", bit_errors);
        }
    } else {
        println!(
            "  ✗ {:4} errors: DID NOT CONVERGE (stopped at {} iters)",
            num_errors, result.iterations
        );
    }
}
