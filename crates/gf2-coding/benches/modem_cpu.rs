//! Criterion benchmarks of the modem mapper and soft demapper: the Gray-QAM
//! fast path, the `ModemSpec::preferred_*` factory path, and the reference
//! path. One throughput element is one coded bit.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GrayQamMapper,
    ModemSpec, ReferenceMapper, ReferenceSoftDemapper,
};

#[path = "bench_support.rs"]
mod bench_support;
use bench_support::{deterministic_bits, deterministic_rx};

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
