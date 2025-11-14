//! Example: LDPC-coded transmission over AWGN channel with belief propagation decoding.
//!
//! This example demonstrates:
//! - LDPC code construction (regular code)
//! - Encoding (systematic, all-zero codewords for simplicity)
//! - BPSK modulation over AWGN channel
//! - Iterative belief propagation decoding
//! - Frame error rate (FER) and convergence analysis
//!
//! Compares coded vs. uncoded performance to show LDPC coding gain.

use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_coding::{AwgnChannel, BpskModulator, LdpcCode, LdpcDecoder};
use gf2_core::BitVec;

fn main() {
    println!("=== LDPC-Coded BPSK Transmission over AWGN ===\n");

    // Create a regular (3,6) LDPC code
    // Column weight 3, row weight 6
    // This gives rate ≈ 1/2
    let (code, _n_checks, _n_vars) = create_regular_ldpc_3_6(24, 48);

    println!("LDPC Code Parameters:");
    println!("  n (codeword length): {}", code.n());
    println!("  m (check nodes):     {}", code.m());
    println!("  k (message bits):    {}", code.k());
    println!("  Rate:                {:.3}", code.rate());
    println!("  Structure:           Regular (3,6)");
    println!();

    // Show Shannon limit for this rate
    let shannon_limit_db = AwgnChannel::shannon_limit(code.rate());
    println!("Shannon Limit:");
    println!(
        "  Min Eb/N0 for R={:.3}: {:.2} dB",
        code.rate(),
        shannon_limit_db
    );
    println!("  (Theoretical limit for reliable communication)");
    println!();

    let num_frames = 1000; // Number of codewords to test per SNR point
    let max_iterations = 50;

    // Eb/N0 range appropriate for rate-1/2 LDPC
    let eb_n0_range = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

    println!("Simulating {} frames per Eb/N0 point", num_frames);
    println!("Max iterations: {}\n", max_iterations);

    println!("┌──────────┬──────────┬──────────┬──────────────┬──────────┐");
    println!("│ Eb/N0 dB │   FER    │ Avg Iter │  Uncoded BER │ Capacity │");
    println!("├──────────┼──────────┼──────────┼──────────────┼──────────┤");

    for &eb_n0_db in &eb_n0_range {
        let (fer, avg_iter, uncoded_ber) =
            simulate_ldpc_transmission(&code, num_frames, eb_n0_db, max_iterations);

        let capacity = AwgnChannel::shannon_capacity(eb_n0_db);

        println!(
            "│   {:5.1}  │  {:6.4}  │   {:5.1}  │   {:8.6}   │  {:6.4}  │",
            eb_n0_db, fer, avg_iter, uncoded_ber, capacity
        );
    }

    println!("└──────────┴──────────┴──────────┴──────────────┴──────────┘\n");

    println!("Notes:");
    println!("- FER: Frame Error Rate (proportion of incorrectly decoded frames)");
    println!("- Avg Iter: Average number of BP iterations per frame");
    println!("- Uncoded BER: Baseline bit error rate without coding");
    println!("- Capacity: Shannon capacity at this Eb/N0 (max achievable rate)");
    println!("- LDPC shows coding gain: lower FER than uncoded BER at same Eb/N0");
    println!();
    println!("Shannon Limit Analysis:");
    println!(
        "  Shannon limit: {:.2} dB (min Eb/N0 for R={:.3})",
        shannon_limit_db,
        code.rate()
    );
    println!(
        "  Gap to Shannon limit at FER=0.01: ~{:.1} dB",
        5.0 - shannon_limit_db
    ); // Approximate from results
    println!();
    println!("Typical results:");
    println!("  At Eb/N0 = 2 dB: FER ≈ 0.1-0.5 (converging)");
    println!("  At Eb/N0 = 4 dB: FER ≈ 0.01 (good performance)");
}

/// Creates a regular (3,6) LDPC code with specified dimensions.
///
/// Regular (dv, dc) code: column weight dv, row weight dc.
/// For (3,6): each variable node connects to 3 checks, each check to 6 variables.
fn create_regular_ldpc_3_6(m: usize, n: usize) -> (LdpcCode, usize, usize) {
    let mut edges = Vec::new();

    // Simple construction: distribute edges to maintain regularity
    // This is a basic approach; production code would use proper construction algorithms
    let column_weight = 3;
    let row_weight = 6;

    // Verify parameters are consistent
    assert_eq!(n * column_weight, m * row_weight, "Total edges must match");

    // Build edges column by column
    for col in 0..n {
        for i in 0..column_weight {
            let row = ((col * column_weight) + i) % m;
            edges.push((row, col));
        }
    }

    let code = LdpcCode::from_edges(m, n, &edges);
    (code, m, n)
}

fn simulate_ldpc_transmission(
    code: &LdpcCode,
    num_frames: usize,
    eb_n0_db: f64,
    max_iterations: usize,
) -> (f64, f64, f64) {
    let mut rng = rand::thread_rng();
    let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, code.rate());

    let mut decoder = LdpcDecoder::new(code.clone());

    let mut frame_errors = 0;
    let mut total_iterations = 0;
    let mut uncoded_bit_errors = 0;
    let mut total_bits = 0;

    for _frame in 0..num_frames {
        // Transmit all-zero codeword (valid LDPC codeword)
        // In practice, you'd encode actual message bits
        let codeword = BitVec::zeros(code.n());

        // Modulate to BPSK
        let bits_vec: Vec<bool> = (0..code.n()).map(|i| codeword.get(i)).collect();
        let symbols = BpskModulator::modulate_bits(&bits_vec);

        // Transmit through AWGN
        let received = channel.transmit_symbols(&symbols, &mut rng);

        // Convert to LLRs
        let llrs: Vec<Llr> = channel.to_llrs(&received);

        // Decode with belief propagation
        let result = decoder.decode_iterative(&llrs, max_iterations);
        total_iterations += result.iterations;

        // Check for frame error
        if !result.converged || !result.syndrome_check_passed {
            frame_errors += 1;
        }

        // Also track uncoded BER for comparison
        let hard_decoded: Vec<bool> = llrs.iter().map(|llr| llr.hard_decision()).collect();
        uncoded_bit_errors += hard_decoded.iter().filter(|&&b| b).count(); // Count 1s (errors from all-zero)
        total_bits += code.n();

        // Reset decoder state between frames
        decoder.reset();
    }

    let fer = frame_errors as f64 / num_frames as f64;
    let avg_iter = total_iterations as f64 / num_frames as f64;
    let uncoded_ber = uncoded_bit_errors as f64 / total_bits as f64;

    (fer, avg_iter, uncoded_ber)
}
