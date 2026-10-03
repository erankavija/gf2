//! Criterion benchmarks for `permanent_mod3_reference` and `permanent_bipedal3`
//! on inputs from [`gf2_algebra::testutil::random_matrix`], seeded per cell.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::{permanent_bipedal3, permanent_mod3_reference};
use gf2_algebra::testutil::random_matrix;

/// Distinct from test seeds, so rotating those leaves the bench inputs
/// unchanged.
const BENCH_SEED: u64 = 0xb315_564a_0000_0000_u64;

fn bench_permanent_mod3_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("permanent_mod3_reference");
    group.sample_size(10); // Criterion's hard minimum.
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(25));

    for n in [8usize, 12, 16, 20] {
        let seed = BENCH_SEED.wrapping_add(n as u64);
        let row_major = random_matrix::<3>(n, seed);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n_val| {
            b.iter(|| permanent_mod3_reference(black_box(&row_major), black_box(n_val)))
        });
    }
    group.finish();
}

fn bench_permanent_bipedal3(c: &mut Criterion) {
    let mut group = c.benchmark_group("permanent_bipedal3");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(25));

    for n in [8usize, 12, 16, 20, 24, 28] {
        let seed = BENCH_SEED
            .wrapping_add(0xbeef_0000u64)
            .wrapping_add(n as u64);
        let row_major = random_matrix::<3>(n, seed);
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
