use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SparseMatrix, SparseMatrixDual};
use gf2_core::BitVec;
use rand::SeedableRng;

fn bench_sparse_matvec(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_matvec");

    for &density in &[0.01, 0.05, 0.10] {
        for &size in &[100, 500, 1000] {
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);
            let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
            let s = SparseMatrix::from_dense(&m);
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
    let s = SparseMatrix::from_dense(&m);
    let x = BitVec::random(size, &mut rng);

    group.bench_function("sparse_matvec", |b| b.iter(|| black_box(s.matvec(&x))));

    // Compare to dense matrix-vector via manual iteration
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

fn bench_sparse_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_transpose");

    for &density in &[0.01, 0.05] {
        for &size in &[100, 500, 1000] {
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);
            let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
            let s = SparseMatrix::from_dense(&m);

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

    let single = SparseMatrix::from_dense(&m);
    let dual = SparseMatrixDual::from_dense(&m);

    // Single CSR: transpose on every column access
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

    // Dual: direct column access via CSC
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

    let dual = SparseMatrixDual::from_dense(&m);

    group.bench_function("alternating_row_col_sweeps", |b| {
        b.iter(|| {
            let mut sum = 0;
            // Row sweep
            for r in 0..dual.rows() {
                for _c in dual.row_iter(r) {
                    sum += 1;
                }
            }
            // Column sweep
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

    let dual = SparseMatrixDual::from_dense(&m);
    let x = BitVec::random(size, &mut rng);

    group.bench_function("matvec", |b| b.iter(|| black_box(dual.matvec(&x))));

    group.bench_function("matvec_transpose", |b| {
        b.iter(|| black_box(dual.matvec_transpose(&x)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sparse_matvec,
    bench_dense_vs_sparse,
    bench_sparse_transpose,
    bench_dual_col_iter_vs_transpose,
    bench_bidirectional_sweep,
    bench_dual_matvec_transpose
);
criterion_main!(benches);
