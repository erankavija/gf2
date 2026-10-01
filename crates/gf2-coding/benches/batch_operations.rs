// Batch Operations Benchmarks
//
// Measures performance of batch encoding/decoding with ComputeBackend.
// Run with: cargo bench --bench batch_operations
//
// The BCH groups run on the canonical construction model
// (`gf2_coding::bch::spec::BinaryBchCode`) and keep their Criterion IDs.
//
// - `bch_batch` encodes with the primitive narrow-sense binary BCH code over
//   GF(2^14) at designed distance 25 (n = 16383, k = 16215, 168 parity bits).
//   It shares its field and generator polynomial with the earlier
//   BCH(16200, 16008, 12) construction but is a different code: that
//   construction paired the 168-bit generator with a 192-bit redundancy, so
//   its length and dimension have no canonical counterpart. The measured
//   message is 207 bits longer and the redundancy 24 bits shorter. Batches go
//   through `BinaryBchCode::encode_batch`, which allocates its result
//   (fresh-alloc).
// - `bch_sequential_vs_batch` encodes with the (15, 11) code over GF(2^4) at
//   designed distance 3, the same code parameters and batch size as before.
//   `sequential_loop` calls the allocating `BlockEncoder::encode` once per
//   message and `batch_operation` calls `BinaryBchCode::encode_batch`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::bch::spec::{BinaryBchCode, DesignedDistance};
use gf2_coding::bch::SystematicLayout;
use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::block::BlockEncoder as CanonicalEncoder;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::field::extension::BinaryPrimeExt;
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

/// Builds the primitive narrow-sense binary BCH code over `GF(2^degree)`.
fn primitive_code(degree: usize, modulus: u64, designed_distance: u64) -> BinaryBchCode {
    let field = Gf2mField::new(degree, modulus).with_tables();
    let extension = BinaryPrimeExt::new(field).unwrap();
    let designed_distance = DesignedDistance::try_from(designed_distance).unwrap();
    BinaryBchCode::primitive_narrow_sense(extension, designed_distance).unwrap()
}

/// Benchmark BCH batch encoding
fn bench_bch_batch(c: &mut Criterion) {
    // Primitive narrow-sense BCH over GF(2^14), t = 12 (designed distance 25).
    let code = primitive_code(14, 0b100000000101011, 25);
    assert_eq!((code.n(), code.k()), (16383, 16215));
    let k = code.k();

    let message = BitVec::zeros(k);

    let mut group = c.benchmark_group("bch_batch");

    for batch_size in [1, 10, 50, 100].iter() {
        let messages: Vec<_> = (0..*batch_size).map(|_| message.clone()).collect();
        let total_bits = k * batch_size;

        group.throughput(Throughput::Bytes(total_bits as u64 / 8));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    black_box(
                        code.encode_batch(black_box(&messages), SystematicLayout::default())
                            .unwrap(),
                    )
                });
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
    let code = primitive_code(4, 0b10011, 3);
    assert_eq!((code.n(), code.k()), (15, 11));
    let k = code.k();

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
    group.throughput(Throughput::Bytes((k * batch_size) as u64 / 8));

    group.bench_function("sequential_loop", |b| {
        b.iter(|| {
            black_box(
                messages
                    .iter()
                    .map(|msg| CanonicalEncoder::encode(&code, msg).unwrap())
                    .collect::<Vec<_>>(),
            )
        });
    });

    group.bench_function("batch_operation", |b| {
        b.iter(|| {
            black_box(
                code.encode_batch(black_box(&messages), SystematicLayout::default())
                    .unwrap(),
            )
        });
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
