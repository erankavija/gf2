//! Example demonstrating random BitVec and BitMatrix generation.
//!
//! Run with: cargo run --example random_generation --features rand

use gf2_core::{matrix::BitMatrix, BitVec};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn main() {
    println!("=== Random BitVec Generation ===\n");

    // Create a random bit vector with uniform distribution (p=0.5)
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random(100, &mut rng);
    println!("Random BitVec (100 bits): {} ones", bv.count_ones());

    // Deterministic random generation
    let bv1 = BitVec::random_seeded(50, 0x1234);
    let bv2 = BitVec::random_seeded(50, 0x1234);
    println!("\nDeterministic generation:");
    println!("  bv1 == bv2: {}", bv1 == bv2);

    // Sparse bit vector (10% ones)
    let sparse = BitVec::random_with_probability(1000, 0.1, &mut rng);
    println!(
        "\nSparse BitVec (p=0.1, 1000 bits): {} ones",
        sparse.count_ones()
    );

    // Dense bit vector (90% ones)
    let dense = BitVec::random_with_probability(1000, 0.9, &mut rng);
    println!(
        "Dense BitVec (p=0.9, 1000 bits): {} ones",
        dense.count_ones()
    );

    println!("\n=== Random BitMatrix Generation ===\n");

    // Create a random matrix
    let m = BitMatrix::random(10, 20, &mut rng);
    println!(
        "Random BitMatrix (10×20): {} rows, {} cols",
        m.rows(),
        m.cols()
    );

    // Count ones in the matrix
    let mut ones = 0;
    for r in 0..m.rows() {
        for c in 0..m.cols() {
            if m.get(r, c) {
                ones += 1;
            }
        }
    }
    println!(
        "  Total ones: {} / {} ({:.1}%)",
        ones,
        m.rows() * m.cols(),
        100.0 * ones as f64 / (m.rows() * m.cols()) as f64
    );

    // Sparse matrix
    let sparse_m = BitMatrix::random_with_probability(50, 50, 0.05, &mut rng);
    let mut ones = 0;
    for r in 0..sparse_m.rows() {
        for c in 0..sparse_m.cols() {
            if sparse_m.get(r, c) {
                ones += 1;
            }
        }
    }
    println!("\nSparse BitMatrix (p=0.05, 50×50):");
    println!(
        "  Total ones: {} / {} ({:.1}%)",
        ones,
        sparse_m.rows() * sparse_m.cols(),
        100.0 * ones as f64 / (sparse_m.rows() * sparse_m.cols()) as f64
    );

    // Fill existing structures with random bits
    let mut bv = BitVec::from_bytes_le(&[0x00; 10]);
    println!("\n=== Fill Existing Structures ===\n");
    println!("Before fill_random: {} ones", bv.count_ones());
    bv.fill_random(&mut rng);
    println!("After fill_random: {} ones", bv.count_ones());
}
