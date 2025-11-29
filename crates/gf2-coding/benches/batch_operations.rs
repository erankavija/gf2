// Batch Operations Benchmarks
//
// Measures performance of batch encoding/decoding with ComputeBackend.
// Run with: cargo bench --bench batch_operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::bch::{BchCode, BchEncoder};
use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::gf2m::Gf2mField;
use gf2_core::BitVec;
use std::path::PathBuf;

/// Load LDPC cache from standard location
fn load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}

/// Benchmark LDPC batch encoding with ComputeBackend
fn bench_ldpc_batch_backend(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let cache = load_cache();
    let encoder = match cache.as_ref() {
        Some(c) => LdpcEncoder::with_cache(code.clone(), c),
        None => LdpcEncoder::new(code.clone()),
    };

    let message = BitVec::zeros(encoder.k());

    let mut group = c.benchmark_group("ldpc_batch_with_backend");

    for batch_size in [1, 10, 50, 100, 202].iter() {
        let messages: Vec<_> = (0..*batch_size).map(|_| message.clone()).collect();
        let total_bits = encoder.k() * batch_size;

        group.throughput(Throughput::Bytes(total_bits as u64 / 8));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| black_box(encoder.encode_batch(black_box(&messages))));
            },
        );
    }

    group.finish();
}

/// Benchmark BCH batch encoding
fn bench_bch_batch(c: &mut Criterion) {
    // Use DVB-T2 BCH(16200, 16008) - t=12 error correction
    let field = Gf2mField::new(14, 0b100000000101011); // GF(2^14)
    let code = BchCode::new(16200, 16008, 12, field);
    let encoder = BchEncoder::new(code);

    let message = BitVec::zeros(encoder.k());

    let mut group = c.benchmark_group("bch_batch");

    for batch_size in [1, 10, 50, 100].iter() {
        let messages: Vec<_> = (0..*batch_size).map(|_| message.clone()).collect();
        let total_bits = encoder.k() * batch_size;

        group.throughput(Throughput::Bytes(total_bits as u64 / 8));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| black_box(encoder.encode_batch(black_box(&messages))));
            },
        );
    }

    group.finish();
}

/// Benchmark comparison: sequential vs batch for LDPC
fn bench_ldpc_sequential_vs_batch(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let cache = load_cache();
    let encoder = match cache.as_ref() {
        Some(c) => LdpcEncoder::with_cache(code.clone(), c),
        None => LdpcEncoder::new(code.clone()),
    };

    let batch_size = 50;
    let messages: Vec<_> = (0..batch_size)
        .map(|_| BitVec::zeros(encoder.k()))
        .collect();

    let mut group = c.benchmark_group("ldpc_sequential_vs_batch");
    group.throughput(Throughput::Bytes((encoder.k() * batch_size) as u64 / 8));

    group.bench_function("sequential_loop", |b| {
        b.iter(|| {
            black_box(
                messages
                    .iter()
                    .map(|msg| encoder.encode(msg))
                    .collect::<Vec<_>>(),
            )
        });
    });

    group.bench_function("batch_operation", |b| {
        b.iter(|| black_box(encoder.encode_batch(black_box(&messages))));
    });

    group.finish();
}

/// Benchmark BCH sequential vs batch
fn bench_bch_sequential_vs_batch(c: &mut Criterion) {
    let field = Gf2mField::new(4, 0b10011);
    let code = BchCode::new(15, 11, 1, field);
    let encoder = BchEncoder::new(code);

    let batch_size = 100;
    let messages: Vec<_> = (0..batch_size)
        .map(|i| {
            let mut msg = BitVec::with_capacity(11);
            for j in 0..11 {
                msg.push_bit((i + j) % 2 == 0);
            }
            msg
        })
        .collect();

    let mut group = c.benchmark_group("bch_sequential_vs_batch");
    group.throughput(Throughput::Bytes((encoder.k() * batch_size) as u64 / 8));

    group.bench_function("sequential_loop", |b| {
        b.iter(|| {
            black_box(
                messages
                    .iter()
                    .map(|msg| encoder.encode(msg))
                    .collect::<Vec<_>>(),
            )
        });
    });

    group.bench_function("batch_operation", |b| {
        b.iter(|| black_box(encoder.encode_batch(black_box(&messages))));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_ldpc_batch_backend,
    bench_bch_batch,
    bench_ldpc_sequential_vs_batch,
    bench_bch_sequential_vs_batch,
);

criterion_main!(benches);
