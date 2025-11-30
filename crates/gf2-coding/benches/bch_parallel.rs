use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::bch::dvb_t2::FrameSize;
use gf2_coding::bch::{BchCode, BchDecoder, BchEncoder, CodeRate};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

fn benchmark_bch_batch_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("bch_batch_decode");

    // DVB-T2 Short frame: k=7032, n=7200, t=12
    let bch = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
    let k = bch.k();
    let encoder = BchEncoder::new(bch.clone());
    let decoder = BchDecoder::new(bch);

    // Test batch sizes: 1, 10, 50, 100
    for batch_size in [1, 10, 50, 100].iter() {
        // Generate test messages
        let messages: Vec<BitVec> = (0..*batch_size)
            .map(|i| {
                let mut msg = BitVec::zeros(k);
                // Set some bits to make messages distinct
                for j in 0..8 {
                    if (i >> j) & 1 == 1 {
                        msg.set(j, true);
                    }
                }
                msg
            })
            .collect();

        // Encode messages
        let codewords: Vec<BitVec> = messages.iter().map(|m| encoder.encode(m)).collect();

        // Benchmark decode_batch
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    let decoded = decoder.decode_batch(black_box(&codewords));
                    black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_bch_single_vs_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("bch_single_vs_batch");

    let bch = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
    let k = bch.k();
    let encoder = BchEncoder::new(bch.clone());
    let decoder = BchDecoder::new(bch);

    let batch_size = 50;
    let messages: Vec<BitVec> = (0..batch_size)
        .map(|i| {
            let mut msg = BitVec::zeros(k);
            for j in 0..8 {
                if (i >> j) & 1 == 1 {
                    msg.set(j, true);
                }
            }
            msg
        })
        .collect();

    let codewords: Vec<BitVec> = messages.iter().map(|m| encoder.encode(m)).collect();

    // Single decode loop
    group.bench_function("single_loop", |b| {
        b.iter(|| {
            let decoded: Vec<_> = codewords
                .iter()
                .map(|cw| decoder.decode(black_box(cw)))
                .collect();
            black_box(decoded);
        });
    });

    // Batch decode
    group.bench_function("batch_api", |b| {
        b.iter(|| {
            let decoded = decoder.decode_batch(black_box(&codewords));
            black_box(decoded);
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_bch_batch_decode, benchmark_bch_single_vs_batch);
criterion_main!(benches);
