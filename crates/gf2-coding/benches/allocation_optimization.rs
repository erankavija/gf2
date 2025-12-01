use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_coding::CodeRate;

fn bench_decode_cached_neighbors(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);

    // High SNR LLRs (should converge quickly in 1-2 iterations)
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

    // Create small batch
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(5.0f32)).collect();
    let batch: Vec<Vec<Llr>> = vec![llrs; 10];

    c.bench_function("ldpc_decode_batch_10", |b| {
        b.iter(|| LdpcDecoder::decode_batch(black_box(&code), black_box(&batch), black_box(50)));
    });
}

criterion_group!(benches, bench_decode_cached_neighbors, bench_decode_batch);
criterion_main!(benches);
