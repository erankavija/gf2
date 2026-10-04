//! Batch encoding and decoding benchmarks. Each BCH benchmark appends the
//! family its measured call runs to the dispatch record `bch_workloads`
//! documents.

mod bch_workloads;

use bch_workloads::record;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::bch::encode::EncodeFamily;
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
use serde_json::json;
use std::path::PathBuf;

fn load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}

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

fn record_bch(id: String, n: usize, k: usize, batch: usize, entry: &str, family: EncodeFamily) {
    record(&json!({
        "id": id,
        "workload": "baseline-comparable",
        "n": n,
        "k": k,
        "batch": batch,
        "workers": 1,
        "entry": entry,
        "family": family.name(),
    }));
}

fn bench_bch_batch(c: &mut Criterion) {
    let code = primitive_code(14, 0b100000000101011, 25);
    assert_eq!((code.n(), code.k()), (16383, 16215));
    let k = code.k();

    let message = BitVec::zeros(k);

    let mut group = c.benchmark_group("bch_encode_pns_16383_16215");

    for batch_size in [1, 10, 50, 100].iter() {
        let messages: Vec<_> = (0..*batch_size).map(|_| message.clone()).collect();
        let total_bits = k * batch_size;

        group.throughput(Throughput::Bytes(total_bits as u64 / 8));
        record_bch(
            format!("bch_encode_pns_16383_16215/{batch_size}"),
            code.n(),
            k,
            *batch_size,
            "BchCode::encode_batch",
            code.selected_encode_family(SystematicLayout::default(), *batch_size),
        );
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
    // The single-message path runs the reference recurrence; the family seam
    // belongs to the batch entry points.
    record_bch(
        "bch_sequential_vs_batch/sequential_loop".to_owned(),
        code.n(),
        k,
        batch_size,
        "BlockEncoder::encode",
        EncodeFamily::REFERENCE,
    );
    record_bch(
        "bch_sequential_vs_batch/batch_operation".to_owned(),
        code.n(),
        k,
        batch_size,
        "BchCode::encode_batch",
        code.selected_encode_family(SystematicLayout::default(), batch_size),
    );

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
