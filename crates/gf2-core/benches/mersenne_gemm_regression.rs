//! Mersenne31 GEMM regression guard — issue `3d06224c` (story
//! `cc5de315`, "Protect Mersenne fast path").
//!
//! Sibling of the full-coverage harness in `fieldmatrix_gemm.rs`. Where
//! that bench sweeps every `(field, size)` cell of the `64c88ae4` story
//! matrix, this one is a single, narrow guard against future regressions
//! of the Mersenne31 fast path documented in
//! `dev/bench_results/2026-05-04-609855d9-gfp-by-family.md` § *Mersenne
//! fast path*: at `n = 256³` the gf2-core delayed-reduction kernel +
//! Mersenne-aware reduction is **1.74× ahead of fflas-ffpack 2.5.0**,
//! the only family where gf2-core leads the pinned reference. Sibling
//! issues `662f7a15` and `9e12659b` concurrently extend the dispatch
//! ladder in `crates/gf2-core/src/gfp/simd_ops.rs::SimdVecOps`; the
//! [hard] criterion #2 of `3d06224c` requires that those extensions
//! never re-order or remove the Mersenne31 branch.
//!
//! The benchmark exposes a single Criterion ID — `mersenne_gemm_256_regression` —
//! recording throughput at the canonical `256³` cell so the pinned
//! bench-day comparison stays stable across waves. Throughput is
//! reported via `Throughput::Elements(2 · 256³)` so Criterion prints
//! Gop/s directly.
//!
//! ## Wall-clock contract
//!
//! Single cell, `sample_size = 10`, `measurement_time = 5 s`. On the
//! reference Zen-3 host this completes in ~10–15 s wall-clock. Bench
//! day (`./benchmarks/run.sh --skip-m4ri`) and this guard are decoupled:
//! the SOTA-protocol pinned-container baseline lives in
//! `dev/bench_results/2026-04-26-reference.csv`; this guard is a
//! native (non-pinned) run on the developer host.
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench mersenne_gemm_regression --features rand
//! cargo bench -p gf2-core --bench mersenne_gemm_regression --features rand -- mersenne_gemm_256_regression
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gfp::Fp;

#[path = "common/seed.rs"]
mod seed;

use seed::{derive_seed, fp_matrix_from_seed, ops_gemm, MASTER_SEED};

/// Mersenne31 prime: `2^31 − 1`. Mirrors the `MERSENNE_31` constant in
/// `fieldmatrix_gemm.rs`; intentionally local so this regression guard
/// does not depend on the surrounding sweep's seed/index conventions.
const MERSENNE_31: u64 = 2_147_483_647;

/// Headline cell — matches the pinned-container reference cell in
/// `dev/bench_results/2026-05-04-609855d9-gfp-by-family.md` § *Per-prime
/// measurements / Headline cell*. Locked to 256³ on purpose: the family
/// classification doc selects 256³ as the pivot cell and 1024³ as the
/// regression check; this guard is the fast cell.
const REGRESSION_N: usize = 256;

fn bench_mersenne_gemm_256(c: &mut Criterion) {
    let mut group = c.benchmark_group("mersenne_gemm_256_regression");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));
    group.throughput(Throughput::Elements(
        ops_gemm(REGRESSION_N, REGRESSION_N, REGRESSION_N) as u64,
    ));

    // Reuse the full-sweep seeding so the matrix entries are byte-identical
    // to the canonical `gemm/Fp_M31/256` cell in `fieldmatrix_gemm.rs`.
    // `SQUARE_SIZES.iter().enumerate()` in the sibling bench enumerates
    // 64, 256, 1024, 4096; index 1 is the 256³ cell.
    const SI: u64 = 1;
    let seed_a = derive_seed(MASTER_SEED, "fgemm", 0, SI, 0);
    let seed_b = derive_seed(MASTER_SEED, "fgemm_b", 0, SI, 0);
    let a: FieldMatrix<Fp<MERSENNE_31>> =
        fp_matrix_from_seed::<MERSENNE_31>(REGRESSION_N, REGRESSION_N, seed_a);
    let b: FieldMatrix<Fp<MERSENNE_31>> =
        fp_matrix_from_seed::<MERSENNE_31>(REGRESSION_N, REGRESSION_N, seed_b);

    group.bench_with_input(
        BenchmarkId::new("Fp_M31", REGRESSION_N),
        &REGRESSION_N,
        |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        },
    );
    group.finish();
}

criterion_group! {
    name = mersenne_regression;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_mersenne_gemm_256
}
criterion_main!(mersenne_regression);
