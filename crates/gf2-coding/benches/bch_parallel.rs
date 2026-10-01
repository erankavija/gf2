// BCH batch decoding benchmarks on the canonical DVB-T2 code.
//
// The code is the DVB-T2 short-frame rate-1/2 outer BCH code
// (`dvb_t2_bch_code`: n = 7200, k = 7032, t = 12), encoded and decoded
// through the canonical surface. The Criterion IDs are unchanged. Two
// measured behaviors differ from the earlier decoder:
//
// - `DvbT2BchDecoder` has no batch entry point, so the batch cells
//   (`bch_batch_decode/*` and `bch_single_vs_batch/batch_api`) loop over the
//   codewords with `decode_into` on one reused workspace and output buffer.
//   They are allocation-free per codeword (warm-reuse), where the earlier
//   batch call allocated its results.
// - `bch_single_vs_batch/single_loop` calls the allocating `decode` once per
//   codeword, so it keeps the fresh-alloc state of the earlier single loop.

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

    // DVB-T2 Short frame: k=7032, n=7200, t=12
    let code = dvb_t2_short_code();
    let decoder = OuterDecoder::new(&code);
    let mut workspace = decoder.workspace();
    let mut bbframe = BitVec::zeros(code.k());

    // Test batch sizes: 1, 10, 50, 100
    for batch_size in [1, 10, 50, 100].iter() {
        let codewords = codewords(&code, *batch_size);
        let outcome = decoder
            .decode_into(&codewords[0], &mut bbframe, &mut workspace)
            .unwrap();
        assert_eq!(outcome, BchDecodeOutcome::NoErrors);

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    decode_all(
                        &decoder,
                        black_box(&codewords),
                        &mut bbframe,
                        &mut workspace,
                    );
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

    // Single decode loop (allocating)
    group.bench_function("single_loop", |b| {
        b.iter(|| {
            let decoded: Vec<_> = codewords
                .iter()
                .map(|cw| decoder.decode(black_box(cw)).unwrap())
                .collect();
            black_box(decoded);
        });
    });

    // Batch decode (one reused workspace)
    group.bench_function("batch_api", |b| {
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
