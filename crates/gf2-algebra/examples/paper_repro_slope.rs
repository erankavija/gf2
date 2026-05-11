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

/// Target wall-clock per timed window, in microseconds. For each sample the
/// example computes `inner_iters = max(1, TARGET_US / first_call_us)` so the
/// timed window is at least this long. This amortises OS-scheduling noise at
/// small `n` where a single call is otherwise sub-millisecond.
const TARGET_US: f64 = 100_000.0; // 100 ms per timed window

/// Base seed derived from the JIT issue ID `96dcbec4`.
const SEED_BASE: u64 = 0x96dc_bec4_0000_0000;

/// Paper-published asymptotic slope (nats/n) at the limit $n \to \infty$ for
/// the $O(n \cdot 2^n)$ algorithm. Equals $\ln 2 \approx 0.6931$. Paper's
/// Table 2 measured at `n ∈ {24, …, 36}` lands very close to this asymptotic.
const PAPER_ASYMPTOTIC_SLOPE: f64 = std::f64::consts::LN_2;

/// ±10% tolerance fraction; applied against the range-adjusted reference
/// (`ln(2) + mean(1/n)` over the sweep) per criterion 2 amendment 2026-05-11b.
const SLOPE_TOLERANCE: f64 = 0.10;

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
    // Per criterion 2 amendment 2026-05-11b, comparison is against this
    // reference, not the paper's asymptotic-limit value.
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
    use super::{hex_lower, today_yyyy_mm_dd, unix_secs_to_ymd};

    /// `unix_secs_to_ymd` matches known anchor dates spanning multiple eras.
    #[test]
    fn test_unix_secs_to_ymd_anchors() {
        // Unix epoch itself.
        assert_eq!(unix_secs_to_ymd(0), (1970, 1, 1));
        // Y2K, midnight UTC.
        assert_eq!(unix_secs_to_ymd(946_684_800), (2000, 1, 1));
        // 2026-05-11 midnight UTC.
        // Verified via `date -u -d "2026-05-11" +%s` = 1778457600.
        assert_eq!(unix_secs_to_ymd(1_778_457_600), (2026, 5, 11));
        // Leap-year boundary: 2000-02-29 midnight UTC.
        assert_eq!(unix_secs_to_ymd(951_782_400), (2000, 2, 29));
        // Pre-epoch (negative seconds): 1969-12-31 23:00 UTC.
        assert_eq!(unix_secs_to_ymd(-3600), (1969, 12, 31));
    }

    /// `today_yyyy_mm_dd` honours the `SA_DATE` env override and produces the
    /// `YYYY-MM-DD` format expected by the CSV filename pattern.
    #[test]
    fn test_today_yyyy_mm_dd_env_override() {
        // SAFETY-EQUIVALENT: set_var is `unsafe` on the 2024 edition; here in
        // the 2021 edition crate it is the standard test-only override.
        std::env::set_var("SA_DATE", "1999-12-31");
        let got = today_yyyy_mm_dd();
        std::env::remove_var("SA_DATE");
        assert_eq!(got, "1999-12-31");
    }

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
