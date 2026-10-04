//! Benchmarks sparse matrix operations over GF(2). Benchmarks marked
//! `[SAGE_CMP]` have counterparts in `scripts/sage_benchmarks.py`
//! (`@/citation/SageMath2026`).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{
    deterministic_ldpc_like_fixture, deterministic_sparse_bitvec_fixture, SpBitMatrix,
    SpBitMatrixDual,
};
use gf2_core::BitVec;
use rand::SeedableRng;

/// [SAGE_CMP] `sparse_matrix * vector` over GF(2).
fn bench_sparse_matvec(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_matvec");

    for &density in &[0.01, 0.05, 0.10] {
        for &size in &[100, 500, 1000] {
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);
            let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
            let s = SpBitMatrix::from_dense(&m);
            let x = BitVec::random(size, &mut rng);

            group.bench_with_input(
                BenchmarkId::new(format!("density_{:.2}", density), size),
                &(&s, &x),
                |b, (s, x)| b.iter(|| black_box(s.matvec(x))),
            );
        }
    }
    group.finish();
}

fn bench_dense_vs_sparse(c: &mut Criterion) {
    let mut group = c.benchmark_group("dense_vs_sparse_1pct");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
    let s = SpBitMatrix::from_dense(&m);
    let x = BitVec::random(size, &mut rng);

    group.bench_function("sparse_matvec", |b| b.iter(|| black_box(s.matvec(&x))));

    group.bench_function("dense_manual_matvec", |b| {
        b.iter(|| {
            let mut y = BitVec::with_capacity(size);
            for r in 0..size {
                let mut acc = false;
                for c in 0..size {
                    if m.get(r, c) {
                        acc ^= x.get(c);
                    }
                }
                y.push_bit(acc);
            }
            black_box(y)
        })
    });

    group.finish();
}

/// [SAGE_CMP] `matrix.transpose()` for sparse GF(2) matrices.
fn bench_sparse_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_transpose");

    for &density in &[0.01, 0.05] {
        for &size in &[100, 500, 1000] {
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);
            let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
            let s = SpBitMatrix::from_dense(&m);

            group.bench_with_input(
                BenchmarkId::new(format!("density_{:.2}", density), size),
                &s,
                |b, s| b.iter(|| black_box(s.transpose())),
            );
        }
    }
    group.finish();
}

fn bench_dual_col_iter_vs_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_col_access");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);

    let single = SpBitMatrix::from_dense(&m);
    let dual = SpBitMatrixDual::from_dense(&m);

    group.bench_function("single_csr_transpose_per_col", |b| {
        b.iter(|| {
            let mut sum = 0;
            for c in 0..size {
                for _r in single.col_iter(c) {
                    sum += 1;
                }
            }
            black_box(sum)
        })
    });

    group.bench_function("dual_direct_col_access", |b| {
        b.iter(|| {
            let mut sum = 0;
            for c in 0..size {
                for _r in dual.col_iter(c) {
                    sum += 1;
                }
            }
            black_box(sum)
        })
    });

    group.finish();
}

fn bench_bidirectional_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("bidirectional_sweep");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);

    let dual = SpBitMatrixDual::from_dense(&m);

    group.bench_function("alternating_row_col_sweeps", |b| {
        b.iter(|| {
            let mut sum = 0;
            for r in 0..dual.rows() {
                for _c in dual.row_iter(r) {
                    sum += 1;
                }
            }
            for c in 0..dual.cols() {
                for _r in dual.col_iter(c) {
                    sum += 1;
                }
            }
            black_box(sum)
        })
    });

    group.finish();
}

fn bench_dual_matvec_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_transpose_matvec");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);

    let dual = SpBitMatrixDual::from_dense(&m);
    let x = BitVec::random(size, &mut rng);

    group.bench_function("matvec", |b| b.iter(|| black_box(dual.matvec(&x))));

    group.bench_function("matvec_transpose", |b| {
        b.iter(|| black_box(dual.matvec_transpose(&x)))
    });

    group.finish();
}

/// `block_csr_no_prefetch` sets the prefetch distance to 0, isolating the
/// prefetch hint from the block-CSR layout.
fn bench_ldpc_block_csr_matvec(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_matvec_ldpc_block_csr");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(3));

    for &(rows, cols, row_weight) in &[
        (4096usize, 8192usize, 6usize),
        (8192, 16384, 6),
        (4096, 32768, 32),
    ] {
        let csr = deterministic_ldpc_like_fixture(rows, cols, row_weight);
        let block = csr.to_default_block_csr();
        let x = deterministic_sparse_bitvec_fixture(cols);
        let case = format!("{rows}x{cols}_w{row_weight}");

        group.bench_with_input(
            BenchmarkId::new("csr", &case),
            &(&csr, &x),
            |b, (csr, x)| b.iter(|| black_box(csr.matvec(x))),
        );
        group.bench_with_input(
            BenchmarkId::new("block_csr_prefetch", &case),
            &(&block, &x),
            |b, (block, x)| b.iter(|| black_box(block.matvec_with_prefetch_distance(x, 16))),
        );
        group.bench_with_input(
            BenchmarkId::new("block_csr_no_prefetch", &case),
            &(&block, &x),
            |b, (block, x)| b.iter(|| black_box(block.matvec_with_prefetch_distance(x, 0))),
        );
    }

    group.finish();
}

/// `reorder_rcm` and the input-vector column permutation run outside the timed
/// loop. `rcm_original_output` includes the result unpermutation.
fn bench_ldpc_rcm_amortized_matvec(c: &mut Criterion) {
    const CALLS: usize = 128;

    let mut group = c.benchmark_group("sparse_matvec_ldpc_rcm_amortized_128");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(3));

    for &(rows, cols, row_weight) in &[
        (4096usize, 8192usize, 6usize),
        (8192, 16384, 6),
        (4096, 32768, 32),
    ] {
        let csr = deterministic_ldpc_like_fixture(rows, cols, row_weight);
        let (rcm, permutation) = csr.reorder_rcm();
        let x = deterministic_sparse_bitvec_fixture(cols);
        let x_rcm = permutation.apply_cols(&x);
        let case = format!("{rows}x{cols}_w{row_weight}");

        group.bench_with_input(
            BenchmarkId::new("csr", &case),
            &(&csr, &x),
            |b, (csr, x)| {
                b.iter(|| {
                    let mut last = BitVec::with_capacity(csr.rows());
                    for _ in 0..CALLS {
                        last = black_box(csr).matvec(black_box(x));
                    }
                    black_box(last)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rcm_reordered_output", &case),
            &(&rcm, &x_rcm),
            |b, (rcm, x_rcm)| {
                b.iter(|| {
                    let mut last = BitVec::with_capacity(rcm.rows());
                    for _ in 0..CALLS {
                        last = black_box(rcm).matvec(black_box(x_rcm));
                    }
                    black_box(last)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rcm_original_output", &case),
            &(&rcm, &permutation, &x_rcm),
            |b, (rcm, permutation, x_rcm)| {
                b.iter(|| {
                    let mut last = BitVec::with_capacity(rcm.rows());
                    for _ in 0..CALLS {
                        let y_rcm = black_box(rcm).matvec(black_box(x_rcm));
                        last = permutation.unapply_rows(&y_rcm);
                    }
                    black_box(last)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sparse_matvec,
    bench_dense_vs_sparse,
    bench_sparse_transpose,
    bench_dual_col_iter_vs_transpose,
    bench_bidirectional_sweep,
    bench_dual_matvec_transpose,
    bench_ldpc_block_csr_matvec,
    bench_ldpc_rcm_amortized_matvec
);
criterion_main!(benches);
