//! Perf-stat harness for M4RM multiplication. Environment variables:
//! - `B3_PERFSTAT_ITERS` (default 2000): measured iterations.
//! - `B3_PERFSTAT_MODE` (default `tiled`): `tiled` for the production
//!   schedule, `rowwise` for the row-wise path, `gray-schedule` for an
//!   explicit Gray-table schedule.
//! - `B3_GRAY_TARGET_BYTES` (default 262144): table footprint for
//!   `gray-schedule`.
//! - `B3_GRAY_MAX_K` (default 10): maximum panel width for `gray-schedule`.
//! - `B3_PERFSTAT_SIZE` (default 1024): square matrix side.
//! - `B3_PERFSTAT_WARMUP` (default 64): warmup iterations.

use gf2_core::alg::m4rm::{
    multiply, multiply_rowwise_for_test, multiply_with_table_schedule_for_test,
};
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
    let gray_target_bytes: usize = std::env::var("B3_GRAY_TARGET_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(256 * 1024);
    let gray_max_k: usize = std::env::var("B3_GRAY_MAX_K")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let size: usize = std::env::var("B3_PERFSTAT_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024);
    let warmup: usize = std::env::var("B3_PERFSTAT_WARMUP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(64);

    let a = random_matrix(size, size, 0x19bc_3199_0000_0001);
    let b = random_matrix(size, size, 0x19bc_3199_0000_0002);

    for _ in 0..warmup {
        let c = multiply_mode(&a, &b, &mode, gray_target_bytes, gray_max_k);
        std::hint::black_box(c);
    }

    let mut checksum = 0u64;
    for _ in 0..iters {
        let c = multiply_mode(&a, &b, &mode, gray_target_bytes, gray_max_k);
        checksum ^= c.row_words(0).iter().fold(0u64, |acc, &w| acc ^ w);
    }

    println!(
        "mode={mode} iters={iters} gray_target_bytes={gray_target_bytes} gray_max_k={gray_max_k} checksum={checksum}"
    );
}

fn multiply_mode(
    a: &BitMatrix,
    b: &BitMatrix,
    mode: &str,
    gray_target_bytes: usize,
    gray_max_k: usize,
) -> BitMatrix {
    match mode {
        "rowwise" => multiply_rowwise_for_test(std::hint::black_box(a), std::hint::black_box(b)),
        "gray-schedule" => multiply_with_table_schedule_for_test(
            std::hint::black_box(a),
            std::hint::black_box(b),
            gray_target_bytes,
            gray_max_k,
        ),
        _ => multiply(std::hint::black_box(a), std::hint::black_box(b)),
    }
}
