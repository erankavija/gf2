use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::gf2m::generation::{GenerationStrategy, PrimitiveGenerator};

fn bench_exhaustive_first(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_exhaustive_first");

    for m in [2, 3, 4, 5, 6, 7, 8] {
        group.bench_with_input(BenchmarkId::from_parameter(m), &m, |b, &m| {
            b.iter(|| {
                let gen = PrimitiveGenerator::new(m)
                    .with_strategy(GenerationStrategy::Exhaustive);
                black_box(gen.find_first())
            });
        });
    }

    group.finish();
}

fn bench_exhaustive_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_exhaustive_all");

    for m in [2, 3, 4, 5, 6, 7, 8, 9, 10] {
        group.bench_with_input(BenchmarkId::from_parameter(m), &m, |b, &m| {
            b.iter(|| {
                let gen = PrimitiveGenerator::new(m)
                    .with_strategy(GenerationStrategy::Exhaustive);
                black_box(gen.find_all())
            });
        });
    }

    group.finish();
}

#[cfg(feature = "parallel")]
fn bench_parallel_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_parallel_all");

    for m in [8, 9, 10, 11, 12] {
        group.bench_with_input(BenchmarkId::from_parameter(m), &m, |b, &m| {
            b.iter(|| {
                let gen = PrimitiveGenerator::new(m)
                    .with_strategy(GenerationStrategy::ParallelExhaustive { threads: 4 });
                black_box(gen.find_all())
            });
        });
    }

    group.finish();
}

fn bench_trinomial_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_trinomial");

    for m in [2, 3, 4, 5, 6, 7, 8, 9, 10] {
        group.bench_with_input(BenchmarkId::from_parameter(m), &m, |b, &m| {
            b.iter(|| {
                let gen = PrimitiveGenerator::new(m)
                    .with_strategy(GenerationStrategy::Trinomial);
                black_box(gen.find_first())
            });
        });
    }

    group.finish();
}

#[cfg(feature = "parallel")]
criterion_group!(
    benches,
    bench_exhaustive_first,
    bench_exhaustive_all,
    bench_parallel_all,
    bench_trinomial_search
);

#[cfg(not(feature = "parallel"))]
criterion_group!(
    benches,
    bench_exhaustive_first,
    bench_exhaustive_all,
    bench_trinomial_search
);

criterion_main!(benches);
