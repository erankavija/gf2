//! Criterion benchmark suite for the permanent algorithm family
//! (epic gf2-algebra-permanent / ae82bd73).
//!
//! Per-group sweep ranges per T10 (b315564a) Amendment 2026-05-11b:
//!   permanent_mod3_reference: n in {8, 12, 16, 20}
//!   permanent_bipedal3:       n in {8, 12, 16, 20, 24, 28, 32, 36}
//!
//! Inputs come from the workspace SSOT helper
//! [`gf2_algebra::testutil::random_matrix`], which uses [`gf2_core::rng::Lcg`]
//! seeded deterministically per cell so consecutive runs on the same hardware
//! reproduce bit-identical inputs and stable timing. See the 2026-05-11b
//! amendment in JIT issue `b315564a` for the rationale (committed-seed
//! reproducibility, Charon/Aeneas extractability, dep minimalism).
//!
//! Per-cell wall-clock expectations (AMD Ryzen 9 5900X, release build):
//!   permanent_mod3_reference n=8:  < 1 ms    => well inside 60 s budget
//!   permanent_mod3_reference n=12: ~10 ms    => well inside 60 s budget
//!   permanent_mod3_reference n=16: ~1 s      => ~10 s per cell at sample_size=10
//!   permanent_mod3_reference n=20: ~30 s     => measurement_time=60s cap exits
//!                                               after a few samples (within budget).
//!   permanent_bipedal3 n=36:       ~10-30 s  => at sample_size=10 fits ~60 s
//!                                               with measurement_time cap.
//!
//! The headline `permanent_mod3_reference` at n=36 (paper's reference workload)
//! is *not* in this sweep — it lands in S1's separate perf-criterion cell where
//! a multi-hour single-iteration measurement is acceptable.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::{permanent_bipedal3, permanent_mod3_reference};
use gf2_algebra::testutil::random_matrix;

/// Workspace SSOT seed for this bench file. Distinct from test seeds so
/// the bench fingerprint does not change when test seeds are rotated.
const BENCH_SEED: u64 = 0xb315_564a_0000_0000_u64;

fn bench_permanent_mod3_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("permanent_mod3_reference");
    group.sample_size(10); // criterion-min
    group.measurement_time(Duration::from_secs(60)); // criterion-4 budget cap

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
    group.measurement_time(Duration::from_secs(60));

    for n in [8usize, 12, 16, 20, 24, 28, 32, 36] {
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
