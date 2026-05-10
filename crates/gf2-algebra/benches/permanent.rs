//! Criterion benchmark suite for the permanent algorithm family
//! (epic gf2-algebra-permanent / ae82bd73).
//!
//! Per-group sweep ranges per T10 (b315564a) Amendment 2026-05-11:
//!   permanent_mod3_reference: n in {8, 12, 16, 20, 24}
//!   permanent_bipedal3:       n in {8, 12, 16, 20, 24, 28, 32, 36}
//!
//! Inputs are generated from a fixed-seed `gf2_core::rng::Lcg` (the
//! workspace SSOT RNG) so consecutive runs on the same hardware reproduce
//! bit-identical inputs and stable timing.
//!
//! Per-cell wall-clock expectations (AMD Ryzen 9 5900X, release build):
//!   permanent_mod3_reference n=8:  < 1 ms  => well inside 60 s budget
//!   permanent_mod3_reference n=12: ~10 ms  => well inside 60 s budget
//!   permanent_mod3_reference n=16: ~1 s    => ~10 s per cell at sample_size=10
//!   permanent_mod3_reference n=20: ~30 s   => ~300 s per cell — measurement_time=60s
//!                                             cap makes Criterion exit after ~2 samples
//!   permanent_mod3_reference n=24: ~8 s    => ~80 s per cell at sample_size=10;
//!                                             measurement_time=60s cap exits after ~7 samples.
//!                                             Borderline; may see "only N samples" warning.
//!   permanent_bipedal3 n=36:       ~10-30 s per call => at sample_size=10 fits ~60 s
//!                                  with measurement_time cap.
//!
//! The `random_matrix_fp3` helper below is intentionally inlined rather than
//! re-exported from `gf2_algebra::testutil::random_matrix` because that
//! module is `#[cfg(test)]`-gated and therefore not visible to bench targets.
//! The logic is identical; if `testutil` is ever made pub, this helper can be
//! deleted and replaced with a direct call.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::{permanent_bipedal3, permanent_mod3_reference};
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

/// Workspace SSOT seed for this bench file. Distinct from test seeds so
/// the bench fingerprint does not change when test seeds are rotated.
const BENCH_SEED: u64 = 0xb315_564a_0000_0000_u64;

/// Generate a deterministic pseudo-random `n × n` matrix of `Fp<3>` elements,
/// row-major. Mirrors `gf2_algebra::testutil::random_matrix::<3>` which is
/// `#[cfg(test)]`-gated and not visible here.
fn random_matrix_fp3(n: usize, seed: u64) -> Vec<Fp<3>> {
    let mut rng = Lcg::new(seed);
    (0..n * n)
        .map(|_| Fp::<3>::new(rng.next_u64() % 3))
        .collect()
}

fn bench_permanent_mod3_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("permanent_mod3_reference");
    group.sample_size(10); // criterion-min
    group.measurement_time(Duration::from_secs(60)); // criterion-4 budget cap

    for n in [8usize, 12, 16, 20, 24] {
        let seed = BENCH_SEED.wrapping_add(n as u64);
        let row_major = random_matrix_fp3(n, seed);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n_val| {
            b.iter(|| permanent_mod3_reference(black_box(&row_major), black_box(n_val)))
        });
    }
    group.finish();
}

fn bench_permanent_bipedal3(c: &mut Criterion) {
    let mut group = c.benchmark_group("permanent_bipedal3");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));

    for n in [8usize, 12, 16, 20, 24, 28, 32, 36] {
        let seed = BENCH_SEED
            .wrapping_add(0xbeef_0000u64)
            .wrapping_add(n as u64);
        let row_major = random_matrix_fp3(n, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| permanent_bipedal3(black_box(&mat)))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_permanent_mod3_reference,
    bench_permanent_bipedal3
);
criterion_main!(benches);
