//! Benchmarks `SpBitMatrix::matmul` on square operands at densities `1/n`
//! (`n = 1024`) and `ln(n)/n` (`n = 4096`), built from distinct derived seeds
//! through `gf2_core::bench_seed`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[path = "common/seed.rs"]
mod seed;

use seed::{bitmatrix_sparse_from_seed, derive_seed, MASTER_SEED};

/// `(label, n, density)`.
const CELLS: &[(&str, usize, f64)] = &[
    ("n_1024_density_1_over_n", 1024, 1.0 / 1024.0),
    // ln(4096) / 4096
    (
        "n_4096_density_lnn_over_n",
        4096,
        8.317_766_166_719_343 / 4096.0,
    ),
];

fn bench_sparse_matmul(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_matmul");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    for (cell_idx, &(label, n, density)) in CELLS.iter().enumerate() {
        let lhs_seed = derive_seed(
            MASTER_SEED,
            "sparse_matmul_lhs",
            0,
            n as u64,
            cell_idx as u64,
        );
        let rhs_seed = derive_seed(
            MASTER_SEED,
            "sparse_matmul_rhs",
            1,
            n as u64,
            cell_idx as u64,
        );
        let a = bitmatrix_sparse_from_seed(n, n, density, lhs_seed);
        let b = bitmatrix_sparse_from_seed(n, n, density, rhs_seed);

        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(&a, &b),
            |bench, (a, b)| {
                bench.iter(|| {
                    let c = black_box(*a).matmul(black_box(*b));
                    black_box(c);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_sparse_matmul);
criterion_main!(benches);
