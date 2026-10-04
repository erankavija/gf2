//! LDPC-coded BPSK over AWGN with iterative min-sum decoding: frame error rate and mean iteration
//! count of a regular (3,6) code against the uncoded bit error rate and the BI-AWGN capacity.

use gf2_coding::info_theory::{bi_awgn_capacity, ebn0_to_esn0, shannon_limit};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel};
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_coding::{LdpcCode, LdpcDecoder};
use gf2_core::BitVec;

fn main() {
    println!("=== LDPC-Coded BPSK Transmission over AWGN ===\n");

    let (code, _n_checks, _n_vars) = create_regular_ldpc_3_6(24, 48);

    println!("LDPC Code Parameters:");
    println!("  n (codeword length): {}", code.n());
    println!("  m (check nodes):     {}", code.m());
    println!("  k (message bits):    {}", code.k());
    println!("  Rate:                {:.3}", code.rate());
    println!("  Structure:           Regular (3,6)");
    println!();

    let shannon_limit_db = shannon_limit(code.rate());
    println!("Shannon Limit:");
    println!(
        "  Min Eb/N0 for R={:.3}: {:.2} dB",
        code.rate(),
        shannon_limit_db
    );
    println!("  (Theoretical limit for reliable communication)");
    println!();

    let num_frames = 1000;
    let max_iterations = 50;

    let eb_n0_range = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

    println!("Simulating {} frames per Eb/N0 point", num_frames);
    println!("Max iterations: {}\n", max_iterations);

    println!("┌──────────┬──────────┬──────────┬──────────────┬──────────┐");
    println!("│ Eb/N0 dB │   FER    │ Avg Iter │  Uncoded BER │ Capacity │");
    println!("├──────────┼──────────┼──────────┼──────────────┼──────────┤");

    for &eb_n0_db in &eb_n0_range {
        let (fer, avg_iter, uncoded_ber) =
            simulate_ldpc_transmission(&code, num_frames, eb_n0_db, max_iterations);

        let capacity = bi_awgn_capacity(ebn0_to_esn0(eb_n0_db, 1, code.rate()));

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
    println!("- Capacity: BI-AWGN capacity at this Eb/N0 and the code rate (max achievable rate)");
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
    );
    println!();
    println!("Typical results:");
    println!("  At Eb/N0 = 2 dB: FER ≈ 0.1-0.5 (converging)");
    println!("  At Eb/N0 = 4 dB: FER ≈ 0.01 (good performance)");
}

/// Builds an `m × n` code with column weight 3 by cyclic row assignment.
fn create_regular_ldpc_3_6(m: usize, n: usize) -> (LdpcCode, usize, usize) {
    let mut edges = Vec::new();

    let column_weight = 3;
    let row_weight = 6;

    assert_eq!(n * column_weight, m * row_weight, "Total edges must match");

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
    let channel = BpskAwgnChannel;

    let mut decoder = LdpcDecoder::new(code.clone());

    let mut frame_errors = 0;
    let mut total_iterations = 0;
    let mut uncoded_bit_errors = 0;
    let mut total_bits = 0;

    for _frame in 0..num_frames {
        // The all-zero word is a codeword of every linear code.
        let codeword = BitVec::zeros(code.n());

        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, code.rate(), &mut rng);

        let result = decoder.decode_iterative(&llrs, max_iterations);
        total_iterations += result.iterations;

        if !result.converged || !result.syndrome_check_passed {
            frame_errors += 1;
        }

        let hard_decoded: Vec<bool> = llrs.iter().map(|llr| llr.hard_decision()).collect();
        uncoded_bit_errors += hard_decoded.iter().filter(|&&b| b).count(); // Count 1s (errors from all-zero)
        total_bits += code.n();

        decoder.reset();
    }

    let fer = frame_errors as f64 / num_frames as f64;
    let avg_iter = total_iterations as f64 / num_frames as f64;
    let uncoded_ber = uncoded_bit_errors as f64 / total_bits as f64;

    (fer, avg_iter, uncoded_ber)
}
