//! Benchmark LLR SIMD operations.
//!
//! Measures the speedup from gf2-kernels-simd AVX2 acceleration for:
//! - boxplus_minsum_n (check node updates in LDPC BP)
//! - saturate_batch
//! - hard_decision_batch

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_coding::llr::Llr;

fn bench_boxplus_minsum_n(c: &mut Criterion) {
    let mut group = c.benchmark_group("llr_boxplus_minsum_n");

    for size in [8, 16, 32, 64, 128, 256] {
        let llrs: Vec<Llr> = (0..size)
            .map(|i| {
                let val = (i as f32) * 0.1;
                if i % 3 == 0 {
                    Llr::new(-val)
                } else {
                    Llr::new(val)
                }
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &llrs, |b, llrs| {
            b.iter(|| black_box(Llr::boxplus_minsum_n(llrs)));
        });
    }

    group.finish();
}

fn bench_saturate_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("llr_saturate_batch");

    for size in [64, 256, 1024, 4096] {
        let llrs: Vec<Llr> = (0..size)
            .map(|i| Llr::new((i as f32) * 0.5 - 100.0))
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &llrs, |b, llrs| {
            b.iter(|| black_box(Llr::saturate_batch(llrs, 10.0)));
        });
    }

    group.finish();
}

fn bench_hard_decision_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("llr_hard_decision_batch");

    for size in [64, 256, 1024, 4096] {
        let llrs: Vec<Llr> = (0..size)
            .map(|i| {
                let val = (i as f32) * 0.1 - 50.0;
                Llr::new(val)
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &llrs, |b, llrs| {
            b.iter(|| black_box(Llr::hard_decision_batch(llrs)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_boxplus_minsum_n,
    bench_saturate_batch,
    bench_hard_decision_batch
);
criterion_main!(benches);
