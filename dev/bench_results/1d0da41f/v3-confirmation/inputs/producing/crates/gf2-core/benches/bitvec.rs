use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::BitVec;

fn bench_shift_left(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_left");
    let size = 64 * 1024;
    let bytes = size / 8;
    let data: Vec<u8> = (0..bytes).map(|i| i as u8).collect();
    let bv = BitVec::from_bytes_le(&data);

    group.throughput(Throughput::Bytes(bytes as u64));

    for shift in [1, 8, 64, 128].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(shift), shift, |b, &s| {
            b.iter_batched(
                || bv.clone(),
                |mut v| {
                    v.shift_left(s);
                    black_box(v);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_shift_right(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_right");
    let size = 64 * 1024;
    let bytes = size / 8;
    let data: Vec<u8> = (0..bytes).map(|i| i as u8).collect();
    let bv = BitVec::from_bytes_le(&data);

    group.throughput(Throughput::Bytes(bytes as u64));

    for shift in [1, 8, 64, 128].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(shift), shift, |b, &s| {
            b.iter_batched(
                || bv.clone(),
                |mut v| {
                    v.shift_right(s);
                    black_box(v);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_shift_left, bench_shift_right);
criterion_main!(benches);
