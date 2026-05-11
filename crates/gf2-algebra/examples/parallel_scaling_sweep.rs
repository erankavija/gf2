//! S2 (jit:4513209c): Parallel scaling sweep for `permanent_bipedal3_parallel`.
//!
//! Measures wall-clock time of `permanent_bipedal3_parallel` across rayon thread
//! counts T ∈ {1, 2, 4, 8, 12} and matrix dimensions n ∈ {28, 32, 36}, computing
//! the per-matrix scaling factor `T_1[k] / (T × T_T[k])` for each independent
//! matrix `k`, then aggregating to a mean and a two-sided 95% CI per (n, T).
//!
//! # Success criterion (verbatim from JIT 4513209c)
//!
//! For n ∈ {28, 32, 36} and T ∈ {2, 4, 8, 12}: scaling factor `T_1 / (T × T_T) ≥
//! 0.85` *within 95% CI*. The harness implements this by checking that the
//! **lower bound** of the per-cell two-sided 95% CI on the scaling factor is
//! ≥ 0.85.
//!
//! # Determinism
//!
//! For each n we draw K matrices from a deterministic LCG seeded by the JIT
//! issue ID. The SAME K matrices are timed at every thread count so the
//! per-matrix `Fp<3>` output can be compared bit-for-bit across T values.
//! The harness asserts equality at runtime and panics on mismatch.
//!
//! # CSV output
//!
//! `dev/benchmarks/gf2_algebra_permanent/s2_parallel_scaling-<DATE>.csv`
//! (overridable via `SA_DATE`). Columns:
//!   n, threads, mean_us, std_us, k_matrices, scaling_factor, scaling_ci_lo,
//!   scaling_ci_hi, fp3_result_hex
//!
//! # Hardware fingerprint
//!
//! Recorded in the CSV header block.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-algebra --release --features "parallel test-support" \
//!   --example parallel_scaling_sweep
//! ```

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::permanent_bipedal3_parallel;
use gf2_algebra::testutil::{random_matrix, today_yyyy_mm_dd};
use std::fs::{self, File};
use std::io::Write;
use std::time::Instant;

/// Thread counts to sweep.
const THREAD_COUNTS: &[usize] = &[1, 2, 4, 8, 12];

/// Matrix dimensions to sweep.
const N_VALUES: &[usize] = &[28, 32, 36];

/// Number of independent matrices per (n) bucket. The same K matrices are
/// reused at every thread count so per-matrix scaling factors stay paired.
/// n=36 T=1 is ~150 s/matrix; K=3 keeps n=36 inside ~12 min.
const K_N28: usize = 5;
const K_N32: usize = 5;
const K_N36: usize = 3;

/// Fixed RNG base seed per n. Per-matrix seed is `base ^ (k as u64)`.
/// Seeds derived from the JIT issue ID `4513209c`.
const SEED_BASES: &[(usize, u64)] = &[
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

fn k_for_n(n: usize) -> usize {
    match n {
        28 => K_N28,
        32 => K_N32,
        36 => K_N36,
        _ => 3,
    }
}

fn seed_base_for_n(n: usize) -> u64 {
    SEED_BASES
        .iter()
        .find(|(k, _)| *k == n)
        .map(|(_, s)| *s)
        .unwrap_or(0x4513_209c_0000_0000u64.wrapping_add(n as u64))
}

/// Two-sided 95% Student's-t critical value for `df` degrees of freedom.
/// Hand-coded for the small df values the harness uses (`K - 1 ∈ {2, 4}`).
fn t_critical_95(df: usize) -> f64 {
    match df {
        2 => 4.302653,
        4 => 2.776445,
        _ => panic!("t_critical_95: unsupported df={df}; add the value to the lookup table"),
    }
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
    writeln!(
        csv,
        "# rng: gf2_core::rng::Lcg (per-matrix seed = base ^ k)"
    )
    .unwrap();
    for &(n, seed) in SEED_BASES {
        writeln!(csv, "# seed_base_n{n}: {seed:#018x}").unwrap();
    }
    writeln!(csv, "# K_n28: {K_N28}, K_n32: {K_N32}, K_n36: {K_N36}").unwrap();
    writeln!(csv, "# thread_counts: {THREAD_COUNTS:?}").unwrap();
    writeln!(
        csv,
        "n,threads,mean_us,std_us,k_matrices,scaling_factor,scaling_ci_lo,scaling_ci_hi,fp3_result_hex"
    )
    .unwrap();

    println!("S2 (jit:4513209c) — parallel permanent scaling sweep");
    println!("Host: {HW_MODEL}");
    println!("Physical cores: {HW_PHYSICAL_CORES}, SMT: {HW_SMT}");
    println!("Thread counts: {THREAD_COUNTS:?}");
    println!("n values: {N_VALUES:?}");
    println!();

    for &n in N_VALUES {
        let base_seed = seed_base_for_n(n);
        let k = k_for_n(n);
        let total_subsets = (1u64 << n).saturating_sub(1);

        println!("=== n={n}: K={k} independent matrices, base seed={base_seed:#018x} ===");

        // Build K independent matrices once; the same K matrices are timed at
        // every thread count so per-matrix scaling factors stay paired.
        let matrices: Vec<Bipedal3Matrix> = (0..k)
            .map(|i| {
                let seed = base_seed ^ (i as u64);
                let row_major = random_matrix::<3>(n, seed);
                Bipedal3Matrix::from_row_major(&row_major, n, n)
            })
            .collect();

        // Reference fp3 values per matrix (computed at T=1 to check determinism).
        let mut ref_fp3: Option<Vec<u64>> = None;

        // timings[k][t_idx] = wall-clock microseconds for matrix k, thread count t.
        let mut timings_per_matrix: Vec<Vec<f64>> =
            vec![Vec::with_capacity(THREAD_COUNTS.len()); k];

        for &t in THREAD_COUNTS {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(t)
                .build()
                .expect("failed to build rayon thread pool");

            let mut fp3_this_t: Vec<u64> = Vec::with_capacity(k);
            let mut elapsed_this_t: Vec<f64> = Vec::with_capacity(k);

            for (i, mat) in matrices.iter().enumerate() {
                let t0 = Instant::now();
                let result =
                    pool.install(|| std::hint::black_box(permanent_bipedal3_parallel(mat)));
                let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
                elapsed_this_t.push(elapsed_us);
                fp3_this_t.push(result.value());
                timings_per_matrix[i].push(elapsed_us);
            }

            // Determinism: each matrix must produce the same Fp<3> across all T.
            match ref_fp3 {
                None => ref_fp3 = Some(fp3_this_t.clone()),
                Some(ref refv) => {
                    for (i, &v) in fp3_this_t.iter().enumerate() {
                        assert_eq!(
                            v, refv[i],
                            "DETERMINISM FAILURE: n={n}, T={t}, matrix={i}: got fp3={v}, expected {}",
                            refv[i]
                        );
                    }
                }
            }

            let mean_us = elapsed_this_t.iter().sum::<f64>() / k as f64;
            let throughput = total_subsets as f64 / (mean_us * 1e-6);
            println!(
                "  T={t:2}  mean={mean_us:>12.1} us  tput={throughput:.3e} subsets/s  K={k} matrices"
            );
        }
        println!();

        // Now compute per-cell scaling factor with 95% CI using paired
        // per-matrix timings: scaling[k][t_idx] = t1[k] / (t · t_t[k]).
        let df = k - 1;
        let t_crit = t_critical_95(df);

        // T=1 is the reference column.
        let t1_idx = THREAD_COUNTS.iter().position(|&t| t == 1).unwrap();

        println!("  Scaling factors (criterion: lower 95% CI bound ≥ 0.85 for T ∈ {{2,4,8,12}}):");
        println!(
            "  {:>4}  {:>12}  {:>10}  {:>10}  {:>10}  {:>10}  {:>4}",
            "T", "mean_us", "std_us", "scaling", "ci_lo_95", "ci_hi_95", "PASS?"
        );

        let mut all_pass = true;
        for (t_idx, &t) in THREAD_COUNTS.iter().enumerate() {
            // Per-cell mean + std of raw timings (for reporting).
            let timings_at_t: Vec<f64> = timings_per_matrix.iter().map(|row| row[t_idx]).collect();
            let mean_us = timings_at_t.iter().sum::<f64>() / k as f64;
            let std_us = (timings_at_t
                .iter()
                .map(|x| (x - mean_us).powi(2))
                .sum::<f64>()
                / df as f64)
                .sqrt();

            // Per-matrix scaling factor; aggregate to mean + 95% CI on the mean.
            let per_matrix_scaling: Vec<f64> = (0..k)
                .map(|i| {
                    let t1 = timings_per_matrix[i][t1_idx];
                    let tt = timings_per_matrix[i][t_idx];
                    if t == 1 {
                        1.0
                    } else {
                        t1 / (t as f64 * tt)
                    }
                })
                .collect();

            let scaling_mean = per_matrix_scaling.iter().sum::<f64>() / k as f64;
            let scaling_var = per_matrix_scaling
                .iter()
                .map(|x| (x - scaling_mean).powi(2))
                .sum::<f64>()
                / df as f64;
            let scaling_std = scaling_var.sqrt();
            let se = scaling_std / (k as f64).sqrt();
            let margin = t_crit * se;
            let ci_lo = scaling_mean - margin;
            let ci_hi = scaling_mean + margin;

            let pass = t == 1 || ci_lo >= 0.85;
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
                "  {:>4}  {:>12.1}  {:>10.1}  {:>10.4}  {:>10.4}  {:>10.4}  {:>5}",
                t, mean_us, std_us, scaling_mean, ci_lo, ci_hi, pass_str
            );

            // Determine the canonical fp3 across all matrices for this n.
            // (Already determinism-checked; we report the first matrix's value.)
            let fp3_val = ref_fp3.as_ref().unwrap()[0];

            writeln!(
                csv,
                "{n},{t},{mean_us:.3},{std_us:.3},{k},{scaling_mean:.6},{ci_lo:.6},{ci_hi:.6},{fp3_val:#x}"
            )
            .unwrap();
        }

        println!();
        if all_pass {
            println!("  n={n}: ALL scaling-CI criteria PASS (lower 95% CI ≥ 0.85).");
        } else {
            println!("  n={n}: SOME scaling-CI criteria FAIL — check individual rows.");
        }
        println!();
    }

    println!("CSV written to: {csv_path}");
}
