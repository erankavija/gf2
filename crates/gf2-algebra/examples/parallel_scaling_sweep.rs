//! S2 (jit:4513209c): Parallel scaling sweep for `permanent_bipedal3_parallel`.
//!
//! Measures wall-clock time of `permanent_bipedal3_parallel` across rayon thread
//! counts T ∈ {1, 2, 4, 8, 12} and matrix dimensions n ∈ {28, 32, 36}, computing
//! the scaling factor `T_1 / (T × T_T)` per (n, T) cell.
//!
//! # Success criterion (verbatim from JIT 4513209c)
//!
//! For n ∈ {28, 32, 36} and T ∈ {2, 4, 8, 12}: scaling factor `T_1 / (T × T_T) ≥ 0.85`.
//!
//! # Determinism
//!
//! A fixed seed per n is used so the same matrix is timed across all thread
//! counts. The `fp3_result_hex` column records the Fp<3> value (canonical int
//! 0..2) across all T; they must be identical. The code confirms this at runtime
//! and panics if they diverge.
//!
//! # CSV output
//!
//! `dev/benchmarks/gf2_algebra_permanent/s2_parallel_scaling-<DATE>.csv`
//! where `<DATE>` defaults to today's UTC date but can be overridden with
//! the `SA_DATE` environment variable.
//!
//! # Hardware fingerprint
//!
//! Recorded in the CSV header block:
//!   - `# host: AMD Ryzen 9 5900X, 12 physical cores, 24 threads (SMT 2x)`
//!   - `# avx2: yes, avx512: no`
//!   - `# rayon: 1.11.0`
//!   - `# rng: gf2_core::rng::Lcg, seed per n (see RNG_SEEDS below)`
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-algebra --release --features "parallel test-support" \
//!   --example parallel_scaling_sweep
//! # Override the output date:
//! SA_DATE=2026-05-11 cargo run -p gf2-algebra --release \
//!   --features "parallel test-support" --example parallel_scaling_sweep
//! ```

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::permanent_bipedal3_parallel;
use gf2_algebra::testutil::random_matrix;
use std::fs::{self, File};
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Thread counts to sweep.
const THREAD_COUNTS: &[usize] = &[1, 2, 4, 8, 12];

/// Matrix dimensions to sweep.
const N_VALUES: &[usize] = &[28, 32, 36];

/// Number of samples per (n, T) cell.
/// n=36 at T=1 is ~167 s/sample; budget only allows 1-2 samples there.
/// Use 5 samples for n ∈ {28, 32} and 3 samples for n=36 to stay ≤ 25 min total.
const SAMPLES_N28: usize = 5;
const SAMPLES_N32: usize = 5;
const SAMPLES_N36: usize = 3;

/// Fixed RNG seed per n. Same matrix used across all thread counts (determinism).
/// Seeds derived from the JIT issue ID `4513209c`.
const RNG_SEEDS: &[(usize, u64)] = &[
    (28, 0x4513_209c_0000_001c),
    (32, 0x4513_209c_0000_0020),
    (36, 0x4513_209c_0000_0024),
];

/// Rayon version recorded in the header.
const RAYON_VERSION: &str = "1.11.0";

/// Hardware fingerprint (Ryzen 9 5900X, verified via lscpu before running).
const HW_MODEL: &str = "AMD Ryzen 9 5900X 12-Core Processor";
const HW_PHYSICAL_CORES: usize = 12;
const HW_SMT: &str = "2x (24 logical CPUs)";
const HW_AVX2: &str = "yes";
const HW_AVX512: &str = "no";

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

fn samples_for_n(n: usize) -> usize {
    match n {
        28 => SAMPLES_N28,
        32 => SAMPLES_N32,
        36 => SAMPLES_N36,
        _ => 3,
    }
}

fn seed_for_n(n: usize) -> u64 {
    RNG_SEEDS
        .iter()
        .find(|(k, _)| *k == n)
        .map(|(_, s)| *s)
        .unwrap_or(0x4513_209c_0000_0000u64.wrapping_add(n as u64))
}

fn main() {
    let date = today_yyyy_mm_dd();
    let csv_dir = "dev/benchmarks/gf2_algebra_permanent";
    let csv_path = format!("{csv_dir}/s2_parallel_scaling-{date}.csv");

    fs::create_dir_all(csv_dir).expect("create benchmarks dir");
    let mut csv = File::create(&csv_path).expect("create CSV");

    // Header block: hardware fingerprint + config.
    writeln!(csv, "# S2 (jit:4513209c) parallel scaling sweep").unwrap();
    writeln!(csv, "# date: {date}").unwrap();
    writeln!(csv, "# host: {HW_MODEL}").unwrap();
    writeln!(csv, "# physical_cores: {HW_PHYSICAL_CORES}, smt: {HW_SMT}").unwrap();
    writeln!(csv, "# avx2: {HW_AVX2}, avx512: {HW_AVX512}").unwrap();
    writeln!(csv, "# rayon: {RAYON_VERSION}").unwrap();
    writeln!(csv, "# rng: gf2_core::rng::Lcg").unwrap();
    for &(n, seed) in RNG_SEEDS {
        writeln!(csv, "# seed_n{n}: {seed:#018x}").unwrap();
    }
    writeln!(
        csv,
        "# samples_n28: {SAMPLES_N28}, samples_n32: {SAMPLES_N32}, samples_n36: {SAMPLES_N36}"
    )
    .unwrap();
    writeln!(csv, "# thread_counts: {THREAD_COUNTS:?}").unwrap();
    writeln!(
        csv,
        "n,threads,mean_us,std_us,samples,scaling_factor,fp3_result_hex"
    )
    .unwrap();

    println!("S2 (jit:4513209c) — parallel permanent scaling sweep");
    println!("Host: {HW_MODEL}");
    println!("Physical cores: {HW_PHYSICAL_CORES}, SMT: {HW_SMT}");
    println!("AVX2: {HW_AVX2}, AVX-512: {HW_AVX512}");
    println!("Rayon: {RAYON_VERSION}");
    println!("Thread counts: {THREAD_COUNTS:?}");
    println!("n values: {N_VALUES:?}");
    println!();

    for &n in N_VALUES {
        let seed = seed_for_n(n);
        let n_samples = samples_for_n(n);
        let total_subsets = (1u64 << n).saturating_sub(1);

        println!("=== n={n} (seed={seed:#018x}, {n_samples} samples per thread count) ===");

        // Build matrix once; it is the same across all thread counts.
        let row_major = random_matrix::<3>(n, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);

        // Collect (T, mean_us) pairs so we can compute scaling factors after
        // all threads are measured.
        let mut results: Vec<(usize, f64, f64, u64)> = Vec::new(); // (threads, mean_us, std_us, fp3_val)

        // Reference fp3 value (computed at T=1 to check determinism).
        let mut reference_fp3: Option<u64> = None;

        for &t in THREAD_COUNTS {
            // Build a dedicated thread pool for this T.
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(t)
                .build()
                .expect("failed to build rayon thread pool");

            let mut timings: Vec<f64> = Vec::with_capacity(n_samples);
            let mut fp3_val: u64 = 0;

            for _ in 0..n_samples {
                let t0 = Instant::now();
                let result =
                    pool.install(|| std::hint::black_box(permanent_bipedal3_parallel(&mat)));
                let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
                timings.push(elapsed_us);
                fp3_val = result.value(); // Fp<3>.value() returns the canonical u64 (0, 1, or 2)
            }

            let ns = timings.len();
            let mean = timings.iter().sum::<f64>() / ns as f64;
            let variance = if ns > 1 {
                timings.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (ns as f64 - 1.0)
            } else {
                0.0
            };
            let std_dev = variance.sqrt();

            // Determinism check.
            match reference_fp3 {
                None => reference_fp3 = Some(fp3_val),
                Some(ref_val) => {
                    assert_eq!(
                        fp3_val, ref_val,
                        "DETERMINISM FAILURE: n={n}, T={t}: got fp3={fp3_val}, expected {ref_val}"
                    );
                }
            }

            let throughput = total_subsets as f64 / (mean * 1e-6);
            println!(
                "  T={t:2}  mean={mean:>12.1} us  std={std_dev:>10.1} us  tput={throughput:.3e} subsets/s  fp3={fp3_val}"
            );

            results.push((t, mean, std_dev, fp3_val));
        }

        // Compute scaling factors relative to T=1.
        let mean_t1 = results
            .iter()
            .find(|(t, _, _, _)| *t == 1)
            .map(|(_, m, _, _)| *m)
            .unwrap();

        println!();
        println!("  Scaling factors (criterion: ≥ 0.85 for T ∈ {{2,4,8,12}}):");
        println!(
            "  {:>4}  {:>12}  {:>10}  {:>14}  {:>7}  {:>4}",
            "T", "mean_us", "std_us", "scaling_factor", "fp3", "PASS?"
        );

        let mut all_pass = true;
        for &(t, mean, std_dev, fp3_val) in &results {
            let scaling = if t == 1 {
                1.0
            } else {
                mean_t1 / (t as f64 * mean)
            };
            let pass = t == 1 || scaling >= 0.85;
            if !pass {
                all_pass = false;
            }
            let pass_str = if t == 1 {
                "  —  "
            } else if pass {
                " PASS"
            } else {
                " FAIL"
            };
            println!(
                "  {:>4}  {:>12.1}  {:>10.1}  {:>14.4}  {:>7}  {:>5}",
                t, mean, std_dev, scaling, fp3_val, pass_str
            );

            // Write CSV row.
            writeln!(
                csv,
                "{n},{t},{mean:.3},{std_dev:.3},{n_samples},{scaling:.6},{fp3_val:#x}"
            )
            .unwrap();
        }

        println!();
        if all_pass {
            println!("  n={n}: ALL scaling criteria PASS.");
        } else {
            println!("  n={n}: SOME scaling criteria FAIL — check individual rows.");
        }
        println!();
    }

    println!("CSV written to: {csv_path}");
}
