//! T15 (jit:05250df5): Chunk-size sweep for `permanent_bipedal3_parallel`.
//!
//! Measures throughput of the rayon parallel permanent at n=28 across chunk
//! sizes spanning four orders of magnitude: {2^7, 2^10, 2^12, 2^14, 2^16,
//! 2^18, 2^20, 2^22} — 128 → 4_194_304 ≈ 32 768x dynamic range (>10^4).
//! For each chunk size, times SAMPLES_PER_CHUNK independent matrices and
//! records mean throughput in subsets/second.
//!
//! # CSV columns
//!
//! - `chunk_size`               — number of Gray-code subsets per rayon chunk.
//! - `mean_us`                  — mean per-permanent wall-clock time (microseconds).
//! - `std_us`                   — sample standard deviation of per-permanent timings.
//! - `throughput_subsets_per_sec` — mean subsets/second (= (2^n - 1) / (mean_us * 1e-6)).
//! - `samples`                  — number of independent matrices timed.
//!
//! # Output path
//!
//! `dev/benchmarks/gf2_algebra_permanent/parallel_chunk_sweep-<DATE>.csv`
//! where `<DATE>` defaults to today's UTC date (`YYYY-MM-DD`) but can be
//! overridden via the `SA_DATE` environment variable.
//!
//! # Chosen default
//!
//! `CHUNK_SUBSETS = 1 << 16` (65536 subsets per chunk) is the value baked into
//! `permanent_bipedal3_parallel`. See the CSV for empirical justification.
//! At n=28 on the dev host (AMD Ryzen 9 5900X, 12c/24t), the throughput plateau
//! sits across `2^14 .. 2^16` (within ~1 σ of each other); `2^16` is chosen
//! as a single round number near the empirical optimum at `2^14`. Smaller
//! chunks (`2^7 = 128`) waste rayon-scheduler overhead (-91% throughput);
//! larger chunks (`2^22 = 4_194_304`) leave tail threads idle near `2^28` (-10%).
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-algebra --release --features "parallel test-support" \
//!   --example parallel_chunk_sweep
//! # Override the output date:
//! SA_DATE=2026-05-11 cargo run -p gf2-algebra --release \
//!   --features "parallel test-support" --example parallel_chunk_sweep
//! ```

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::{
    permanent_bipedal3_parallel_with_chunk, CHUNK_SUBSETS,
};
use gf2_algebra::testutil::random_matrix;
use std::fs::{self, File};
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Matrix dimension for the sweep. n=28 gives 2^28 - 1 ≈ 268M subsets, which
/// is large enough for reliable throughput measurements while finishing in a
/// few minutes per chunk size on the dev host.
const SWEEP_N: usize = 28;

/// Number of independently seeded matrices timed per chunk size.
const SAMPLES_PER_CHUNK: usize = 3;

/// Base seed derived from the JIT issue ID `05250df5`.
const SEED_BASE: u64 = 0x0525_0df5_0000_0000;

/// Chunk sizes to sweep, spanning >10^4 in dynamic range
/// (128 ≤ chunk ≤ 4_194_304 = `2^7 .. 2^22`, ratio 32 768x).
const CHUNK_SIZES: &[usize] = &[
    1 << 7,  // 128 — far below the typical L1d-friendly band; checks scheduler overhead floor
    1 << 10, // 1024
    1 << 12, // 4096
    1 << 14, // 16_384
    1 << 16, // 65_536  — current CHUNK_SUBSETS default
    1 << 18, // 262_144
    1 << 20, // 1_048_576
    1 << 22, // 4_194_304 — well above where rayon load-balance starves tail threads
];

/// Convert Unix epoch seconds to a `YYYY-MM-DD` UTC date string.
/// (Inlined to avoid pulling `chrono`/`time` as a dep for an example.)
fn unix_secs_to_ymd(secs: i64) -> (i32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y_final = if m <= 2 { y + 1 } else { y };
    (y_final, m, d)
}

fn today_yyyy_mm_dd() -> String {
    if let Ok(s) = std::env::var("SA_DATE") {
        return s;
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = unix_secs_to_ymd(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

// SSOT: the algorithm body lives in
// `gf2_algebra::permanent::parallel_bipedal3::permanent_bipedal3_parallel_with_chunk`.
// This example only varies the `chunk_subsets` parameter and times the
// call; it does NOT duplicate the production code path. That guarantee
// keeps the recorded chunk-sweep CSV trustworthy as the empirical basis
// for the `CHUNK_SUBSETS` default constant.

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
            let _result =
                std::hint::black_box(permanent_bipedal3_parallel_with_chunk(&mat, chunk));
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
