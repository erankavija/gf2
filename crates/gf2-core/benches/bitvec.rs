use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::BitVec;

fn bench_bit_xor_into(c: &mut Criterion) {
    let mut group = c.benchmark_group("bit_xor_into");

    for size in [1024, 64 * 1024, 1024 * 1024].iter() {
        let bytes = size / 8;
        let data1: Vec<u8> = (0..bytes).map(|i| i as u8).collect();
        let data2: Vec<u8> = (0..bytes).map(|i| (i ^ 0xAA) as u8).collect();

        group.throughput(Throughput::Bytes(*size as u64 / 8));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut bv1 = BitVec::from_bytes_le(&data1);
                let bv2 = BitVec::from_bytes_le(&data2);
                bv1.bit_xor_into(&bv2);
                black_box(bv1);
            });
        });
    }

    group.finish();
}

fn bench_count_ones(c: &mut Criterion) {
    let mut group = c.benchmark_group("count_ones");

    for size in [1024, 64 * 1024, 1024 * 1024].iter() {
        let bytes = size / 8;
        let data: Vec<u8> = (0..bytes).map(|i| i as u8).collect();

        group.throughput(Throughput::Bytes(*size as u64 / 8));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let bv = BitVec::from_bytes_le(&data);
            b.iter(|| {
                let count = bv.count_ones();
                black_box(count);
            });
        });
    }

    group.finish();
}

fn bench_shift_left(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_left");
    let size = 64 * 1024;
    let bytes = size / 8;
    let data: Vec<u8> = (0..bytes).map(|i| i as u8).collect();

    group.throughput(Throughput::Bytes(size as u64 / 8));

    for shift in [1, 8, 64, 128].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(shift), shift, |b, &s| {
            b.iter(|| {
                let mut bv = BitVec::from_bytes_le(&data);
                bv.shift_left(s);
                black_box(bv);
            });
        });
    }

    group.finish();
}

fn bench_shift_right(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_right");
    let size = 64 * 1024;
    let bytes = size / 8;
    let data: Vec<u8> = (0..bytes).map(|i| i as u8).collect();

    group.throughput(Throughput::Bytes(size as u64 / 8));

    for shift in [1, 8, 64, 128].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(shift), shift, |b, &s| {
            b.iter(|| {
                let mut bv = BitVec::from_bytes_le(&data);
                bv.shift_right(s);
                black_box(bv);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_bit_xor_into,
    bench_count_ones,
    bench_shift_left,
    bench_shift_right
);
criterion_main!(benches);
