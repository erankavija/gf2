//! `SpBitMatrix::matmul` — Criterion benchmarks at the two reference
//! sparsity regimes called out in JIT issue `2403c054`'s success
//! criterion #3:
//!
//! - `(n=1024, density=1/n)` — exactly the "≈ 1 non-zero per row"
//!   regime characteristic of LDPC parity-check fragments.
//! - `(n=4096, density=ln(n)/n)` — the Erdős–Rényi `ln n / n`
//!   density at which a uniform random matrix becomes connected w.h.p.
//!
//! Both cases construct square `n × n` operands `A` and `B` with
//! deterministic SplitMix64-derived seeds, so successive bench runs
//! see byte-identical inputs and the criterion #3 measurement is
//! reproducible.
//!
//! ## Determinism contract
//!
//! Inputs are built via `gf2_core::bench_seed::bitmatrix_sparse_from_seed`,
//! which draws an independent SplitMix64 stream per matrix. The two
//! factors `A` and `B` of each pair use distinct derived seeds (via the
//! shared `derive_seed` mixer), so they are not equal-by-construction —
//! `A · B` exercises the full sparse cross-row XOR-accumulator path
//! rather than collapsing to `A · I`.
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench sparse_matmul --features rand
//! cargo bench -p gf2-core --bench sparse_matmul --features rand -- --test
//! cargo bench -p gf2-core --bench sparse_matmul --features rand -- sparse_matmul/n_1024_density_1_over_n
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[path = "common/seed.rs"]
mod seed;

use seed::{bitmatrix_sparse_from_seed, derive_seed, MASTER_SEED};

/// Reference cells from criterion #3: `(label, n, density)`.
///
/// Density for the second cell is `ln(4096) / 4096`. The literal value
/// is precomputed here (rather than `(4096f64).ln()` at runtime) so the
/// fixture is a pure compile-time constant of `n` and the master seed.
const CELLS: &[(&str, usize, f64)] = &[
    // density = 1 / 1024 ≈ 9.7656e-4
    ("n_1024_density_1_over_n", 1024, 1.0 / 1024.0),
    // density = ln(4096) / 4096 = 8.317766166719343 / 4096 ≈ 2.0307e-3
    ("n_4096_density_lnn_over_n", 4096, 8.317_766_166_719_343 / 4096.0),
];

fn bench_sparse_matmul(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_matmul");
    // Sparse matmul work scales near-linearly with `n²·density² + n`,
    // so a sample size of 10 with a 5 s measurement window keeps total
    // wall time bounded while still yielding a statistically usable
    // median. Mirrors the cadence used by `sparse_spmv` and
    // `sparse_matvec_ldpc_*`.
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    for (cell_idx, &(label, n, density)) in CELLS.iter().enumerate() {
        let lhs_seed = derive_seed(MASTER_SEED, "sparse_matmul_lhs", 0, n as u64, cell_idx as u64);
        let rhs_seed = derive_seed(MASTER_SEED, "sparse_matmul_rhs", 1, n as u64, cell_idx as u64);
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
