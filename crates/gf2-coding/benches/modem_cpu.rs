//! CPU-side criterion benchmarks for the modem mapper and demapper
//! hot-path loops (JIT issue `52112411`).
//!
//! Coverage:
//!
//! 1. `GrayQamMapper::map_bits` across orders {4, 16, 64, 256} and batch
//!    sizes {256, 4096, 16384}, including one sweep routed through the
//!    shared-API factory `ModemSpec::preferred_mapper` so the factory
//!    itself is exercised under criterion.
//! 2. `FastGrayQamDemapper::demap_llrs` across the same orders and
//!    batch sizes for both `DemapMethod::MaxLog` and
//!    `DemapMethod::ExactLogMap`.
//! 3. Reference-path baseline: `ReferenceMapper::map_bits` and
//!    `ReferenceSoftDemapper::demap_llrs` at QPSK (order 4) and
//!    16-QAM (order 16) so the reference-vs-fast performance gap is
//!    visible.
//!
//! Throughput is reported in `Throughput::Elements(batch_size * m)`:
//! one "element" is one coded bit, matching how downstream consumers
//! of the modem framework (BER/FER simulators, LDPC front-ends) reason
//! about throughput.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GrayQamMapper,
    ModemSpec, ReferenceMapper, ReferenceSoftDemapper,
};

/// Deterministic bit pattern to feed mappers. Matches the shape of the
/// patterns used by the existing modem unit tests without pulling a
/// full PRNG into the bench.
fn deterministic_bits(n_bits: usize) -> Vec<bool> {
    (0..n_bits)
        .map(|i| (i.wrapping_mul(2654435761)) & 1 == 1)
        .collect()
}

/// Deterministic received-sample scratch for the demapper bench.
fn deterministic_rx(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    // Cheap splitmix-style sequence: good enough for a bench's data
    // distribution, no dependency on `rand`.
    let mut s: u64 = 0x9E3779B97F4A7C15;
    let mut next = || -> f32 {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let v = (s >> 33) as u32;
        (v as f32) / (u32::MAX as f32) * 2.0 - 1.0
    };
    let rx_i: Vec<f32> = (0..n).map(|_| next()).collect();
    let rx_q: Vec<f32> = (0..n).map(|_| next()).collect();
    let noise_var = vec![0.25_f32; n];
    (rx_i, rx_q, noise_var)
}

const QAM_ORDERS: [usize; 4] = [4, 16, 64, 256];
const BATCH_SIZES: [usize; 3] = [256, 4096, 16384];

fn bench_gray_qam_mapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("modem/gray_qam_mapper_map_bits");
    for &order in &QAM_ORDERS {
        let mapper = GrayQamMapper::<f32>::from_preset_order(order);
        let m = mapper.spec().bits_per_symbol() as usize;
        for &batch in &BATCH_SIZES {
            let n_bits = batch * m;
            let bits = deterministic_bits(n_bits);
            let mut out_i = vec![0.0_f32; batch];
            let mut out_q = vec![0.0_f32; batch];
            group.throughput(Throughput::Elements(n_bits as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("order_{order}"), batch),
                &bits,
                |b, bits| {
                    b.iter(|| {
                        mapper.map_bits(
                            black_box(bits.as_slice()),
                            black_box(out_i.as_mut_slice()),
                            black_box(out_q.as_mut_slice()),
                        );
                    });
                },
            );
        }
    }
    group.finish();
}

/// Shared-API variant: exercise the boxed trait object returned by
/// `ModemSpec::preferred_mapper` at one representative order so the
/// factory-method path itself is bench-covered alongside the direct
/// construction path above.
fn bench_preferred_mapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("modem/preferred_mapper_map_bits");
    for &order in &[16usize, 64] {
        let spec = ModemSpec::<f32>::gray_square_qam(order);
        let mapper = spec.preferred_mapper();
        let m = mapper.spec().bits_per_symbol() as usize;
        for &batch in &[4096usize, 16384] {
            let n_bits = batch * m;
            let bits = deterministic_bits(n_bits);
            let mut out_i = vec![0.0_f32; batch];
            let mut out_q = vec![0.0_f32; batch];
            group.throughput(Throughput::Elements(n_bits as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("order_{order}"), batch),
                &bits,
                |b, bits| {
                    b.iter(|| {
                        mapper.map_bits(
                            black_box(bits.as_slice()),
                            black_box(out_i.as_mut_slice()),
                            black_box(out_q.as_mut_slice()),
                        );
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_fast_gray_qam_demapper(c: &mut Criterion) {
    for &method in &[DemapMethod::MaxLog, DemapMethod::ExactLogMap] {
        let tag = match method {
            DemapMethod::MaxLog => "max_log",
            DemapMethod::ExactLogMap => "exact_log_map",
        };
        let mut group = c.benchmark_group(format!("modem/fast_gray_qam_demapper_{tag}"));
        for &order in &QAM_ORDERS {
            let spec = ModemSpec::<f32>::gray_square_qam(order);
            let m = spec.bits_per_symbol() as usize;
            let demapper = FastGrayQamDemapper::new(spec);
            for &batch in &BATCH_SIZES {
                let (rx_i, rx_q, noise_var) = deterministic_rx(batch);
                let mut out = vec![Llr::new(0.0); batch * m];
                group.throughput(Throughput::Elements((batch * m) as u64));
                group.bench_with_input(
                    BenchmarkId::new(format!("order_{order}"), batch),
                    &method,
                    |b, &method| {
                        b.iter(|| {
                            let input = DemapInput::<f32> {
                                rx_i: &rx_i,
                                rx_q: &rx_q,
                                gain_i: None,
                                gain_q: None,
                                noise_var: &noise_var,
                                method,
                            };
                            demapper.demap_llrs(black_box(input), black_box(&mut out));
                        });
                    },
                );
            }
        }
        group.finish();
    }
}

/// Shared-API variant for the soft demapper at one representative order
/// so the factory-method construction path is bench-covered.
fn bench_preferred_soft_demapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("modem/preferred_soft_demapper_demap_llrs");
    for &order in &[16usize, 64] {
        let spec = ModemSpec::<f32>::gray_square_qam(order);
        let demapper = spec.preferred_soft_demapper();
        let m = demapper.spec().bits_per_symbol() as usize;
        for &batch in &[4096usize, 16384] {
            let (rx_i, rx_q, noise_var) = deterministic_rx(batch);
            let mut out = vec![Llr::new(0.0); batch * m];
            group.throughput(Throughput::Elements((batch * m) as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("order_{order}"), batch),
                &DemapMethod::MaxLog,
                |b, &method| {
                    b.iter(|| {
                        let input = DemapInput::<f32> {
                            rx_i: &rx_i,
                            rx_q: &rx_q,
                            gain_i: None,
                            gain_q: None,
                            noise_var: &noise_var,
                            method,
                        };
                        demapper.demap_llrs(black_box(input), black_box(&mut out));
                    });
                },
            );
        }
    }
    group.finish();
}

/// Reference-path baseline at QPSK (4) and 16-QAM so the
/// reference-vs-fast gap on the same input sizes is visible in the
/// bench output.
fn bench_reference_mapper_and_demapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("modem/reference_baseline");
    let batch = 4096usize;
    for &order in &[4usize, 16] {
        let spec = ModemSpec::<f32>::gray_square_qam(order);
        let m = spec.bits_per_symbol() as usize;
        let mapper = ReferenceMapper::new(spec.clone());
        let demapper = ReferenceSoftDemapper::new(spec);
        let n_bits = batch * m;
        let bits = deterministic_bits(n_bits);
        let mut out_i = vec![0.0_f32; batch];
        let mut out_q = vec![0.0_f32; batch];
        group.throughput(Throughput::Elements(n_bits as u64));
        group.bench_with_input(
            BenchmarkId::new("reference_mapper", format!("order_{order}")),
            &bits,
            |b, bits| {
                b.iter(|| {
                    mapper.map_bits(
                        black_box(bits.as_slice()),
                        black_box(out_i.as_mut_slice()),
                        black_box(out_q.as_mut_slice()),
                    );
                });
            },
        );

        let (rx_i, rx_q, noise_var) = deterministic_rx(batch);
        let mut out = vec![Llr::new(0.0); batch * m];
        group.bench_with_input(
            BenchmarkId::new("reference_soft_demapper", format!("order_{order}")),
            &DemapMethod::MaxLog,
            |b, &method| {
                b.iter(|| {
                    let input = DemapInput::<f32> {
                        rx_i: &rx_i,
                        rx_q: &rx_q,
                        gain_i: None,
                        gain_q: None,
                        noise_var: &noise_var,
                        method,
                    };
                    demapper.demap_llrs(black_box(input), black_box(&mut out));
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    modem_cpu_benches,
    bench_gray_qam_mapper,
    bench_preferred_mapper,
    bench_fast_gray_qam_demapper,
    bench_preferred_soft_demapper,
    bench_reference_mapper_and_demapper,
);
criterion_main!(modem_cpu_benches);
