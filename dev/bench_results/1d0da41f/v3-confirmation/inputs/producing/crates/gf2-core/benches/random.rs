use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::{matrix::BitMatrix, BitVec};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn bench_bitvec_random_uniform(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitvec_random_uniform");

    for size in [1024, 4096, 16384, 65536, 262144, 1048576].iter() {
        group.throughput(Throughput::Bytes((*size / 8) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut rng = StdRng::seed_from_u64(42);
            b.iter(|| {
                let bv = BitVec::random(black_box(size), &mut rng);
                black_box(bv)
            });
        });
    }
    group.finish();
}

fn bench_bitvec_random_with_probability(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitvec_random_with_prob");

    for (p, label) in [(0.1, "p01"), (0.5, "p05"), (0.9, "p09")].iter() {
        for size in [1024, 16384, 65536].iter() {
            group.throughput(Throughput::Bytes((*size / 8) as u64));
            group.bench_with_input(
                BenchmarkId::new(*label, size),
                &(*size, *p),
                |b, &(size, p)| {
                    let mut rng = StdRng::seed_from_u64(42);
                    b.iter(|| {
                        let bv = BitVec::random_with_probability(black_box(size), p, &mut rng);
                        black_box(bv)
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_bitmatrix_random_uniform(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitmatrix_random_uniform");

    for size in [64, 128, 256, 512, 1024].iter() {
        let total_bits = size * size;
        group.throughput(Throughput::Bytes((total_bits / 8) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut rng = StdRng::seed_from_u64(42);
            b.iter(|| {
                let m = BitMatrix::random(black_box(size), black_box(size), &mut rng);
                black_box(m)
            });
        });
    }
    group.finish();
}

fn bench_bitmatrix_random_with_probability(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitmatrix_random_with_prob");

    for (p, label) in [(0.1, "p01"), (0.5, "p05"), (0.9, "p09")].iter() {
        for size in [64, 256, 512].iter() {
            let total_bits = size * size;
            group.throughput(Throughput::Bytes((total_bits / 8) as u64));
            group.bench_with_input(
                BenchmarkId::new(*label, size),
                &(*size, *p),
                |b, &(size, p)| {
                    let mut rng = StdRng::seed_from_u64(42);
                    b.iter(|| {
                        let m = BitMatrix::random_with_probability(
                            black_box(size),
                            black_box(size),
                            p,
                            &mut rng,
                        );
                        black_box(m)
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_bitvec_fill_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitvec_fill_random");

    for size in [1024, 16384, 65536, 262144].iter() {
        group.throughput(Throughput::Bytes((*size / 8) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut rng = StdRng::seed_from_u64(42);
            b.iter(|| {
                let mut bv = BitVec::from_bytes_le(&vec![0u8; size / 8]);
                bv.fill_random(&mut rng);
                black_box(bv)
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_bitvec_random_uniform,
    bench_bitvec_random_with_probability,
    bench_bitmatrix_random_uniform,
    bench_bitmatrix_random_with_probability,
    bench_bitvec_fill_random,
);
criterion_main!(benches);
