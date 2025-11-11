//! Example: Uncoded transmission over AWGN channel with soft-decision decoding.
//!
//! This example demonstrates:
//! - BPSK modulation of random bits
//! - AWGN channel simulation at various Eb/N0 values
//! - Soft-decision (LLR) and hard-decision decoding
//! - Bit error rate (BER) computation
//!
//! This serves as a baseline for comparing coded vs. uncoded transmission.

use gf2_coding::{AwgnChannel, BpskModulator};
use gf2_core::BitVec;

fn main() {
    println!("=== Uncoded BPSK Transmission over AWGN ===\n");

    let num_bits = 1_000_000;
    let eb_n0_range = vec![0.0, 3.0, 6.0, 9.0, 12.0]; // Eb/N0 in dB

    println!("Simulating {} bits per Eb/N0 point\n", num_bits);
    println!("┌──────────┬─────────────┬─────────────┐");
    println!("│ Eb/N0 dB │  Hard BER   │  Soft BER   │");
    println!("├──────────┼─────────────┼─────────────┤");

    for &eb_n0_db in &eb_n0_range {
        let (hard_ber, soft_ber) = simulate_transmission(num_bits, eb_n0_db);
        println!(
            "│   {:5.1}  │  {:7.5}  │  {:7.5}  │",
            eb_n0_db, hard_ber, soft_ber
        );
    }

    println!("└──────────┴─────────────┴─────────────┘\n");

    println!("Notes:");
    println!("- Hard BER: Direct symbol-to-bit conversion (sign of received symbol)");
    println!("- Soft BER: LLR-based decision (should match hard decision for uncoded)");
    println!("- Theoretical BER for uncoded BPSK: Q(sqrt(2 * Eb/N0))");
    println!("  where Q(x) is the Q-function (tail probability of standard normal)");
}

fn simulate_transmission(num_bits: usize, eb_n0_db: f64) -> (f64, f64) {
    let mut rng = rand::thread_rng();
    let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, 1.0); // Rate = 1.0 for uncoded

    // Generate random bits using BitVec
    let bits = BitVec::random(num_bits, &mut rng);

    // Modulate to BPSK symbols (convert BitVec to bool iterator)
    let bits_vec: Vec<bool> = (0..num_bits).map(|i| bits.get(i)).collect();
    let symbols = BpskModulator::modulate_bits(&bits_vec);

    // Transmit through AWGN channel
    let received = channel.transmit_symbols(&symbols, &mut rng);

    // Hard-decision decoding (direct symbol sign)
    let hard_decoded: Vec<bool> = received
        .iter()
        .map(|&r| BpskModulator::demodulate_hard(r))
        .collect();

    // Soft-decision decoding (convert to LLRs then hard decide)
    let llrs = channel.to_llrs(&received);
    let soft_decoded: Vec<bool> = llrs.iter().map(|llr| llr.hard_decision()).collect();

    // Compute bit error rates
    let hard_errors = count_errors(&bits, &hard_decoded);
    let soft_errors = count_errors(&bits, &soft_decoded);

    let hard_ber = hard_errors as f64 / num_bits as f64;
    let soft_ber = soft_errors as f64 / num_bits as f64;

    (hard_ber, soft_ber)
}

fn count_errors(transmitted: &BitVec, received: &[bool]) -> usize {
    (0..transmitted.len())
        .filter(|&i| transmitted.get(i) != received[i])
        .count()
}
