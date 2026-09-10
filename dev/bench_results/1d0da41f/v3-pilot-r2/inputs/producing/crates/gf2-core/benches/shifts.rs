//! Benchmark suite for bit shift operations.
//!
//! Compares scalar baseline vs SIMD implementations across different buffer sizes
//! and shift amounts to determine optimal implementation strategy.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::BitVec;

/// Helper to create a BitVec with random-ish data
fn create_bitvec(num_bytes: usize) -> BitVec {
    let data: Vec<u8> = (0..num_bytes).map(|i| i as u8).collect();
    BitVec::from_bytes_le(&data)
}

/// Benchmark shift_left with word-aligned shift amount (k % 64 == 0)
fn bench_shift_left_word_aligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_left_word_aligned");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        // Shift by 128 bits (2 words) - word-aligned
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift128", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_left(128);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark shift_left with bit-level shift amount (k % 64 != 0)
fn bench_shift_left_bit_level(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_left_bit_level");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        // Shift by 137 bits (2 words + 9 bits) - requires bit combining
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift137", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_left(137);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark shift_right with word-aligned shift amount
fn bench_shift_right_word_aligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_right_word_aligned");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift128", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_right(128);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark shift_right with bit-level shift amount
fn bench_shift_right_bit_level(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_right_bit_level");

    for size_kb in [1, 4, 16, 64, 256].iter() {
        let num_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(num_bytes as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB_shift137", size_kb)),
            &num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_right(137);
                    black_box(&bv);
                });
            },
        );
    }

    group.finish();
}

/// Comprehensive comparison: word-aligned vs bit-level shifts
fn bench_shift_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_comparison");

    let num_bytes = 64 * 1024; // 64 KB
    group.throughput(Throughput::Bytes(num_bytes as u64));

    let shift_amounts = [
        ("shift_0", 0),
        ("shift_1", 1),
        ("shift_7", 7),
        ("shift_63", 63),
        ("shift_64", 64), // Word boundary
        ("shift_65", 65),
        ("shift_127", 127),
        ("shift_128", 128), // Word boundary
    ];

    for (name, shift) in shift_amounts.iter() {
        group.bench_with_input(BenchmarkId::new("shift_left", name), shift, |b, &k| {
            let mut bv = create_bitvec(num_bytes);
            b.iter(|| {
                bv.shift_left(k);
                black_box(&bv);
            });
        });

        group.bench_with_input(BenchmarkId::new("shift_right", name), shift, |b, &k| {
            let mut bv = create_bitvec(num_bytes);
            b.iter(|| {
                bv.shift_right(k);
                black_box(&bv);
            });
        });
    }

    group.finish();
}

/// Benchmark across multiple buffer sizes to find SIMD crossover point
fn bench_shift_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_sizes");

    // Test small to large buffers
    let sizes = [
        ("64B", 64),
        ("256B", 256),
        ("1KB", 1024),
        ("4KB", 4 * 1024),
        ("16KB", 16 * 1024),
        ("64KB", 64 * 1024),
        ("256KB", 256 * 1024),
        ("1MB", 1024 * 1024),
    ];

    for (name, num_bytes) in sizes.iter() {
        group.throughput(Throughput::Bytes(*num_bytes as u64));

        // Word-aligned shift
        group.bench_with_input(
            BenchmarkId::new("word_aligned", name),
            num_bytes,
            |b, &n| {
                let mut bv = create_bitvec(n);
                b.iter(|| {
                    bv.shift_left(128); // 2 words
                    black_box(&bv);
                });
            },
        );

        // Bit-level shift
        group.bench_with_input(BenchmarkId::new("bit_level", name), num_bytes, |b, &n| {
            let mut bv = create_bitvec(n);
            b.iter(|| {
                bv.shift_left(137); // 2 words + 9 bits
                black_box(&bv);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_shift_left_word_aligned,
    bench_shift_left_bit_level,
    bench_shift_right_word_aligned,
    bench_shift_right_bit_level,
    bench_shift_comparison,
    bench_shift_sizes,
);
criterion_main!(benches);
