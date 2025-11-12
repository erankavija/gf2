use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SparseMatrix;
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

criterion_group!(
    benches,
    bench_sparse_matvec,
    bench_dense_vs_sparse,
    bench_sparse_transpose
);
criterion_main!(benches);
