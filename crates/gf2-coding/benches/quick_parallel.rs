//! Quick parallel scaling benchmark (runs in <1 minute)
//!
//! Measures thread scaling with small batch sizes for fast feedback.
//! Run with: RAYON_NUM_THREADS=N cargo bench --bench quick_parallel --features parallel
//!
//! Examples:
//!   RAYON_NUM_THREADS=1 cargo bench --bench quick_parallel --features parallel
//!   RAYON_NUM_THREADS=8 cargo bench --bench quick_parallel --features parallel

use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(feature = "parallel")]
use criterion::{black_box, BenchmarkId, Throughput};
#[cfg(feature = "parallel")]
use gf2_coding::ldpc::encoding::EncodingCache;
#[cfg(feature = "parallel")]
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder};
#[cfg(feature = "parallel")]
use gf2_coding::llr::Llr;
#[cfg(feature = "parallel")]
use gf2_coding::traits::BlockEncoder;
#[cfg(feature = "parallel")]
use gf2_coding::CodeRate;
#[cfg(feature = "parallel")]
use gf2_core::BitVec;
#[cfg(feature = "parallel")]
use std::path::PathBuf;

#[cfg(feature = "parallel")]
fn load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}

/// Quick LDPC encode benchmark (small batch)
#[cfg(feature = "parallel")]
fn bench_ldpc_encode_quick(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let cache = load_cache();
    let encoder = match cache.as_ref() {
        Some(c) => LdpcEncoder::with_cache(code.clone(), c),
        None => LdpcEncoder::new(code.clone()),
    };

    let message = BitVec::zeros(encoder.k());
    let batch_size = 10; // Small batch for quick runs
    let messages: Vec<_> = (0..batch_size).map(|_| message.clone()).collect();

    let mut group = c.benchmark_group("ldpc_encode_quick");
    group.throughput(Throughput::Bytes((encoder.k() * batch_size) as u64 / 8));

    group.bench_function("encode_batch_10", |b| {
        b.iter(|| black_box(encoder.encode_batch(black_box(&messages))));
    });

    group.finish();
}

/// Quick LDPC decode benchmark (small batch, few iterations)
#[cfg(feature = "parallel")]
fn bench_ldpc_decode_quick(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();
    let batch_size = 10; // Small batch for quick runs
    let llr_blocks: Vec<Vec<Llr>> = (0..batch_size).map(|_| llrs.clone()).collect();

    let mut group = c.benchmark_group("ldpc_decode_quick");
    group.throughput(Throughput::Bytes((code.k() * batch_size) as u64 / 8));

    group.bench_function("decode_batch_10", |b| {
        b.iter(|| {
            black_box(LdpcDecoder::decode_batch(
                black_box(&code),
                black_box(&llr_blocks),
                20, // Fewer iterations for speed
            ))
        });
    });

    group.finish();
}

/// Compare different batch sizes
#[cfg(feature = "parallel")]
fn bench_batch_sizes(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();

    let mut group = c.benchmark_group("batch_size_scaling");

    for batch_size in [1, 5, 10, 20].iter() {
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
                        20,
                    ))
                });
            },
        );
    }

    group.finish();
}

/// Print current thread configuration
#[cfg(feature = "parallel")]
fn print_config(c: &mut Criterion) {
    use std::env;

    let num_threads = env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| num_cpus::get());

    c.bench_function("_print_config", |b| {
        eprintln!("\n=== Benchmark Configuration ===");
        eprintln!("RAYON_NUM_THREADS: {}", num_threads);
        eprintln!("Physical cores: {}", num_cpus::get_physical());
        eprintln!("Logical cores: {}", num_cpus::get());
        eprintln!("Batch sizes: 1, 5, 10, 20");
        eprintln!("LDPC iterations: 20 (reduced for speed)");
        eprintln!("================================\n");

        b.iter(|| {});
    });
}

#[cfg(feature = "parallel")]
criterion_group!(
    benches,
    print_config,
    bench_ldpc_encode_quick,
    bench_ldpc_decode_quick,
    bench_batch_sizes,
);

#[cfg(not(feature = "parallel"))]
fn no_parallel_warning(_: &mut Criterion) {
    eprintln!("This benchmark requires the 'parallel' feature.");
    eprintln!("Run with: cargo bench --bench quick_parallel --features parallel");
}

#[cfg(not(feature = "parallel"))]
criterion_group!(benches, no_parallel_warning);

criterion_main!(benches);
