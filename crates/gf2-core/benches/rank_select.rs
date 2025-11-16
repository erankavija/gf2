use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::BitVec;

fn bench_rank(c: &mut Criterion) {
    let mut group = c.benchmark_group("rank");

    for size_kb in [1, 16, 64, 256].iter() {
        let bits = size_kb * 8 * 1024;
        let bytes: Vec<u8> = (0..(bits / 8)).map(|i| (i % 256) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);

        group.throughput(Throughput::Bytes((bits / 8) as u64));

        // Benchmark rank at various positions
        group.bench_with_input(BenchmarkId::new("rank_middle", size_kb), &bv, |b, bv| {
            let pos = bits / 2;
            b.iter(|| black_box(bv.rank(black_box(pos))));
        });

        group.bench_with_input(BenchmarkId::new("rank_end", size_kb), &bv, |b, bv| {
            let pos = bits - 1;
            b.iter(|| black_box(bv.rank(black_box(pos))));
        });
    }

    group.finish();
}

fn bench_rank_naive(c: &mut Criterion) {
    let mut group = c.benchmark_group("rank_naive");

    for size_kb in [1, 16, 64, 256].iter() {
        let bits = size_kb * 8 * 1024;
        let bytes: Vec<u8> = (0..(bits / 8)).map(|i| (i % 256) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);

        group.throughput(Throughput::Bytes((bits / 8) as u64));

        // Naive rank implementation for comparison
        let rank_naive =
            |bv: &BitVec, idx: usize| -> usize { (0..=idx).filter(|&i| bv.get(i)).count() };

        group.bench_with_input(BenchmarkId::new("naive_middle", size_kb), &bv, |b, bv| {
            let pos = bits / 2;
            b.iter(|| black_box(rank_naive(bv, black_box(pos))));
        });

        group.bench_with_input(BenchmarkId::new("naive_end", size_kb), &bv, |b, bv| {
            let pos = bits - 1;
            b.iter(|| black_box(rank_naive(bv, black_box(pos))));
        });
    }

    group.finish();
}

fn bench_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("select");

    for size_kb in [1, 16, 64, 256].iter() {
        let bits = size_kb * 8 * 1024;
        let bytes: Vec<u8> = (0..(bits / 8)).map(|i| (i % 256) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);
        let total_ones = bv.count_ones();

        group.throughput(Throughput::Bytes((bits / 8) as u64));

        // Benchmark select at various ranks
        group.bench_with_input(BenchmarkId::new("select_middle", size_kb), &bv, |b, bv| {
            let k = total_ones / 2;
            b.iter(|| black_box(bv.select(black_box(k))));
        });

        group.bench_with_input(BenchmarkId::new("select_end", size_kb), &bv, |b, bv| {
            let k = total_ones - 1;
            b.iter(|| black_box(bv.select(black_box(k))));
        });
    }

    group.finish();
}

fn bench_select_naive(c: &mut Criterion) {
    let mut group = c.benchmark_group("select_naive");

    for size_kb in [1, 16, 64, 256].iter() {
        let bits = size_kb * 8 * 1024;
        let bytes: Vec<u8> = (0..(bits / 8)).map(|i| (i % 256) as u8).collect();
        let bv = BitVec::from_bytes_le(&bytes);
        let total_ones = bv.count_ones();

        group.throughput(Throughput::Bytes((bits / 8) as u64));

        // Naive select implementation for comparison
        let select_naive = |bv: &BitVec, k: usize| -> Option<usize> {
            let mut count = 0;
            for i in 0..bv.len() {
                if bv.get(i) {
                    if count == k {
                        return Some(i);
                    }
                    count += 1;
                }
            }
            None
        };

        group.bench_with_input(BenchmarkId::new("naive_middle", size_kb), &bv, |b, bv| {
            let k = total_ones / 2;
            b.iter(|| black_box(select_naive(bv, black_box(k))));
        });

        group.bench_with_input(BenchmarkId::new("naive_end", size_kb), &bv, |b, bv| {
            let k = total_ones - 1;
            b.iter(|| black_box(select_naive(bv, black_box(k))));
        });
    }

    group.finish();
}

fn bench_index_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("rank_select_index_build");

    for size_kb in [1, 16, 64, 256].iter() {
        let bits = size_kb * 8 * 1024;
        let bytes: Vec<u8> = (0..(bits / 8)).map(|i| (i % 256) as u8).collect();

        group.throughput(Throughput::Bytes((bits / 8) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size_kb), size_kb, |b, _| {
            b.iter_batched(
                || BitVec::from_bytes_le(&bytes),
                |bv| {
                    // Force index build by calling rank
                    black_box(bv.rank(0));
                    black_box(bv);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_rank,
    bench_rank_naive,
    bench_select,
    bench_select_naive,
    bench_index_build
);
criterion_main!(benches);
