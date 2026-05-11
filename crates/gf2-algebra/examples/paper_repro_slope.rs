//! Sa (jit:96dcbec4): Reproduce the paper's Table 2 scaling slope using
//! `permanent_mod3_reference` on the dev host.
//!
//! This example times the in-tree [`permanent_mod3_reference`] (W2-T8, the
//! faithful Rust port of Scheinerman 2024 arxiv 2407.20205v2 Julia listing)
//! over a range of matrix sizes `n`, fits `ln(mean_us) = a + b*n` by ordinary
//! least squares, and checks that the observed slope `b` lies within ±10% of
//! the paper's published slope 0.693 nats/n (Table 2, Appendix B).
//!
//! The paper's Table 2 covers `n ∈ {24, 26, …, 36}` on a 4.20 GHz desktop.
//! Running the Rust port at `n = 36` takes ~hours per matrix on the dev host
//! (Ryzen 9 5900X), so this harness covers `n ∈ {8, 10, 12, 14, 16, 18, 20,
//! 22, 24}` (9 points). The slope is invariant under the absolute
//! multiplicative speedup that separates hosts; this harness only measures the
//! asymptotic exponent, not the constant factor.
//!
//! # Output
//!
//! - Prints per-n timing statistics to stdout.
//! - Writes a CSV to
//!   `dev/benchmarks/gf2_algebra_permanent/paper_repro_slope-YYYY-MM-DD.csv`
//!   with columns `n,mean_us,std_us,samples`.
//! - Prints the observed slope, paper slope, and residual ratio.
//! - Exits with code 1 if the slope falls outside [0.624, 0.762] (±10%
//!   tolerance per the issue criterion).
//!
//! # Reproducibility
//!
//! Each matrix is drawn from [`gf2_algebra::testutil::random_matrix::<3>`]
//! using a fixed seed derived from the issue ID and `(n, sample_idx)`. The
//! matrix inputs are therefore bit-identical across repeated runs; only the
//! timing columns vary (measurement noise, ~5–10%). The RNG is
//! `gf2_core::rng::Lcg` — the workspace SSOT for deterministic generation.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
//! ```

use gf2_algebra::permanent::permanent_mod3_reference;
use gf2_algebra::testutil::random_matrix;
use std::fs::{self, File};
use std::io::Write;
use std::time::Instant;

/// Number of independently seeded matrices timed per `n`.
const SAMPLES_PER_N: usize = 5;

/// Base seed derived from the JIT issue ID `96dcbec4`.
const SEED_BASE: u64 = 0x96dc_bec4_0000_0000;

/// Paper-published mean slope (nats/n), computed from Table 2 of
/// Scheinerman 2024 (arxiv 2407.20205v2), `permanent_mod3` column,
/// `n ∈ {24, …, 36}`. Equals ln(2) ≈ 0.6931 as expected for O(n·2^n).
const PAPER_SLOPE: f64 = 0.693;

/// ±10% tolerance expressed as absolute bounds on the observed slope.
const SLOPE_LO: f64 = 0.624;
const SLOPE_HI: f64 = 0.762;

fn main() {
    // n sweep: covers 9 points in {8, 10, …, 24}.
    // - n=24 is the bottom of the paper's Table 2 range; included so the
    //   sweep partially overlaps the paper's domain.
    // - n=8..22 is cheap enough for SAMPLES_PER_N=5 and gives strong
    //   log-linear regression statistics (R² typically > 0.999).
    let n_values: &[usize] = &[8, 10, 12, 14, 16, 18, 20, 22, 24];

    let date = "2026-05-11";
    let csv_dir = "dev/benchmarks/gf2_algebra_permanent";
    let csv_path = format!("{csv_dir}/paper_repro_slope-{date}.csv");

    fs::create_dir_all(csv_dir).expect("create benchmarks dir");
    let mut csv = File::create(&csv_path).expect("create CSV");
    writeln!(csv, "n,mean_us,std_us,samples").unwrap();

    println!("Sa (jit:96dcbec4) — paper Table 2 slope reproduction");
    println!("Sweep: n ∈ {:?}, {} samples each", n_values, SAMPLES_PER_N);
    println!("{:-<60}", "");

    let mut points: Vec<(f64, f64)> = Vec::new(); // (n as f64, ln(mean_us))

    for &n in n_values {
        let mut samples: Vec<f64> = Vec::with_capacity(SAMPLES_PER_N);
        for sample_idx in 0..SAMPLES_PER_N {
            // Derive a unique seed per (n, sample_idx) from the issue-ID base.
            // Mixing strategy: n anchors the seed to the dimension; sample_idx
            // shifts within the block. Wrapping arithmetic keeps seeds in u64.
            let seed = SEED_BASE
                .wrapping_add(n as u64)
                .wrapping_mul(1_000_003)
                .wrapping_add(sample_idx as u64);

            let row_major = random_matrix::<3>(n, seed);

            let t0 = Instant::now();
            let _ = std::hint::black_box(permanent_mod3_reference(&row_major, n));
            let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
            samples.push(elapsed_us);
        }

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance =
            samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64;
        let std = variance.sqrt();

        writeln!(csv, "{n},{mean:.3},{std:.3},{}", samples.len()).unwrap();
        println!(
            "n={n:3}  mean={mean:10.3} us  std={std:8.3} us  samples={}",
            samples.len()
        );

        points.push((n as f64, mean.ln()));
    }

    // Ordinary least-squares fit of ln(mean_us) = intercept + slope * n.
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

    println!("{:-<60}", "");
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
