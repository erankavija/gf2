//! Perf-stat harness for the M4RM Gray-code table build (kernel B2).
//!
//! Run with `perf stat -r 10` to capture cycles, IPC, L1d misses, and
//! branch misses for the production table builder at the B2 design point.

use gf2_core::alg::m4rm::build_gray_table_flat;
use gf2_core::kernels::ops::resolve_xor_inplace;
use gf2_core::matrix::BitMatrix;

fn random_matrix(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let mut state = seed | 1;
    let mut m = BitMatrix::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if state & 1 == 1 {
                m.set(r, c, true);
            }
        }
    }
    m
}

fn main() {
    let iters: usize = std::env::var("B2_PERFSTAT_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    let b = random_matrix(1024, 1024, 0x9E37_79B9_7F4A_7C15);
    let k_block: usize = 8;
    let n: usize = 1024;
    let stride_words = n.div_ceil(64);
    let table_size = 1usize << k_block;
    let mut buffer = vec![0u64; table_size * stride_words];
    let xor = resolve_xor_inplace(stride_words);

    for _ in 0..1024 {
        build_gray_table_flat(&b, 0, k_block, n, &mut buffer, xor);
    }

    for _ in 0..iters {
        build_gray_table_flat(
            std::hint::black_box(&b),
            0,
            k_block,
            n,
            std::hint::black_box(&mut buffer),
            xor,
        );
    }

    let sum: u64 = buffer.iter().sum();
    println!("iters={} buffer_sum={}", iters, sum);
}
