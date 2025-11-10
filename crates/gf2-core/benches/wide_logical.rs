use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::BitVec;
use rand::{Rng, SeedableRng};

fn make_bitvec_random(len_bits: usize, seed: u64) -> BitVec {
    let byte_len = (len_bits + 7) / 8;
    let mut bytes = vec![0u8; byte_len];
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    for b in &mut bytes { *b = rng.gen(); }
    let mut bv = BitVec::from_bytes_le(&bytes);
    if len_bits < bv.len() {
        bv.resize(len_bits, false);
    }
    bv
}

fn sizes() -> Vec<usize> {
    let mut v = vec![1usize << 10, 1 << 12, 1 << 14, 1 << 16, 1 << 18, 1 << 20];
    if std::env::var("GF2_BENCH_INCLUDE_8M").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false) {
        v.push(1 << 23);
    }
    v
}

fn bench_op(c: &mut Criterion, name: &str, mut f: impl FnMut(&mut BitVec, &BitVec)) {
    let mut group = c.benchmark_group(name);
    let sizes = sizes();
    for &sz_bytes in &sizes {
        let len_bits = sz_bytes * 8;
        let a = make_bitvec_random(len_bits, 0xA5A5_0000);
        let b = make_bitvec_random(len_bits, 0x5A5A_0000);
        group.throughput(Throughput::Bytes(sz_bytes as u64));
        group.bench_with_input(BenchmarkId::new(name, sz_bytes), &sz_bytes, |bencher, _| {
            bencher.iter_batched(
                || a.clone(),
                |mut dst| { f(&mut dst, &b); black_box(&dst); },
                criterion::BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_wide_logical(c: &mut Criterion) {
    bench_op(c, "xor_into", |dst, b| dst.bit_xor_into(b));
    bench_op(c, "and_into", |dst, b| dst.bit_and_into(b));
    bench_op(c, "or_into", |dst, b| dst.bit_or_into(b));

    let mut group = c.benchmark_group("not_into");
    let sizes = sizes();
    for &sz_bytes in &sizes {
        let len_bits = sz_bytes * 8;
        let a = make_bitvec_random(len_bits, 0xFACE_F00D);
        group.throughput(Throughput::Bytes(sz_bytes as u64));
        group.bench_with_input(BenchmarkId::new("not_into", sz_bytes), &sz_bytes, |bencher, _| {
            bencher.iter_batched(
                || a.clone(),
                |mut dst| { dst.not_into(); black_box(&dst); },
                criterion::BatchSize::SmallInput,
            )
        });
    }
    group.finish();

    // Count ones benchmark (unary throughput)
    let mut group = c.benchmark_group("count_ones");
    for &sz_bytes in &sizes {
        let len_bits = sz_bytes * 8;
        let a = make_bitvec_random(len_bits, 0xC0FF_EE00);
        group.throughput(Throughput::Bytes(sz_bytes as u64));
        group.bench_with_input(BenchmarkId::new("count_ones", sz_bytes), &sz_bytes, |bencher, _| {
            bencher.iter(|| {
                let cnt = a.count_ones();
                black_box(cnt);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_wide_logical);
criterion_main!(benches);
