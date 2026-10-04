//! BCH decoding benchmarks on the DVB-T2 short-frame rate-1/2 outer code
//! (`dvb_t2_bch_code`: n = 7200, k = 7032, t = 12). Every codeword is
//! error-free.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::bch::dvb_t2::{
    dvb_t2_bch_code, DvbT2BchCode, DvbT2BchDecoder as OuterDecoder, FrameSize,
};
use gf2_coding::bch::BchDecodeOutcome;
use gf2_coding::traits::block::{BlockCode, BlockEncoder};
use gf2_coding::CodeRate;
use gf2_core::BitVec;

fn dvb_t2_short_code() -> DvbT2BchCode {
    let code = dvb_t2_bch_code(FrameSize::Short, CodeRate::Rate1_2).unwrap();
    assert_eq!((code.n(), code.k()), (7200, 7032));
    code
}

/// Messages made distinct by the low 8 bits of their index, and their codewords.
fn codewords(code: &DvbT2BchCode, count: usize) -> Vec<BitVec> {
    let k = code.k();
    (0..count)
        .map(|i| {
            let mut msg = BitVec::zeros(k);
            for j in 0..8 {
                if (i >> j) & 1 == 1 {
                    msg.set(j, true);
                }
            }
            code.encode(&msg).unwrap()
        })
        .collect()
}

/// Decodes every codeword into `bbframe` on one workspace.
fn decode_all(
    decoder: &OuterDecoder<'_>,
    codewords: &[BitVec],
    bbframe: &mut BitVec,
    workspace: &mut gf2_coding::bch::dvb_t2::DvbT2DecodeWorkspace,
) {
    for cw in codewords {
        let outcome = decoder.decode_into(cw, bbframe, workspace).unwrap();
        black_box(outcome);
    }
}

fn benchmark_bch_batch_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("bch_batch_decode");

    let code = dvb_t2_short_code();
    let decoder = OuterDecoder::new(&code);

    for batch_size in [1, 10, 50, 100].iter() {
        let codewords = codewords(&code, *batch_size);
        let (outcome, _) = decoder.decode(&codewords[0]).unwrap();
        assert_eq!(outcome, BchDecodeOutcome::NoErrors);

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    let decoded: Vec<_> = black_box(&codewords)
                        .iter()
                        .map(|cw| decoder.decode(cw).unwrap())
                        .collect();
                    black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_bch_single_vs_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("bch_single_vs_batch");

    let code = dvb_t2_short_code();
    let decoder = OuterDecoder::new(&code);
    let mut workspace = decoder.workspace();
    let mut bbframe = BitVec::zeros(code.k());

    let codewords = codewords(&code, 50);

    group.bench_function("single_loop", |b| {
        b.iter(|| {
            let decoded: Vec<_> = codewords
                .iter()
                .map(|cw| decoder.decode(black_box(cw)).unwrap())
                .collect();
            black_box(decoded);
        });
    });

    group.bench_function("decode_into_loop", |b| {
        b.iter(|| {
            decode_all(
                &decoder,
                black_box(&codewords),
                &mut bbframe,
                &mut workspace,
            );
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_bch_batch_decode,
    benchmark_bch_single_vs_batch
);
criterion_main!(benches);
