//! Perf-stat harness for M4RM block multiply scheduling (kernel B3).
//!
//! Run with `perf stat -r 10` and set `B3_PERFSTAT_MODE=rowwise` to capture
//! the pre-register-tile schedule, or leave it unset for the production tiled
//! schedule.

use gf2_core::alg::m4rm::{multiply, multiply_rowwise_for_test};
use gf2_core::matrix::BitMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn random_matrix(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = BitMatrix::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen_bool(0.5) {
                m.set(r, c, true);
            }
        }
    }
    m
}

fn main() {
    let iters: usize = std::env::var("B3_PERFSTAT_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2_000);
    let mode = std::env::var("B3_PERFSTAT_MODE").unwrap_or_else(|_| "tiled".to_string());

    let a = random_matrix(1024, 1024, 0x19bc_3199_0000_0001);
    let b = random_matrix(1024, 1024, 0x19bc_3199_0000_0002);

    for _ in 0..64 {
        let c = if mode == "rowwise" {
            multiply_rowwise_for_test(std::hint::black_box(&a), std::hint::black_box(&b))
        } else {
            multiply(std::hint::black_box(&a), std::hint::black_box(&b))
        };
        std::hint::black_box(c);
    }

    let mut checksum = 0u64;
    for _ in 0..iters {
        let c = if mode == "rowwise" {
            multiply_rowwise_for_test(std::hint::black_box(&a), std::hint::black_box(&b))
        } else {
            multiply(std::hint::black_box(&a), std::hint::black_box(&b))
        };
        checksum ^= c.row_words(0).iter().fold(0u64, |acc, &w| acc ^ w);
    }

    println!("mode={mode} iters={iters} checksum={checksum}");
}
