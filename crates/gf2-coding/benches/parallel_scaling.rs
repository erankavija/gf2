// Parallel Scaling Benchmarks
//
// Measures parallel performance scaling with different thread counts.
// Run with: cargo bench --bench parallel_scaling --features parallel
//
// Set thread count with environment variable:
//   RAYON_NUM_THREADS=8 cargo bench --bench parallel_scaling --features parallel

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

/// Load LDPC cache from standard location
fn load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}

/// Configure rayon thread pool with specified number of threads
#[cfg(feature = "parallel")]
fn configure_threads(num_threads: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .ok(); // Ignore error if already configured
}

/// Benchmark LDPC decoding with different thread counts
#[cfg(feature = "parallel")]
fn bench_ldpc_decode_thread_scaling(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();
    let batch_size = 100;
    let llr_blocks: Vec<Vec<Llr>> = (0..batch_size).map(|_| llrs.clone()).collect();

    let mut group = c.benchmark_group("ldpc_decode_thread_scaling");
    group.throughput(Throughput::Bytes((code.k() * batch_size) as u64 / 8));

    // Get number of physical cores (not including hyperthreading)
    let physical_cores = num_cpus::get_physical();

    // Benchmark with different thread counts: 1, 2, 4, 8, physical_cores, all
    let thread_counts: Vec<usize> = vec![1, 2, 4, 8]
        .into_iter()
        .filter(|&n| n <= physical_cores)
        .chain(std::iter::once(physical_cores))
        .chain(std::iter::once(num_cpus::get()))
        .collect();

    for num_threads in thread_counts {
        configure_threads(num_threads);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, _| {
                b.iter(|| {
                    black_box(LdpcDecoder::decode_batch(
                        black_box(&code),
                        black_box(&llr_blocks),
                        50,
                    ))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark LDPC encoding with different thread counts
#[cfg(feature = "parallel")]
fn bench_ldpc_encode_thread_scaling(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let cache = load_cache();
    let encoder = match cache.as_ref() {
        Some(c) => LdpcEncoder::with_cache(code.clone(), c),
        None => LdpcEncoder::new(code.clone()),
    };

    let message = BitVec::zeros(encoder.k());
    let batch_size = 100;
    let messages: Vec<_> = (0..batch_size).map(|_| message.clone()).collect();

    let mut group = c.benchmark_group("ldpc_encode_thread_scaling");
    group.throughput(Throughput::Bytes((encoder.k() * batch_size) as u64 / 8));

    let physical_cores = num_cpus::get_physical();
    let thread_counts: Vec<usize> = vec![1, 2, 4, 8]
        .into_iter()
        .filter(|&n| n <= physical_cores)
        .chain(std::iter::once(physical_cores))
        .chain(std::iter::once(num_cpus::get()))
        .collect();

    for num_threads in thread_counts {
        configure_threads(num_threads);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, _| {
                b.iter(|| black_box(encoder.encode_batch(black_box(&messages))));
            },
        );
    }

    group.finish();
}

/// Benchmark parallel vs sequential for different batch sizes
#[cfg(feature = "parallel")]
fn bench_parallel_vs_sequential(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();

    let mut group = c.benchmark_group("parallel_vs_sequential");

    for batch_size in [10, 50, 100, 202].iter() {
        let llr_blocks: Vec<Vec<Llr>> = (0..*batch_size).map(|_| llrs.clone()).collect();
        group.throughput(Throughput::Bytes((code.k() * batch_size) as u64 / 8));

        // Sequential (1 thread)
        configure_threads(1);
        group.bench_with_input(
            BenchmarkId::new("sequential", batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    black_box(LdpcDecoder::decode_batch(
                        black_box(&code),
                        black_box(&llr_blocks),
                        50,
                    ))
                });
            },
        );

        // Parallel (all threads)
        configure_threads(num_cpus::get());
        group.bench_with_input(
            BenchmarkId::new("parallel", batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    black_box(LdpcDecoder::decode_batch(
                        black_box(&code),
                        black_box(&llr_blocks),
                        50,
                    ))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark with optimal thread count (physical cores)
#[cfg(feature = "parallel")]
fn bench_optimal_threads(c: &mut Criterion) {
    let physical_cores = num_cpus::get_physical();
    configure_threads(physical_cores);

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();
    let batch_size = 202;
    let llr_blocks: Vec<Vec<Llr>> = (0..batch_size).map(|_| llrs.clone()).collect();

    let mut group = c.benchmark_group("optimal_configuration");
    group.throughput(Throughput::Bytes((code.k() * batch_size) as u64 / 8));

    group.bench_function(format!("{}_physical_cores", physical_cores), |b| {
        b.iter(|| {
            black_box(LdpcDecoder::decode_batch(
                black_box(&code),
                black_box(&llr_blocks),
                50,
            ))
        });
    });

    group.finish();

    // Print throughput summary
    eprintln!("\n=== Configuration ===");
    eprintln!("Physical cores: {}", physical_cores);
    eprintln!("Logical cores: {}", num_cpus::get());
    eprintln!("Batch size: {}", batch_size);
    eprintln!("Code: DVB-T2 Normal Rate 3/5");
}

#[cfg(feature = "parallel")]
criterion_group!(
    benches,
    bench_ldpc_decode_thread_scaling,
    bench_ldpc_encode_thread_scaling,
    bench_parallel_vs_sequential,
    bench_optimal_threads,
);

#[cfg(not(feature = "parallel"))]
fn no_parallel_warning(_: &mut Criterion) {
    eprintln!("This benchmark requires the 'parallel' feature.");
    eprintln!("Run with: cargo bench --bench parallel_scaling --features parallel");
}

#[cfg(not(feature = "parallel"))]
criterion_group!(benches, no_parallel_warning);

criterion_main!(benches);
