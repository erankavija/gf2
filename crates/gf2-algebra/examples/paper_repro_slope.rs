//! Reproduces the Table 2 scaling slope of `@/citation/Scheinerman2024` with
//! [`permanent_mod3_reference`].
//!
//! Times the function over `n ∈ {8, 10, …, 24}` (the paper's table covers
//! `n ∈ {24, 26, …, 36}`), fits `ln(mean_us) = a + b*n` by ordinary least
//! squares, and exits nonzero unless `b` lies within ±10% of
//! `ln 2 + mean(1/n)`, the slope of an `O(n·2^n)` cost over the sweep.
//!
//! Writes `dev/benchmarks/gf2_algebra_permanent/paper_repro_slope-<DATE>.csv`
//! (date overridable via `SA_DATE`) with columns `n`, `mean_us`, `std_us`,
//! `samples` and `input_hash`: the SHA-256 over `n` and each sample's seed,
//! index and matrix entries, identical across runs.
//!
//! ```bash
//! cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
//! # Override the date in the filename (e.g. for CI):
//! SA_DATE=2026-05-11 cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
//! ```

use gf2_algebra::permanent::permanent_mod3_reference;
use gf2_algebra::testutil::{random_matrix, today_yyyy_mm_dd};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::time::Instant;

/// Number of independently seeded matrices timed per `n`.
const SAMPLES_PER_N: usize = 5;

/// Target wall-clock per timed window, in microseconds. For each sample the
/// example computes `inner_iters = max(1, TARGET_US / first_call_us)` so the
/// timed window is at least this long. This amortises OS-scheduling noise at
/// small `n` where a single call is otherwise sub-millisecond.
const TARGET_US: f64 = 100_000.0; // 100 ms per timed window

const SEED_BASE: u64 = 0x96dc_bec4_0000_0000;

/// Asymptotic slope (nats/n) at the limit $n \to \infty$ for the
/// $O(n \cdot 2^n)$ algorithm: $\ln 2 \approx 0.6931$.
const PAPER_ASYMPTOTIC_SLOPE: f64 = std::f64::consts::LN_2;

/// ±10% tolerance fraction; applied against the range-adjusted reference
/// (`ln(2) + mean(1/n)` over the sweep).
const SLOPE_TOLERANCE: f64 = 0.10;

fn main() {
    // n=24 is the bottom of the paper's Table 2 range, so the sweep overlaps it.
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
        hasher.update((n as u64).to_le_bytes());

        // Calibrate inner-iteration count: a single call at sample_idx=0,
        // then choose inner_iters so the timed window is ≥ TARGET_US.
        let calibration_seed = SEED_BASE.wrapping_add(n as u64).wrapping_mul(1_000_003);
        let calibration_matrix = random_matrix::<3>(n, calibration_seed);
        let t_cal = Instant::now();
        let _ = std::hint::black_box(permanent_mod3_reference(&calibration_matrix, n));
        let single_call_us = t_cal.elapsed().as_secs_f64() * 1_000_000.0;
        let inner_iters = ((TARGET_US / single_call_us).ceil() as usize).max(1);

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
            for _ in 0..inner_iters {
                let _ = std::hint::black_box(permanent_mod3_reference(&row_major, n));
            }
            let elapsed_us = (t0.elapsed().as_secs_f64() * 1_000_000.0) / inner_iters as f64;
            samples.push(elapsed_us);
        }

        let n_samples = samples.len();
        let mean = samples.iter().sum::<f64>() / n_samples as f64;
        // Bessel-corrected sample variance (n-1 in the denominator) so the
        // std_us column matches the canonical unbiased-estimator definition.
        let variance = if n_samples > 1 {
            samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n_samples as f64 - 1.0)
        } else {
            0.0
        };
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

    // Range-adjusted reference: for an O(n·2^n) algorithm, the integrated
    // slope over [n_min, n_max] equals ln(2) + mean(1/n) over the sweep.
    let mean_inv_n: f64 =
        n_values.iter().map(|&n| 1.0 / n as f64).sum::<f64>() / n_values.len() as f64;
    let reference_slope = PAPER_ASYMPTOTIC_SLOPE + mean_inv_n;
    let slope_lo = reference_slope * (1.0 - SLOPE_TOLERANCE);
    let slope_hi = reference_slope * (1.0 + SLOPE_TOLERANCE);
    let residual = slope / reference_slope;

    println!("{:-<78}", "");
    println!("observed slope     = {slope:.4} nats/n  intercept = {intercept:.4}  R² = {r_sq:.4}");
    println!("paper asymptotic   = {PAPER_ASYMPTOTIC_SLOPE:.4} nats/n  (ln 2, n → ∞)");
    println!("mean(1/n) over sweep = {mean_inv_n:.4}");
    println!("range-adjusted ref = {reference_slope:.4} nats/n  (= ln 2 + mean(1/n))");
    println!(
        "residual ratio     = observed / reference = {residual:.4}  (criterion: [{:.2}, {:.2}])",
        1.0 - SLOPE_TOLERANCE,
        1.0 + SLOPE_TOLERANCE
    );
    println!("CSV written to: {csv_path}");

    let ok = (slope_lo..=slope_hi).contains(&slope);
    if ok {
        println!("PASS: slope {slope:.4} ∈ [{slope_lo:.4}, {slope_hi:.4}]");
    } else {
        eprintln!(
            "FAIL: observed slope {slope:.4} is OUTSIDE ±{}% of range-adjusted reference {reference_slope:.4}",
            (SLOPE_TOLERANCE * 100.0) as u32
        );
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

#[cfg(test)]
mod tests {
    use super::hex_lower;

    /// `hex_lower` produces the canonical lowercase-hex SHA-256 encoding.
    #[test]
    fn test_hex_lower() {
        assert_eq!(hex_lower(&[]), "");
        assert_eq!(hex_lower(&[0x00]), "00");
        assert_eq!(hex_lower(&[0xff]), "ff");
        assert_eq!(hex_lower(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        // Length × 2.
        assert_eq!(hex_lower(&[0; 32]).len(), 64);
    }
}
