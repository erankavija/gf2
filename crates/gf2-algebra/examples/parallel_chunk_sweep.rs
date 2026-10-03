//! Chunk-size sweep for `permanent_bipedal3_parallel_with_chunk` at n=28.
//!
//! Times `SAMPLES_PER_CHUNK` seeded matrices per chunk size in `CHUNK_SIZES` and
//! writes `dev/benchmarks/gf2_algebra_permanent/parallel_chunk_sweep-<DATE>.csv`
//! (date overridable via `SA_DATE`) with columns `chunk_size`, `mean_us`,
//! `std_us`, `throughput_subsets_per_sec` (`(2^n - 1) / (mean_us * 1e-6)`) and
//! `samples`.

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::{
    permanent_bipedal3_parallel_with_chunk, CHUNK_SUBSETS,
};
use gf2_algebra::testutil::{random_matrix, today_yyyy_mm_dd};
use std::fs::{self, File};
use std::io::Write;
use std::time::Instant;

const SWEEP_N: usize = 28;

const SAMPLES_PER_CHUNK: usize = 3;

const SEED_BASE: u64 = 0x0525_0df5_0000_0000;

const CHUNK_SIZES: &[usize] = &[
    1 << 7,  // 128
    1 << 10, // 1024
    1 << 12, // 4096
    1 << 14, // 16_384
    1 << 16, // 65_536
    1 << 18, // 262_144
    1 << 20, // 1_048_576
    1 << 22, // 4_194_304
];

fn main() {
    let date = today_yyyy_mm_dd();
    let csv_dir = "dev/benchmarks/gf2_algebra_permanent";
    let csv_path = format!("{csv_dir}/parallel_chunk_sweep-{date}.csv");

    fs::create_dir_all(csv_dir).expect("create benchmarks dir");
    let mut csv = File::create(&csv_path).expect("create CSV");
    writeln!(
        csv,
        "chunk_size,mean_us,std_us,throughput_subsets_per_sec,samples"
    )
    .unwrap();

    let total_subsets = (1u64 << SWEEP_N) - 1;
    let default_chunk = CHUNK_SUBSETS;

    println!("T15 (jit:05250df5) — parallel permanent chunk-size sweep");
    println!("n={SWEEP_N}, total subsets={total_subsets}, default CHUNK_SUBSETS={default_chunk}");
    println!(
        "Sweep: chunk sizes {:?}, {} samples each",
        CHUNK_SIZES, SAMPLES_PER_CHUNK
    );
    println!("Threads: {} (rayon default)", rayon::current_num_threads());
    println!("{:-<78}", "");

    let mut best_chunk = CHUNK_SIZES[0];
    let mut best_throughput = 0.0f64;

    for &chunk in CHUNK_SIZES {
        let mut samples: Vec<f64> = Vec::with_capacity(SAMPLES_PER_CHUNK);

        for sample_idx in 0..SAMPLES_PER_CHUNK {
            let seed = SEED_BASE
                .wrapping_add(SWEEP_N as u64)
                .wrapping_mul(1_000_003)
                .wrapping_add(sample_idx as u64);
            let row_major = random_matrix::<3>(SWEEP_N, seed);
            let mat = Bipedal3Matrix::from_row_major(&row_major, SWEEP_N, SWEEP_N);

            let t0 = Instant::now();
            let _result = std::hint::black_box(permanent_bipedal3_parallel_with_chunk(&mat, chunk));
            let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
            samples.push(elapsed_us);
        }

        let n_samples = samples.len();
        let mean = samples.iter().sum::<f64>() / n_samples as f64;
        let variance = if n_samples > 1 {
            samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n_samples as f64 - 1.0)
        } else {
            0.0
        };
        let std_dev = variance.sqrt();
        let throughput = total_subsets as f64 / (mean * 1e-6);

        writeln!(
            csv,
            "{chunk},{mean:.3},{std_dev:.3},{throughput:.0},{n_samples}"
        )
        .unwrap();

        let marker = if chunk == default_chunk {
            " <-- default CHUNK_SUBSETS"
        } else {
            ""
        };
        println!(
            "chunk=2^{:2} ({:7})  mean={:10.3} us  std={:8.3} us  tput={:.3e} subsets/s{}",
            (chunk as f64).log2() as u32,
            chunk,
            mean,
            std_dev,
            throughput,
            marker
        );

        if throughput > best_throughput {
            best_throughput = throughput;
            best_chunk = chunk;
        }
    }

    println!("{:-<78}", "");
    println!(
        "Best chunk size: 2^{} = {} subsets  throughput = {:.3e} subsets/s",
        (best_chunk as f64).log2() as u32,
        best_chunk,
        best_throughput
    );
    println!(
        "Default CHUNK_SUBSETS = 2^{} = {}",
        (default_chunk as f64).log2() as u32,
        default_chunk
    );
    if best_chunk == default_chunk {
        println!("PASS: default chunk size matches best observed.");
    } else {
        println!(
            "NOTE: best observed chunk 2^{} != default 2^{}; consider updating CHUNK_SUBSETS.",
            (best_chunk as f64).log2() as u32,
            (default_chunk as f64).log2() as u32
        );
    }
    println!("CSV written to: {csv_path}");
}
