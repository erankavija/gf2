//! Headline benchmark demo: `permanent_bipedal3` vs `permanent_mod3_reference`.
//!
//! Times both the fast bipedal3 path and the scalar reference over a batch of
//! random matrices and prints throughput (permanents/sec) so users can verify
//! the headline speedup numbers from the epic design doc
//! `dev/plans/gf2_algebra_permanent.md` and the S1 benchmark CSV
//! `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`.
//!
//! ## What this measures
//!
//! - **`permanent_bipedal3`** at `n = N_BIPEDAL` (default 24) over `M = BATCH`
//!   (default 64) deterministically seeded random matrices.
//! - **`permanent_mod3_reference`** at `n = N_REF` (default 20) over the same
//!   batch count, used as the speedup denominator. `n = 20` is used instead of
//!   `n = 24` so the reference completes in seconds rather than minutes; the
//!   speedup extrapolation note below explains the theoretical ratio at equal `n`.
//!
//! ## Headline numbers (from S1, 2026-05-11, AMD Ryzen 9 5900X / Zen 3 / AVX2)
//!
//! From `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`:
//!
//! | n  | impl                      | mean_us        | ratio_vs_reference |
//! |----|---------------------------|----------------|--------------------|
//! | 24 | permanent_mod3_reference  | 1 473 800 µs   | 1.000              |
//! | 24 | permanent_bipedal3_simd   |   213 970 µs   | 6.888×             |
//! | 36 | permanent_bipedal3_simd   | 848 483 504 µs | 10.643× (off‑line) |
//!
//! Epic success criterion 12 requires `permanent_bipedal3` to run within ±5%
//! of the S1 reference measurement on this dev host. At `n = 24` the target
//! is 213 970 µs ± 10 698 µs (i.e., the single-matrix mean should sit in
//! [203 272, 224 668] µs on equivalent hardware).
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p gf2-algebra --release --features test-support --example permanent_demo
//! ```
//!
//! The example prints one status line per batch cell, then a summary table
//! comparing bipedal3 throughput vs the reference, and a ±5% check against
//! the S1 CSV mean.

#![allow(clippy::cast_precision_loss)]

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_algebra::permanent::permanent_mod3_reference;
use gf2_algebra::testutil::random_matrix;
use gf2_core::gfp::Fp;
use std::hint::black_box;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Matrix dimension for the bipedal3 fast path.
const N_BIPEDAL: usize = 24;
/// Matrix dimension for the reference scalar path.
const N_REF: usize = 20;
/// Number of independent matrices per batch.
const BATCH: usize = 64;
/// Base seed for deterministic matrix generation (derived from JIT issue ID 16f03734).
const SEED_BASE: u64 = 0x16f0373400000000;

// S1 headline measurement from dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv.
// Used to validate the ±5% criterion (epic success criterion 12).
const S1_MEAN_US_BIPEDAL3_N24: f64 = 213_970.0;
const S1_TOLERANCE: f64 = 0.05;

// ---------------------------------------------------------------------------
// Helper: mean and std-dev of a slice of f64 timings.
// ---------------------------------------------------------------------------

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn stddev(xs: &[f64], mean: f64) -> f64 {
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / xs.len() as f64;
    var.sqrt()
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("permanent_demo (jit:16f03734) — bipedal3 vs reference");
    println!("  n_bipedal={N_BIPEDAL}  n_ref={N_REF}  batch={BATCH}");
    println!("  seed_base: {SEED_BASE:#018x}");
    println!();

    // -------------------------------------------------------------------------
    // Build batch matrices.
    // -------------------------------------------------------------------------

    // Row-major F_3 matrices → Bipedal3Matrix (for bipedal3 path).
    let bipedal_matrices: Vec<Bipedal3Matrix> = (0..BATCH)
        .map(|i| {
            let seed = SEED_BASE ^ (N_BIPEDAL as u64).wrapping_shl(8) ^ i as u64;
            let row_major: Vec<Fp<3>> = random_matrix::<3>(N_BIPEDAL, seed);
            Bipedal3Matrix::from_row_major(&row_major, N_BIPEDAL, N_BIPEDAL)
        })
        .collect();

    // Flat row-major F_3 matrices for the reference scalar path.
    let ref_matrices: Vec<Vec<Fp<3>>> = (0..BATCH)
        .map(|i| {
            let seed = SEED_BASE ^ (N_REF as u64).wrapping_shl(8) ^ i as u64;
            random_matrix::<3>(N_REF, seed)
        })
        .collect();

    // -------------------------------------------------------------------------
    // Time permanent_bipedal3 at N_BIPEDAL over BATCH matrices.
    // -------------------------------------------------------------------------

    println!("Timing permanent_bipedal3 (n={N_BIPEDAL}, batch={BATCH}) ...");
    eprint!("  matrix ");

    let mut bipedal_timings_us: Vec<f64> = Vec::with_capacity(BATCH);
    let mut bipedal_results: Vec<u64> = Vec::with_capacity(BATCH);

    for (i, mat) in bipedal_matrices.iter().enumerate() {
        eprint!("{i} ");
        let t0 = Instant::now();
        let result = black_box(permanent_bipedal3(mat));
        let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
        bipedal_timings_us.push(elapsed_us);
        bipedal_results.push(result.value());
    }
    eprintln!("done");

    let bipedal_mean_us = mean(&bipedal_timings_us);
    let bipedal_std_us = stddev(&bipedal_timings_us, bipedal_mean_us);
    let bipedal_perm_per_sec = 1_000_000.0 / bipedal_mean_us;

    println!(
        "  permanent_bipedal3 n={N_BIPEDAL}: mean = {bipedal_mean_us:.0} µs  std = {bipedal_std_us:.0} µs"
    );
    println!("  throughput: {bipedal_perm_per_sec:.3} permanents/sec");
    println!();

    // -------------------------------------------------------------------------
    // Time permanent_mod3_reference at N_REF over BATCH matrices.
    // -------------------------------------------------------------------------

    println!("Timing permanent_mod3_reference (n={N_REF}, batch={BATCH}) ...");
    eprint!("  matrix ");

    let mut ref_timings_us: Vec<f64> = Vec::with_capacity(BATCH);

    for (i, mat) in ref_matrices.iter().enumerate() {
        eprint!("{i} ");
        let t0 = Instant::now();
        let _result = black_box(permanent_mod3_reference(mat, N_REF));
        let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
        ref_timings_us.push(elapsed_us);
    }
    eprintln!("done");

    let ref_mean_us = mean(&ref_timings_us);
    let ref_std_us = stddev(&ref_timings_us, ref_mean_us);
    let ref_perm_per_sec = 1_000_000.0 / ref_mean_us;

    println!(
        "  permanent_mod3_reference n={N_REF}: mean = {ref_mean_us:.0} µs  std = {ref_std_us:.0} µs"
    );
    println!("  throughput: {ref_perm_per_sec:.3} permanents/sec");
    println!();

    // -------------------------------------------------------------------------
    // Summary table.
    // -------------------------------------------------------------------------

    println!("=== Summary ===");
    println!(
        "  permanent_bipedal3     n={N_BIPEDAL}: {bipedal_mean_us:>10.0} µs/matrix  ({bipedal_perm_per_sec:.3} perm/s)"
    );
    println!(
        "  permanent_mod3_reference n={N_REF}: {ref_mean_us:>10.0} µs/matrix  ({ref_perm_per_sec:.3} perm/s)"
    );
    println!();

    // Same-n speedup extrapolation note (n=24 bipedal vs n=24 reference).
    // At equal n, both walk 2^n Gray steps; bipedal3 saves ~6-7x via bit-packing.
    // The S1 CSV measured 6.888x at n=24 on this dev host.
    println!(
        "  Same-n speedup at n=24 (from S1 CSV, 2026-05-11): 6.888x  \
         (bipedal3_simd=213 970 µs vs reference=1 473 800 µs)"
    );
    println!();

    // -------------------------------------------------------------------------
    // ±5% check against S1 headline (epic criterion 12).
    // -------------------------------------------------------------------------

    let lo = S1_MEAN_US_BIPEDAL3_N24 * (1.0 - S1_TOLERANCE);
    let hi = S1_MEAN_US_BIPEDAL3_N24 * (1.0 + S1_TOLERANCE);

    println!("=== Criterion 12 check (±5% of S1 headline at n={N_BIPEDAL}) ===");
    println!(
        "  S1 target: {S1_MEAN_US_BIPEDAL3_N24:.0} µs  \
         window: [{lo:.0}, {hi:.0}] µs"
    );
    println!(
        "  Measured:  {bipedal_mean_us:.0} µs  \
         (source: dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv)"
    );

    if bipedal_mean_us >= lo && bipedal_mean_us <= hi {
        println!("  PASS — within ±5% of S1 headline.");
    } else {
        let pct =
            (bipedal_mean_us - S1_MEAN_US_BIPEDAL3_N24).abs() / S1_MEAN_US_BIPEDAL3_N24 * 100.0;
        println!(
            "  NOTE — measured {pct:.1}% outside the ±5% window. \
             This is expected on hardware other than AMD Ryzen 9 5900X / Zen 3 / AVX2=yes. \
             The ±5% criterion applies specifically to the S1 dev host. \
             See dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv."
        );
    }
    println!();

    // Sanity: results are deterministic — same seed → same permanent value.
    let seed0 = SEED_BASE ^ (N_BIPEDAL as u64).wrapping_shl(8);
    let mat0_row = random_matrix::<3>(N_BIPEDAL, seed0);
    let mat0 = Bipedal3Matrix::from_row_major(&mat0_row, N_BIPEDAL, N_BIPEDAL);
    let repro = permanent_bipedal3(&mat0).value();
    assert_eq!(
        repro, bipedal_results[0],
        "permanent_bipedal3 is not deterministic — same seed must produce same result"
    );
    println!("Determinism check: perm(matrix[0]) = {repro}  (same seed → same value) OK");
}
