// Simple profiling harness for RREF
// Run with: cargo bench --bench rref_profile
// Or with flamegraph: cargo flamegraph --bench rref_profile

use gf2_core::alg::rref::rref;
use gf2_core::matrix::BitMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::hint::black_box;

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
    // Profile medium-sized matrix
    let m = random_matrix(1024, 1024, 42);

    println!("Profiling RREF on 1024×1024 matrix...");
    println!("Running 100 iterations for profiling...");

    for _ in 0..100 {
        let _result = black_box(rref(black_box(&m), false));
    }

    println!("Complete!");
}
