//! Decoding benchmarks for each `DecoderAlgorithm` variant on a DVB-T2
//! short-frame LDPC code.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_coding::CodeRate;

/// DVB-T2 short-frame LDPC code and high-SNR LLRs of the all-zero codeword.
fn setup_short_frame() -> (LdpcCode, Vec<Llr>) {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0f32)).collect();
    (code, llrs)
}

fn bench_decode_algorithms(c: &mut Criterion) {
    let (code, llrs) = setup_short_frame();
    let k_bits = code.k();
    let max_iterations = 50;

    let algorithms: Vec<(&str, DecoderAlgorithm)> = vec![
        ("MinSum", DecoderAlgorithm::MinSum),
        (
            "NormalizedMinSum_0.875",
            DecoderAlgorithm::NormalizedMinSum(0.875),
        ),
        ("OffsetMinSum_0.5", DecoderAlgorithm::OffsetMinSum(0.5)),
        ("SumProduct", DecoderAlgorithm::SumProduct),
    ];

    let mut group = c.benchmark_group("ldpc_decode_algorithm");
    group.throughput(Throughput::Elements(k_bits as u64));

    for (name, algo) in &algorithms {
        let config = DecoderConfig::new(*algo, true);

        group.bench_with_input(BenchmarkId::new("short_r1_2", name), algo, |b, _algo| {
            let mut decoder = LdpcDecoder::with_config(code.clone(), config);
            b.iter(|| {
                decoder.reset();
                black_box(decoder.decode_iterative(black_box(&llrs), max_iterations))
            });
        });
    }

    group.finish();
}

fn bench_decode_multiframe(c: &mut Criterion) {
    let (code, llrs) = setup_short_frame();
    let k_bits = code.k();
    let frames = 10;
    let max_iterations = 50;

    let algorithms: Vec<(&str, DecoderAlgorithm)> = vec![
        ("MinSum", DecoderAlgorithm::MinSum),
        (
            "NormalizedMinSum_0.875",
            DecoderAlgorithm::NormalizedMinSum(0.875),
        ),
        ("OffsetMinSum_0.5", DecoderAlgorithm::OffsetMinSum(0.5)),
    ];

    let mut group = c.benchmark_group("ldpc_decode_multiframe");
    group.throughput(Throughput::Elements((k_bits * frames) as u64));

    for (name, algo) in &algorithms {
        let config = DecoderConfig::new(*algo, true);

        group.bench_with_input(
            BenchmarkId::new(format!("{}x_short_r1_2", frames), name),
            algo,
            |b, _algo| {
                let mut decoder = LdpcDecoder::with_config(code.clone(), config);
                b.iter(|| {
                    for _ in 0..frames {
                        decoder.reset();
                        black_box(decoder.decode_iterative(black_box(&llrs), max_iterations));
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_early_termination(c: &mut Criterion) {
    let (code, llrs) = setup_short_frame();
    let k_bits = code.k();
    let max_iterations = 50;

    let mut group = c.benchmark_group("ldpc_decode_early_term");
    group.throughput(Throughput::Elements(k_bits as u64));

    let config_early = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
    group.bench_function("early_termination_on", |b| {
        let mut decoder = LdpcDecoder::with_config(code.clone(), config_early);
        b.iter(|| {
            decoder.reset();
            black_box(decoder.decode_iterative(black_box(&llrs), max_iterations))
        });
    });

    let config_no_early = DecoderConfig::new(DecoderAlgorithm::MinSum, false);
    group.bench_function("early_termination_off", |b| {
        let mut decoder = LdpcDecoder::with_config(code.clone(), config_no_early);
        b.iter(|| {
            decoder.reset();
            black_box(decoder.decode_iterative(black_box(&llrs), max_iterations))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_decode_algorithms,
    bench_decode_multiframe,
    bench_early_termination,
);

criterion_main!(benches);
