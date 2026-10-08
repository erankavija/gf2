use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_coding::ldpc::{DecodedWord, LdpcBatchDecoder, LdpcCode, LdpcDecoder};
use gf2_coding::llr::Llr;
use gf2_coding::pooled::PooledBatchDecoder;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome, IterativeSoftDecoder};
use gf2_coding::{CodeRate, DecoderConfig};
use gf2_core::BitVec;

fn bench_decode_cached_neighbors(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(5.0f32)).collect();

    c.bench_function("ldpc_decode_with_cached_neighbors", |b| {
        b.iter(|| {
            let mut decoder = LdpcDecoder::new(black_box(code.clone()));
            decoder.decode_iterative(black_box(&llrs), black_box(50))
        });
    });
}

fn bench_decode_batch(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(5.0f32)).collect();
    let batch: Vec<Vec<Llr>> = vec![llrs; 10];

    let frames: Vec<&[Llr]> = batch.iter().map(Vec::as_slice).collect();
    let mut decoder = PooledBatchDecoder::new(|| {
        LdpcBatchDecoder::new(
            code.clone(),
            DecoderConfig::default(),
            50,
            DecodedWord::Message,
        )
    });
    let mut decoded = vec![BitVec::new(); frames.len()];
    let mut outcomes = vec![DecodeOutcome::default(); frames.len()];

    c.bench_function("ldpc_decode_batch_10", |b| {
        b.iter(|| {
            decoder.decode_batch(black_box(&frames), &mut decoded, &mut outcomes);
            black_box((&decoded, &outcomes));
        });
    });
}

criterion_group!(benches, bench_decode_cached_neighbors, bench_decode_batch);
criterion_main!(benches);
