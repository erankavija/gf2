//! Fixed-iteration sparse matvec harness for `perf stat`.
//!
//! Criterion adapts iteration counts per benchmark function, which can obscure
//! cache-event rates when comparing CSR with transformed layouts. This example
//! executes exactly the requested number of LDPC-sized matvecs for each mode so
//! `perf stat -r 10` compares the same operation count.

use gf2_core::sparse::SpBitMatrix;
use gf2_core::BitVec;
use std::env;
use std::hint::black_box;
use std::time::Instant;

fn deterministic_ldpc_like(rows: usize, cols: usize, row_weight: usize) -> SpBitMatrix {
    let mut entries = Vec::with_capacity(rows * row_weight);
    for r in 0..rows {
        let base = r.wrapping_mul(1_315_423_911usize) ^ rows.rotate_left(7);
        for k in 0..row_weight {
            let stride = 2 * k + 1;
            let col = base
                .wrapping_add(k.wrapping_mul(97_531))
                .wrapping_add(r.wrapping_mul(stride))
                % cols;
            entries.push((r, col));
        }
    }
    SpBitMatrix::from_coo_deduplicated(rows, cols, &entries)
}

fn deterministic_bitvec(len: usize) -> BitVec {
    let mut x = BitVec::with_capacity(len);
    for i in 0..len {
        x.push_bit(((i.wrapping_mul(0x9E37_79B1) ^ (i >> 3)) & 7) < 3);
    }
    x
}

fn parse_arg(args: &[String], name: &str, default: usize) -> usize {
    args.windows(2)
        .find_map(|window| {
            (window[0] == name)
                .then(|| window[1].parse().ok())
                .flatten()
        })
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args
        .windows(2)
        .find_map(|window| (window[0] == "--mode").then(|| window[1].as_str()))
        .unwrap_or("csr");
    let rows = parse_arg(&args, "--rows", 8192);
    let cols = parse_arg(&args, "--cols", 16384);
    let row_weight = parse_arg(&args, "--row-weight", 6);
    let block_rows = parse_arg(&args, "--block-rows", 32);
    let prefetch_distance = parse_arg(&args, "--prefetch-distance", 16);
    let iters = parse_arg(&args, "--iters", 100_000);

    let csr = deterministic_ldpc_like(rows, cols, row_weight);
    let block = csr.to_block_csr(block_rows);
    let x = deterministic_bitvec(cols);
    assert_eq!(
        block.matvec_with_prefetch_distance(&x, 0),
        csr.matvec(&x),
        "block-CSR harness must match CSR before timing"
    );

    let start = Instant::now();
    let mut checksum = 0u64;
    for _ in 0..iters {
        let y = match mode {
            "csr" => csr.matvec(black_box(&x)),
            "block_csr_no_prefetch" => block.matvec_with_prefetch_distance(black_box(&x), 0),
            "block_csr_prefetch" => {
                block.matvec_with_prefetch_distance(black_box(&x), prefetch_distance)
            }
            other => panic!(
                "unknown --mode {other}; expected csr, block_csr_no_prefetch, or block_csr_prefetch"
            ),
        };
        checksum ^= y
            .words()
            .iter()
            .fold(y.len() as u64, |acc, &word| acc.rotate_left(1) ^ word);
    }

    let elapsed = start.elapsed();
    println!(
        "mode={mode} rows={rows} cols={cols} row_weight={row_weight} block_rows={block_rows} \
         prefetch_distance={prefetch_distance} iters={iters} elapsed={elapsed:?} checksum={checksum}"
    );
}
