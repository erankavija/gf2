use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gfp::Fp;

type F = Fp<7>;

fn bench_zeros(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix/zeros");
    for &n in &[64usize, 256, 1024] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let m = FieldMatrix::<F>::zeros(black_box(n), black_box(n));
                black_box(m);
            });
        });
    }
    group.finish();
}

fn bench_identity(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix/identity");
    for &n in &[64usize, 256, 1024] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let m = FieldMatrix::<F>::identity(black_box(n));
                black_box(m);
            });
        });
    }
    group.finish();
}

fn bench_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix/transpose");
    for &n in &[64usize, 256, 1024] {
        let m = FieldMatrix::<F>::random_seeded(n, n, 0xC0FFEE ^ n as u64);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let t = black_box(&m).transpose();
                black_box(t);
            });
        });
    }
    group.finish();
}

fn bench_row_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix/row_access");
    for &n in &[64usize, 256, 1024] {
        let m = FieldMatrix::<F>::random_seeded(n, n, 0xBEEFu64 ^ n as u64);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let r = black_box(&m).row(black_box(n / 2));
                black_box(r.len());
            });
        });
    }
    group.finish();
}

fn bench_col_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix/col_access");
    for &n in &[64usize, 256, 1024] {
        let m = FieldMatrix::<F>::random_seeded(n, n, 0xFEEDu64 ^ n as u64);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let col = black_box(&m).col(black_box(n / 2));
                black_box(col.len());
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_zeros,
    bench_identity,
    bench_transpose,
    bench_row_access,
    bench_col_access
);
criterion_main!(benches);
