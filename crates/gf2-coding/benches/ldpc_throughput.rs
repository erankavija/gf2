// LDPC Throughput Benchmarks
//
// Measures encoding and decoding performance for DVB-T2 LDPC codes.
// Run with: cargo bench --bench ldpc_throughput

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_coding::CodeRate;
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

/// Benchmark LDPC encoding for a single block
fn bench_ldpc_encode_single(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let cache = load_cache();
    let encoder = match cache.as_ref() {
        Some(c) => LdpcEncoder::with_cache(code.clone(), c),
        None => LdpcEncoder::new(code.clone()),
    };

    let message = BitVec::zeros(encoder.k());

    let mut group = c.benchmark_group("ldpc_encode_single");
    group.throughput(Throughput::Bytes(encoder.k() as u64 / 8));

    group.bench_function("dvb_t2_normal_rate_3_5", |b| {
        b.iter(|| black_box(encoder.encode(black_box(&message))));
    });

    group.finish();
}

/// Benchmark LDPC encoding for multiple blocks (sequential)
fn bench_ldpc_encode_batch(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let cache = load_cache();
    let encoder = match cache.as_ref() {
        Some(c) => LdpcEncoder::with_cache(code.clone(), c),
        None => LdpcEncoder::new(code.clone()),
    };

    let message = BitVec::zeros(encoder.k());

    let mut group = c.benchmark_group("ldpc_encode_batch");

    for batch_size in [10, 50, 100, 202].iter() {
        let messages: Vec<_> = (0..*batch_size).map(|_| message.clone()).collect();

        group.throughput(Throughput::Bytes((encoder.k() * batch_size) as u64 / 8));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    black_box(encoder.encode_batch(black_box(&messages)))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark LDPC decoding for a single block (error-free)
fn bench_ldpc_decode_single(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let mut decoder = LdpcDecoder::new(code.clone());

    // Create high-confidence LLRs (error-free channel)
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();

    let mut group = c.benchmark_group("ldpc_decode_single");
    group.throughput(Throughput::Bytes(code.k() as u64 / 8));

    group.bench_function("dvb_t2_normal_rate_3_5", |b| {
        b.iter(|| {
            decoder.reset();
            black_box(decoder.decode_iterative(black_box(&llrs), 50))
        });
    });

    group.finish();
}

/// Benchmark LDPC decoding for multiple blocks (parallel with rayon)
fn bench_ldpc_decode_batch(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();

    let mut group = c.benchmark_group("ldpc_decode_batch");

    for batch_size in [10, 50, 100, 202].iter() {
        let llr_blocks: Vec<Vec<Llr>> = (0..*batch_size).map(|_| llrs.clone()).collect();

        group.throughput(Throughput::Bytes((code.k() * batch_size) as u64 / 8));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    black_box(LdpcDecoder::decode_batch(
                        black_box(&code),
                        black_box(&llr_blocks),
                        50
                    ))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cache loading time
fn bench_cache_load(c: &mut Criterion) {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");

    if !cache_dir.exists() {
        eprintln!(
            "Cache not found at {:?}, skipping cache benchmark",
            cache_dir
        );
        return;
    }

    let mut group = c.benchmark_group("cache_operations");

    group.bench_function("load_all_12_configs", |b| {
        b.iter(|| black_box(EncodingCache::from_directory(black_box(&cache_dir)).unwrap()));
    });

    group.finish();
}

/// Benchmark encoder creation with and without cache
fn bench_encoder_creation(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let cache = load_cache();

    let mut group = c.benchmark_group("encoder_creation");

    if let Some(ref cache) = cache {
        group.bench_function("with_cache", |b| {
            b.iter(|| {
                black_box(LdpcEncoder::with_cache(
                    black_box(code.clone()),
                    black_box(cache),
                ))
            });
        });
    }

    // Skip without_cache - too slow for benchmarking (2-10 seconds)

    group.finish();
}

criterion_group!(
    benches,
    bench_ldpc_encode_single,
    bench_ldpc_encode_batch,
    bench_ldpc_decode_single,
    bench_ldpc_decode_batch,
    bench_cache_load,
    bench_encoder_creation,
);

criterion_main!(benches);
