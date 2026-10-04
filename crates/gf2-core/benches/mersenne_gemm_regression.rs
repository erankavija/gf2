//! Benchmarks `Fp<MERSENNE_31>` GEMM at `n = 256` under the single Criterion
//! group `mersenne_gemm_256_regression`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gfp::Fp;

#[path = "common/seed.rs"]
mod seed;

use seed::{derive_seed, fp_matrix_from_seed, ops_gemm, MASTER_SEED};

/// `2^31 − 1`.
const MERSENNE_31: u64 = 2_147_483_647;

const REGRESSION_N: usize = 256;

fn bench_mersenne_gemm_256(c: &mut Criterion) {
    let mut group = c.benchmark_group("mersenne_gemm_256_regression");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));
    group.throughput(Throughput::Elements(
        ops_gemm(REGRESSION_N, REGRESSION_N, REGRESSION_N) as u64,
    ));

    // Index of 256 in `SQUARE_SIZES` of `fieldmatrix_gemm.rs`, so the operands
    // equal those of its `gemm/Fp_M31/256` cell.
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
