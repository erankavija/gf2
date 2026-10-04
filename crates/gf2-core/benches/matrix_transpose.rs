//! Benchmarks [`BitMatrix::transpose`] at square, word-boundary and
//! rectangular shapes. Its 64×64 block kernel has an AVX2 PSHUFB lane and a
//! scalar fallback (`@/citation/Warren2012` Section 7-3).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::BitMatrix;

fn bench_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_transpose");

    for &size in &[64usize, 256, 1024, 4096] {
        let m = BitMatrix::random_seeded(size, size, 0x42);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(m.transpose()))
        });
    }
    group.finish();
}

fn bench_transpose_word_boundaries(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_transpose_word_boundaries");

    for &size in &[63usize, 64, 65, 127, 128, 129] {
        let m = BitMatrix::random_seeded(size, size, 0x42);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(m.transpose()))
        });
    }
    group.finish();
}

fn bench_transpose_rectangular(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_transpose_rectangular");

    for &(rows, cols) in &[(256usize, 1024), (1024, 256), (512, 4096), (4096, 512)] {
        let m = BitMatrix::random_seeded(rows, cols, 0x42);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}", rows, cols)),
            &(rows, cols),
            |b, _| b.iter(|| black_box(m.transpose())),
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_transpose,
    bench_transpose_word_boundaries,
    bench_transpose_rectangular,
);
criterion_main!(benches);
