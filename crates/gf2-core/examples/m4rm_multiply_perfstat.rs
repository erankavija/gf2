//! Perf-stat harness for M4RM block multiply scheduling (kernel B3).
//!
//! Run with `perf stat -r 10` and set `B3_PERFSTAT_MODE=rowwise` to capture
//! the pre-register-tile schedule, or leave it unset for the production tiled
//! schedule.
//!
//! Environment variables:
//! - `B3_PERFSTAT_ITERS` (default 2_000) — measured-iteration count.
//! - `B3_PERFSTAT_MODE` (default `tiled`) — `tiled` for the production
//!   register-tiled schedule, `rowwise` for the pre-register-tile path.
//! - `B3_PERFSTAT_SIZE` (default 1024) — square matrix side. The
//!   `jit:0fd48627` GF(2) M4RI gap profile drives this at 1024 (cache-
//!   resident) and 4096 (LLC-streaming) to expose the bottleneck regime.
//! - `B3_PERFSTAT_WARMUP` (default 64) — warmup-iteration count. Larger
//!   sizes (e.g. 4096) typically use a smaller warmup so a perf-stat
//!   capture finishes in bounded wall time.

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
    // B3_PERFSTAT_SIZE selects the square matmul side length. Default is the
    // historical 1024 used by the B3 register-tile bring-up; profiling work for
    // jit:0fd48627 also drives 4096 for the cache-resident-vs-streaming regime
    // comparison.
    let size: usize = std::env::var("B3_PERFSTAT_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024);
    // B3_PERFSTAT_WARMUP keeps the historical 64-iter warmup on n=1024 but
    // lets larger sizes use a smaller warmup so a perf-stat run finishes in
    // bounded wall time.
    let warmup: usize = std::env::var("B3_PERFSTAT_WARMUP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(64);

    let a = random_matrix(size, size, 0x19bc_3199_0000_0001);
    let b = random_matrix(size, size, 0x19bc_3199_0000_0002);

    for _ in 0..warmup {
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
