//! Throughput demo: `permanent_bipedal3` vs `permanent_mod3_reference`.
//!
//! Times `permanent_bipedal3` at `n = N_BIPEDAL` and
//! `permanent_mod3_reference` at `n = N_REF` over `BATCH` seeded random
//! matrices each, prints permanents/sec for both, and reports whether the
//! bipedal3 mean lies within ±5% of `S1_MEAN_US_BIPEDAL3_N24`. The matrix
//! generator is inline, so the command needs no feature flags.

#![allow(clippy::cast_precision_loss)]

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_algebra::permanent::permanent_mod3_reference;
use gf2_core::gfp::Fp;
use std::hint::black_box;
use std::time::Instant;

/// Deterministic `F_3` matrix generator (SplitMix64 reduced mod 3), inline so
/// this example needs no `test-support` feature.
fn demo_random_matrix(n: usize, seed: u64) -> Vec<Fp<3>> {
    let mut state = seed;
    (0..n * n)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            Fp::<3>::new(z % 3)
        })
        .collect()
}

const N_BIPEDAL: usize = 24;
const N_REF: usize = 20;
const BATCH: usize = 64;
const SEED_BASE: u64 = 0x16f0373400000000;

const S1_MEAN_US_BIPEDAL3_N24: f64 = 213_970.0;
const S1_TOLERANCE: f64 = 0.05;

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn stddev(xs: &[f64], mean: f64) -> f64 {
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / xs.len() as f64;
    var.sqrt()
}

fn main() {
    println!("permanent_demo (jit:16f03734) — bipedal3 vs reference");
    println!("  n_bipedal={N_BIPEDAL}  n_ref={N_REF}  batch={BATCH}");
    println!("  seed_base: {SEED_BASE:#018x}");
    println!();

    let bipedal_matrices: Vec<Bipedal3Matrix> = (0..BATCH)
        .map(|i| {
            let seed = SEED_BASE ^ (N_BIPEDAL as u64).wrapping_shl(8) ^ i as u64;
            let row_major: Vec<Fp<3>> = demo_random_matrix(N_BIPEDAL, seed);
            Bipedal3Matrix::from_row_major(&row_major, N_BIPEDAL, N_BIPEDAL)
        })
        .collect();

    let ref_matrices: Vec<Vec<Fp<3>>> = (0..BATCH)
        .map(|i| {
            let seed = SEED_BASE ^ (N_REF as u64).wrapping_shl(8) ^ i as u64;
            demo_random_matrix(N_REF, seed)
        })
        .collect();

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

    println!("=== Summary ===");
    println!(
        "  permanent_bipedal3     n={N_BIPEDAL}: {bipedal_mean_us:>10.0} µs/matrix  ({bipedal_perm_per_sec:.3} perm/s)"
    );
    println!(
        "  permanent_mod3_reference n={N_REF}: {ref_mean_us:>10.0} µs/matrix  ({ref_perm_per_sec:.3} perm/s)"
    );
    println!();

    println!(
        "  Same-n speedup at n=24 (from S1 CSV, 2026-05-11): 6.888x  \
         (bipedal3_simd=213 970 µs vs reference=1 473 800 µs)"
    );
    println!();

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

    let seed0 = SEED_BASE ^ (N_BIPEDAL as u64).wrapping_shl(8);
    let mat0_row = demo_random_matrix(N_BIPEDAL, seed0);
    let mat0 = Bipedal3Matrix::from_row_major(&mat0_row, N_BIPEDAL, N_BIPEDAL);
    let repro = permanent_bipedal3(&mat0).value();
    assert_eq!(
        repro, bipedal_results[0],
        "permanent_bipedal3 is not deterministic — same seed must produce same result"
    );
    println!("Determinism check: perm(matrix[0]) = {repro}  (same seed → same value) OK");
}
