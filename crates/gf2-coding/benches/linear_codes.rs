//! Benchmarks for linear block codes.
//!
//! These benchmarks measure the performance of encoding, syndrome computation,
//! and decoding across different code parameters.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_core::BitVec;

fn create_message(k: usize, pattern: usize) -> BitVec {
    let mut msg = BitVec::new();
    for i in 0..k {
        msg.push_bit((i + pattern) % 3 == 0);
    }
    msg
}

fn bench_hamming_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("hamming_encode");

    for r in [3, 4, 5, 6] {
        let code = LinearBlockCode::hamming(r);
        let n = code.n();
        let k = code.k();
        let msg = create_message(k, 0);

        group.throughput(Throughput::Bytes(k as u64 / 8));
        group.bench_with_input(BenchmarkId::new("single", format!("({},{})", n, k)), &code, |b, code| {
            b.iter(|| {
                let codeword = code.encode(black_box(&msg));
                black_box(codeword);
            });
        });
    }

    group.finish();
}

fn bench_hamming_encode_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("hamming_encode_batch");

    for (r, batch_size) in [(3, 10_000), (4, 10_000), (5, 1_000)] {
        let code = LinearBlockCode::hamming(r);
        let n = code.n();
        let k = code.k();
        
        let messages: Vec<BitVec> = (0..batch_size)
            .map(|i| create_message(k, i))
            .collect();

        group.throughput(Throughput::Bytes((k * batch_size) as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("batch", format!("({},{})_x{}", n, k, batch_size)),
            &code,
            |b, code| {
                b.iter(|| {
                    for msg in &messages {
                        let codeword = code.encode(black_box(msg));
                        black_box(codeword);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_syndrome_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("syndrome_computation");

    for r in [3, 4, 5, 6] {
        let code = LinearBlockCode::hamming(r);
        let n = code.n();
        let k = code.k();
        let msg = create_message(k, 0);
        let codeword = code.encode(&msg);

        group.throughput(Throughput::Bytes(n as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("syndrome", format!("({},{})", n, k)),
            &code,
            |b, code| {
                b.iter(|| {
                    let syndrome = code.syndrome(black_box(&codeword));
                    black_box(syndrome);
                });
            },
        );
    }

    group.finish();
}

fn bench_decoder_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("decoder_construction");

    for r in [3, 4, 5, 6, 7] {
        let code = LinearBlockCode::hamming(r);
        let n = code.n();
        let k = code.k();

        group.bench_with_input(
            BenchmarkId::new("syndrome_table", format!("({},{})", n, k)),
            &code,
            |b, code| {
                b.iter(|| {
                    let decoder = SyndromeTableDecoder::new(black_box(code.clone()));
                    black_box(decoder);
                });
            },
        );
    }

    group.finish();
}

fn bench_decode_no_error(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_no_error");

    for r in [3, 4, 5, 6] {
        let code = LinearBlockCode::hamming(r);
        let decoder = SyndromeTableDecoder::new(code.clone());
        let n = code.n();
        let k = code.k();
        
        let msg = create_message(k, 0);
        let codeword = code.encode(&msg);

        group.throughput(Throughput::Bytes(n as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("decode", format!("({},{})", n, k)),
            &decoder,
            |b, decoder| {
                b.iter(|| {
                    let decoded = decoder.decode(black_box(&codeword));
                    black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

fn bench_decode_with_error(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_with_error");

    for r in [3, 4, 5, 6] {
        let code = LinearBlockCode::hamming(r);
        let decoder = SyndromeTableDecoder::new(code.clone());
        let n = code.n();
        let k = code.k();
        
        let msg = create_message(k, 0);
        let mut corrupted = code.encode(&msg);
        
        // Introduce error at middle position
        let error_pos = n / 2;
        corrupted.set(error_pos, !corrupted.get(error_pos));

        group.throughput(Throughput::Bytes(n as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("decode_corrected", format!("({},{})", n, k)),
            &decoder,
            |b, decoder| {
                b.iter(|| {
                    let decoded = decoder.decode(black_box(&corrupted));
                    black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

fn bench_decode_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_batch");

    for (r, batch_size) in [(3, 10_000), (4, 10_000), (5, 1_000)] {
        let code = LinearBlockCode::hamming(r);
        let decoder = SyndromeTableDecoder::new(code.clone());
        let n = code.n();
        let k = code.k();
        
        let codewords: Vec<BitVec> = (0..batch_size)
            .map(|i| {
                let msg = create_message(k, i);
                let mut codeword = code.encode(&msg);
                
                // Introduce error in half of them
                if i % 2 == 0 {
                    let error_pos = (i * 7) % n;
                    codeword.set(error_pos, !codeword.get(error_pos));
                }
                
                codeword
            })
            .collect();

        group.throughput(Throughput::Bytes((n * batch_size) as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("batch", format!("({},{})_x{}", n, k, batch_size)),
            &decoder,
            |b, decoder| {
                b.iter(|| {
                    for codeword in &codewords {
                        let decoded = decoder.decode(black_box(codeword));
                        black_box(decoded);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_encode_decode_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_decode_roundtrip");

    for r in [3, 4, 5, 6] {
        let code = LinearBlockCode::hamming(r);
        let decoder = SyndromeTableDecoder::new(code.clone());
        let n = code.n();
        let k = code.k();
        
        let msg = create_message(k, 42);

        group.throughput(Throughput::Bytes(k as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("roundtrip", format!("({},{})", n, k)),
            &(&code, &decoder),
            |b, (code, decoder)| {
                b.iter(|| {
                    let codeword = code.encode(black_box(&msg));
                    let decoded = decoder.decode(black_box(&codeword));
                    black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

fn bench_project_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("project_message");

    for r in [3, 4, 5, 6] {
        let code = LinearBlockCode::hamming(r);
        let n = code.n();
        let k = code.k();
        
        let msg = create_message(k, 0);
        let codeword = code.encode(&msg);

        group.throughput(Throughput::Bytes(k as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("project", format!("({},{})", n, k)),
            &code,
            |b, code| {
                b.iter(|| {
                    let extracted = code.project_message(black_box(&codeword));
                    black_box(extracted);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_hamming_encode,
    bench_hamming_encode_batch,
    bench_syndrome_computation,
    bench_decoder_construction,
    bench_decode_no_error,
    bench_decode_with_error,
    bench_decode_batch,
    bench_encode_decode_roundtrip,
    bench_project_message,
);
criterion_main!(benches);
