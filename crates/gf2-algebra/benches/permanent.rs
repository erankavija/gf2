//! Criterion benchmark suite for the permanent algorithm family
//! (epic gf2-algebra-permanent / ae82bd73).
//!
//! Per-group sweep ranges per T10 (b315564a) Amendment 2026-05-11c:
//!   permanent_mod3_reference: n in {8, 12, 16, 20}
//!   permanent_bipedal3:       n in {8, 12, 16, 20, 24, 28}
//!
//! n=32 (~9.4 s/call) and n=36 (~150 s/call) were dropped from the bipedal
//! sweep on 2026-05-11 because Criterion's hard minimum sample_size of 10
//! would push each of those cells past the criterion-4 60 s/cell budget on
//! the dev host (10 * 9.4 s = 94 s and 10 * 150 s = 1500 s respectively).
//! The headline n=36 speedup measurement instead lands in S1's dedicated
//! perf-criterion cell, which uses an iter_custom-style single-iteration
//! timing where multi-hour wall-clock is acceptable.
//!
//! Inputs come from the workspace SSOT helper
//! [`gf2_algebra::testutil::random_matrix`], which uses [`gf2_core::rng::Lcg`]
//! seeded deterministically per cell so consecutive runs on the same hardware
//! reproduce bit-identical inputs and stable timing. See the 2026-05-11b
//! amendment in JIT issue `b315564a` for the rationale (committed-seed
//! reproducibility, Charon/Aeneas extractability, dep minimalism).
//!
//! Per-cell wall-clock budget: criterion 4 contracts each cell under 60 s on
//! the dev host. We use `sample_size(10)` (Criterion's minimum) and tune
//! `warm_up_time(1 s)` + `measurement_time(45 s)` so warm-up + measurement +
//! Criterion overhead stays under 60 s total for every cell in the sweep.
//! Sample timing on AMD Ryzen 9 5900X (release build) measured 2026-05-11:
//!   permanent_mod3_reference n=8/12/16: sub-second to ~10 s — well inside.
//!   permanent_mod3_reference n=20:      ~77 ms mean × ~78 iters × 10 samples
//!                                       ≈ 46 s measurement + 1 s warm-up.
//!   permanent_bipedal3 n=8..24:         microseconds to seconds — well inside.
//!   permanent_bipedal3 n=28:            ~0.59 s mean × 10 samples ≈ 6 s.
//!
//! The headline `permanent_mod3_reference` n=36 (paper's reference workload)
//! and the matching `permanent_bipedal3` n=36 50× speedup measurement land in
//! S1's separate perf-criterion cell, which uses a single-iteration timing
//! where multi-hour wall-clock is acceptable.

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
    group.sample_size(10); // Criterion's hard minimum.
    group.warm_up_time(Duration::from_secs(1)); // trimmed from the 3 s default
    group.measurement_time(Duration::from_secs(45)); // 45 + 1 warm-up + overhead < 60 s/cell

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
    group.measurement_time(Duration::from_secs(45));

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
