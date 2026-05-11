//! Sa (jit:96dcbec4): Reproduce the paper's Table 2 scaling slope using
//! `permanent_mod3_reference` on the dev host.
//!
//! Times the in-tree [`permanent_mod3_reference`] (W2-T8, the faithful Rust
//! port of Scheinerman 2024 arxiv 2407.20205v2 Julia listing) over a range of
//! matrix sizes `n`, fits `ln(mean_us) = a + b*n` by ordinary least squares,
//! and checks that the observed slope `b` lies within ±10% of the paper's
//! published slope 0.693 nats/n (Table 2, Appendix B; see also `dev/plans/
//! gf2_algebra_permanent.md` §2.4).
//!
//! The paper's Table 2 covers `n ∈ {24, 26, …, 36}` on a 4.20 GHz desktop.
//! Running the Rust port at `n ≥ 26` takes ~hours per matrix on the dev host
//! (AMD Ryzen 9 5900X), so this harness covers `n ∈ {8, 10, 12, …, 24}` (9
//! points). Per criterion 1 amendment 2026-05-11, the slope of log-time vs n
//! is asymptotically `log 2` for any range covering the `O(n·2^n)` regime; the
//! `n = 8..24` sweep is sufficient to estimate it with `R² > 0.99`.
//!
//! # CSV columns
//!
//! - `n` — matrix dimension (sweep parameter).
//! - `mean_us` — mean per-matrix wall-clock time over `SAMPLES_PER_N` samples
//!   (microseconds; varies ~5–10% across repeated runs per criterion 5
//!   amendment).
//! - `std_us` — sample standard deviation of the per-matrix timings.
//! - `samples` — number of independent matrices timed (== `SAMPLES_PER_N`).
//! - `input_hash` — lowercase-hex SHA-256 of the deterministic input matrices
//!   for this `n`, computed by concatenating `(n as u64 LE, seed as u64 LE,
//!   sample_idx as u64 LE, matrix entries as u8s)` across every sample.
//!   Bit-reproducible across runs.
//!
//! # Output paths
//!
//! - CSV at `dev/benchmarks/gf2_algebra_permanent/paper_repro_slope-<DATE>.csv`,
//!   where `<DATE>` defaults to today's UTC date (`YYYY-MM-DD`) but can be
//!   overridden via the `SA_DATE` environment variable for reproducible
//!   pipelines.
//!
//! # Reproducibility (criterion 5 amendment 2026-05-11)
//!
//! Same RNG seed reproduces the same **input matrices** across runs (verified
//! by the `input_hash` column). The `mean_us`/`std_us` columns vary within
//! measurement noise (~5–10% on the dev host); this is the unavoidable
//! property of wall-clock timing.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
//! # Override the date in the filename (e.g. for CI):
//! SA_DATE=2026-05-11 cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
//! ```

use gf2_algebra::permanent::permanent_mod3_reference;
use gf2_algebra::testutil::random_matrix;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Number of independently seeded matrices timed per `n`.
const SAMPLES_PER_N: usize = 5;

/// Base seed derived from the JIT issue ID `96dcbec4`.
const SEED_BASE: u64 = 0x96dc_bec4_0000_0000;

/// Paper-published mean slope (nats/n), computed from Table 2 of
/// Scheinerman 2024 (arxiv 2407.20205v2), `permanent_mod3` column,
/// `n ∈ {24, …, 36}`. Equals `ln(2) ≈ 0.6931` as expected for `O(n·2^n)`.
const PAPER_SLOPE: f64 = 0.693;

/// ±10% tolerance expressed as absolute bounds on the observed slope.
const SLOPE_LO: f64 = 0.624;
const SLOPE_HI: f64 = 0.762;

/// Convert Unix epoch seconds to a `YYYY-MM-DD` UTC date string.
///
/// Inline implementation to avoid pulling `chrono` or `time` as deps for a
/// test-time example. Algorithm from Howard Hinnant's date library
/// (<https://howardhinnant.github.io/date_algorithms.html>): civil-from-days.
fn unix_secs_to_ymd(secs: i64) -> (i32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468; // shift to civil epoch (0000-03-01)
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32; // day-of-era [0..146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // year-of-era
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year (Mar = 0)
    let mp = (5 * doy + 2) / 153; // month-of-year (Mar = 0)
    let d = doy - (153 * mp + 2) / 5 + 1; // day-of-month
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

fn main() {
    // n sweep: 9 points in {8, 10, …, 24}.
    // - n=24 is the bottom of the paper's Table 2 range; included so the
    //   sweep partially overlaps the paper's domain.
    // - n=8..22 is cheap enough for SAMPLES_PER_N=5 and gives strong
    //   log-linear regression statistics (R² typically > 0.999).
    let n_values: &[usize] = &[8, 10, 12, 14, 16, 18, 20, 22, 24];

    let date = today_yyyy_mm_dd();
    let csv_dir = "dev/benchmarks/gf2_algebra_permanent";
    let csv_path = format!("{csv_dir}/paper_repro_slope-{date}.csv");

    fs::create_dir_all(csv_dir).expect("create benchmarks dir");
    let mut csv = File::create(&csv_path).expect("create CSV");
    writeln!(csv, "n,mean_us,std_us,samples,input_hash").unwrap();

    println!("Sa (jit:96dcbec4) — paper Table 2 slope reproduction");
    println!("Sweep: n ∈ {:?}, {} samples each", n_values, SAMPLES_PER_N);
    println!("{:-<78}", "");

    let mut points: Vec<(f64, f64)> = Vec::new(); // (n as f64, ln(mean_us))

    for &n in n_values {
        let mut samples: Vec<f64> = Vec::with_capacity(SAMPLES_PER_N);
        let mut hasher = Sha256::new();
        // SHA-256 input fingerprint: hash (n as u64 LE, seed as u64 LE,
        // sample_idx as u64 LE, matrix entries as u8s) across every sample.
        // Bit-reproducible across runs given the same seeds and matrices.
        hasher.update((n as u64).to_le_bytes());

        for sample_idx in 0..SAMPLES_PER_N {
            let seed = SEED_BASE
                .wrapping_add(n as u64)
                .wrapping_mul(1_000_003)
                .wrapping_add(sample_idx as u64);
            hasher.update(seed.to_le_bytes());
            hasher.update((sample_idx as u64).to_le_bytes());

            let row_major = random_matrix::<3>(n, seed);
            for entry in &row_major {
                hasher.update([entry.value() as u8]);
            }

            let t0 = Instant::now();
            let _ = std::hint::black_box(permanent_mod3_reference(&row_major, n));
            let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
            samples.push(elapsed_us);
        }

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance =
            samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64;
        let std = variance.sqrt();
        let hash_hex = hex_lower(&hasher.finalize());

        writeln!(csv, "{n},{mean:.3},{std:.3},{},{hash_hex}", samples.len()).unwrap();
        println!(
            "n={n:3}  mean={mean:10.3} us  std={std:8.3} us  hash={}…{}",
            &hash_hex[0..8],
            &hash_hex[56..]
        );

        points.push((n as f64, mean.ln()));
    }

    // OLS fit of ln(mean_us) = intercept + slope * n.
    let n_pts = points.len() as f64;
    let sx: f64 = points.iter().map(|(x, _)| x).sum();
    let sy: f64 = points.iter().map(|(_, y)| y).sum();
    let sxx: f64 = points.iter().map(|(x, _)| x * x).sum();
    let sxy: f64 = points.iter().map(|(x, y)| x * y).sum();
    let denom = n_pts * sxx - sx * sx;
    let slope = (n_pts * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / n_pts;

    let r_sq = {
        let y_mean = sy / n_pts;
        let ss_tot: f64 = points.iter().map(|(_, y)| (y - y_mean).powi(2)).sum();
        let ss_res: f64 = points
            .iter()
            .map(|(x, y)| (y - (intercept + slope * x)).powi(2))
            .sum();
        if ss_tot == 0.0 {
            1.0
        } else {
            1.0 - ss_res / ss_tot
        }
    };

    let residual = slope / PAPER_SLOPE;

    println!("{:-<78}", "");
    println!("observed slope = {slope:.4} nats/n  intercept = {intercept:.4}  R² = {r_sq:.4}");
    println!("paper slope    = {PAPER_SLOPE:.3} nats/n  (ln 2 ≈ 0.6931, O(n·2^n))");
    println!("residual ratio = observed / paper = {residual:.4}  (criterion: [0.90, 1.10])");
    println!("CSV written to: {csv_path}");

    let ok = (SLOPE_LO..=SLOPE_HI).contains(&slope);
    if ok {
        println!("PASS: slope {slope:.4} ∈ [{SLOPE_LO:.3}, {SLOPE_HI:.3}]");
    } else {
        eprintln!("FAIL: observed slope {slope:.4} is OUTSIDE ±10% of paper's {PAPER_SLOPE:.3}");
        std::process::exit(1);
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
