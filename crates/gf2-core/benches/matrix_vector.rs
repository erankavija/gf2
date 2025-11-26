//! Benchmarks for dense BitMatrix matrix-vector multiplication.
//!
//! These benchmarks measure the performance of matrix-vector operations
//! for dense bit-packed matrices over GF(2).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::{BitMatrix, BitVec};
use rand::SeedableRng;

/// Benchmark dense matrix-vector multiplication: y = A × x
fn bench_matvec(c: &mut Criterion) {
    let mut group = c.benchmark_group("dense_matvec");

    for &size in &[64, 128, 256, 512, 1024] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m = BitMatrix::random(size, size, &mut rng);
        let x = BitVec::random(size, &mut rng);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(m.matvec(&x)))
        });
    }
    group.finish();
}

/// Benchmark dense transpose matrix-vector multiplication: y = A^T × x
fn bench_matvec_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("dense_matvec_transpose");

    for &size in &[64, 128, 256, 512, 1024] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m = BitMatrix::random(size, size, &mut rng);
        let x = BitVec::random(size, &mut rng);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(m.matvec_transpose(&x)))
        });
    }
    group.finish();
}

/// Benchmark matvec at different matrix densities
fn bench_matvec_by_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("matvec_by_density");

    let size = 512;
    for &density in &[0.1, 0.3, 0.5, 0.7, 0.9] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
        let x = BitVec::random(size, &mut rng);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:.1}", density)),
            &density,
            |b, _| b.iter(|| black_box(m.matvec(&x))),
        );
    }
    group.finish();
}

/// Benchmark rectangular matrix-vector multiplication
fn bench_matvec_rectangular(c: &mut Criterion) {
    let mut group = c.benchmark_group("matvec_rectangular");

    for &(rows, cols) in &[(100, 1000), (500, 100), (1000, 1000)] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m = BitMatrix::random(rows, cols, &mut rng);
        let x = BitVec::random(cols, &mut rng);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}", rows, cols)),
            &(rows, cols),
            |b, _| b.iter(|| black_box(m.matvec(&x))),
        );
    }
    group.finish();
}

/// Benchmark transpose rectangular matrix-vector multiplication
fn bench_matvec_transpose_rectangular(c: &mut Criterion) {
    let mut group = c.benchmark_group("matvec_transpose_rectangular");

    for &(rows, cols) in &[(100, 1000), (500, 100), (1000, 1000)] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m = BitMatrix::random(rows, cols, &mut rng);
        let x = BitVec::random(rows, &mut rng);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}", rows, cols)),
            &(rows, cols),
            |b, _| b.iter(|| black_box(m.matvec_transpose(&x))),
        );
    }
    group.finish();
}

/// Benchmark dense vs sparse matvec at various densities
fn bench_dense_vs_sparse_matvec(c: &mut Criterion) {
    use gf2_core::sparse::SpBitMatrix;
    let mut group = c.benchmark_group("dense_vs_sparse_matvec");

    let size = 500;
    for &density in &[0.01, 0.05, 0.1, 0.3, 0.5] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m_dense = BitMatrix::random_with_probability(size, size, density, &mut rng);
        let m_sparse = SpBitMatrix::from_dense(&m_dense);
        let x = BitVec::random(size, &mut rng);

        group.bench_with_input(
            BenchmarkId::new("dense", format!("{:.2}", density)),
            &density,
            |b, _| b.iter(|| black_box(m_dense.matvec(&x))),
        );

        group.bench_with_input(
            BenchmarkId::new("sparse", format!("{:.2}", density)),
            &density,
            |b, _| b.iter(|| black_box(m_sparse.matvec(&x))),
        );
    }
    group.finish();
}

/// Benchmark dense vs sparse transpose matvec at various densities
fn bench_dense_vs_sparse_transpose(c: &mut Criterion) {
    use gf2_core::sparse::SpBitMatrixDual;
    let mut group = c.benchmark_group("dense_vs_sparse_transpose");

    let size = 500;
    for &density in &[0.01, 0.05, 0.1, 0.3, 0.5] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m_dense = BitMatrix::random_with_probability(size, size, density, &mut rng);
        let m_sparse = SpBitMatrixDual::from_dense(&m_dense);
        let x = BitVec::random(size, &mut rng);

        group.bench_with_input(
            BenchmarkId::new("dense", format!("{:.2}", density)),
            &density,
            |b, _| b.iter(|| black_box(m_dense.matvec_transpose(&x))),
        );

        group.bench_with_input(
            BenchmarkId::new("sparse", format!("{:.2}", density)),
            &density,
            |b, _| b.iter(|| black_box(m_sparse.matvec_transpose(&x))),
        );
    }
    group.finish();
}

/// Benchmark word boundary cases (63, 64, 65 bits)
fn bench_matvec_word_boundaries(c: &mut Criterion) {
    let mut group = c.benchmark_group("matvec_word_boundaries");

    for &size in &[63, 64, 65, 127, 128, 129] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let m = BitMatrix::random(size, size, &mut rng);
        let x = BitVec::random(size, &mut rng);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(m.matvec(&x)))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_matvec,
    bench_matvec_transpose,
    bench_matvec_by_density,
    bench_matvec_rectangular,
    bench_matvec_transpose_rectangular,
    bench_dense_vs_sparse_matvec,
    bench_dense_vs_sparse_transpose,
    bench_matvec_word_boundaries,
);
criterion_main!(benches);
