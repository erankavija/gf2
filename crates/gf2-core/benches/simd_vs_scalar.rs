//! SIMD vs Scalar backend performance comparison.
//!
//! This benchmark suite measures the actual performance characteristics of
//! SIMD vs Scalar backends across different buffer sizes and operations.
//!
//! Key objectives:
//! 1. Validate the 8-word threshold assumption
//! 2. Measure actual speedup factors
//! 3. Identify which operations benefit most from SIMD
//! 4. Document performance characteristics for different sizes

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::kernels::{scalar::ScalarBackend, Backend};
use rand::{Rng, SeedableRng};

fn random_data(word_count: usize, seed: u64) -> Vec<u64> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    (0..word_count).map(|_| rng.gen()).collect()
}

/// Test sizes covering the threshold region and beyond.
///
/// - 1, 2, 4: Very small (should favor scalar)
/// - 7: Just below threshold
/// - 8: At threshold (decision point)
/// - 16, 32: Just above threshold
/// - 64, 128, 256: Medium buffers
/// - 1024, 4096: Large buffers (should favor SIMD)
fn sizes() -> Vec<usize> {
    vec![1, 2, 4, 7, 8, 16, 32, 64, 128, 256, 1024, 4096]
}

/// Benchmark XOR operation: dst[i] ^= src[i]
fn bench_xor(c: &mut Criterion) {
    let mut group = c.benchmark_group("xor");

    for &size in &sizes() {
        let bytes = size * 8;
        group.throughput(Throughput::Bytes(bytes as u64));

        // Benchmark scalar backend
        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, &size| {
            let backend = &ScalarBackend;
            let mut dst = random_data(size, 0xDEADBEEF);
            let src = random_data(size, 0xCAFEBABE);

            bencher.iter(|| {
                backend.xor(&mut dst, &src);
                black_box(&dst);
            });
        });

        // Benchmark SIMD backend (if available)
        #[cfg(feature = "simd")]
        if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
            group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, &size| {
                let mut dst = random_data(size, 0xDEADBEEF);
                let src = random_data(size, 0xCAFEBABE);

                bencher.iter(|| {
                    backend.xor(&mut dst, &src);
                    black_box(&dst);
                });
            });
        }
    }

    group.finish();
}

/// Benchmark AND operation: dst[i] &= src[i]
fn bench_and(c: &mut Criterion) {
    let mut group = c.benchmark_group("and");

    for &size in &sizes() {
        let bytes = size * 8;
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, &size| {
            let backend = &ScalarBackend;
            let mut dst = random_data(size, 0x12345678);
            let src = random_data(size, 0x87654321);

            bencher.iter(|| {
                backend.and(&mut dst, &src);
                black_box(&dst);
            });
        });

        #[cfg(feature = "simd")]
        if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
            group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, &size| {
                let mut dst = random_data(size, 0x12345678);
                let src = random_data(size, 0x87654321);

                bencher.iter(|| {
                    backend.and(&mut dst, &src);
                    black_box(&dst);
                });
            });
        }
    }

    group.finish();
}

/// Benchmark OR operation: dst[i] |= src[i]
fn bench_or(c: &mut Criterion) {
    let mut group = c.benchmark_group("or");

    for &size in &sizes() {
        let bytes = size * 8;
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, &size| {
            let backend = &ScalarBackend;
            let mut dst = random_data(size, 0xABCDEF00);
            let src = random_data(size, 0x00FEDCBA);

            bencher.iter(|| {
                backend.or(&mut dst, &src);
                black_box(&dst);
            });
        });

        #[cfg(feature = "simd")]
        if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
            group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, &size| {
                let mut dst = random_data(size, 0xABCDEF00);
                let src = random_data(size, 0x00FEDCBA);

                bencher.iter(|| {
                    backend.or(&mut dst, &src);
                    black_box(&dst);
                });
            });
        }
    }

    group.finish();
}

/// Benchmark NOT operation: buf[i] = !buf[i]
fn bench_not(c: &mut Criterion) {
    let mut group = c.benchmark_group("not");

    for &size in &sizes() {
        let bytes = size * 8;
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, &size| {
            let backend = &ScalarBackend;
            let mut buf = random_data(size, 0xFEEDFACE);

            bencher.iter(|| {
                backend.not(&mut buf);
                black_box(&buf);
            });
        });

        #[cfg(feature = "simd")]
        if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
            group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, &size| {
                let mut buf = random_data(size, 0xFEEDFACE);

                bencher.iter(|| {
                    backend.not(&mut buf);
                    black_box(&buf);
                });
            });
        }
    }

    group.finish();
}

/// Benchmark popcount operation: count total set bits
fn bench_popcount(c: &mut Criterion) {
    let mut group = c.benchmark_group("popcount");

    for &size in &sizes() {
        let bytes = size * 8;
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, &size| {
            let backend = &ScalarBackend;
            let buf = random_data(size, 0xC0FFEE00);

            bencher.iter(|| {
                let count = backend.popcount(&buf);
                black_box(count);
            });
        });

        #[cfg(feature = "simd")]
        if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
            group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, &size| {
                let buf = random_data(size, 0xC0FFEE00);

                bencher.iter(|| {
                    let count = backend.popcount(&buf);
                    black_box(count);
                });
            });
        }
    }

    group.finish();
}

/// Benchmark with different data patterns to check if performance varies.
fn bench_patterns(c: &mut Criterion) {
    let size = 256; // Medium size for pattern testing
    let bytes = size * 8;

    let mut group = c.benchmark_group("patterns");
    group.throughput(Throughput::Bytes(bytes as u64));

    // Pattern 1: All zeros
    group.bench_function("scalar/zeros", |bencher| {
        let backend = &ScalarBackend;
        let mut dst = vec![0u64; size];
        let src = vec![0u64; size];
        bencher.iter(|| {
            backend.xor(&mut dst, &src);
            black_box(&dst);
        });
    });

    #[cfg(feature = "simd")]
    if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
        group.bench_function("simd/zeros", |bencher| {
            let mut dst = vec![0u64; size];
            let src = vec![0u64; size];
            bencher.iter(|| {
                backend.xor(&mut dst, &src);
                black_box(&dst);
            });
        });
    }

    // Pattern 2: All ones
    group.bench_function("scalar/ones", |bencher| {
        let backend = &ScalarBackend;
        let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; size];
        let src = vec![0xFFFFFFFFFFFFFFFFu64; size];
        bencher.iter(|| {
            backend.xor(&mut dst, &src);
            black_box(&dst);
        });
    });

    #[cfg(feature = "simd")]
    if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
        group.bench_function("simd/ones", |bencher| {
            let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; size];
            let src = vec![0xFFFFFFFFFFFFFFFFu64; size];
            bencher.iter(|| {
                backend.xor(&mut dst, &src);
                black_box(&dst);
            });
        });
    }

    // Pattern 3: Alternating
    group.bench_function("scalar/alternating", |bencher| {
        let backend = &ScalarBackend;
        let mut dst = vec![0xAAAAAAAAAAAAAAAAu64; size];
        let src = vec![0x5555555555555555u64; size];
        bencher.iter(|| {
            backend.xor(&mut dst, &src);
            black_box(&dst);
        });
    });

    #[cfg(feature = "simd")]
    if let Some(backend) = gf2_core::kernels::simd::maybe_simd() {
        group.bench_function("simd/alternating", |bencher| {
            let mut dst = vec![0xAAAAAAAAAAAAAAAAu64; size];
            let src = vec![0x5555555555555555u64; size];
            bencher.iter(|| {
                backend.xor(&mut dst, &src);
                black_box(&dst);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_xor,
    bench_and,
    bench_or,
    bench_not,
    bench_popcount,
    bench_patterns
);
criterion_main!(benches);
