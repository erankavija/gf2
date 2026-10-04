//! Matched `reference/...` and `fast/...` benchmark pairs for the modem mapper
//! and soft demapper at identical `(order, batch)` inputs. One throughput
//! element is one coded bit.

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
const BATCH_SIZES: [usize; 2] = [1024, 16384];

fn bench_mapper_generic_vs_fast(c: &mut Criterion) {
    let mut group = c.benchmark_group("modem/mapper_generic_vs_fast");
    for &order in &QAM_ORDERS {
        let spec = ModemSpec::<f32>::gray_square_qam(order);
        let m = spec.bits_per_symbol() as usize;
        let reference = ReferenceMapper::new(spec.clone());
        let fast = GrayQamMapper::<f32>::from_preset_order(order);
        for &batch in &BATCH_SIZES {
            let n_bits = batch * m;
            let bits = deterministic_bits(n_bits);
            let mut out_i_ref = vec![0.0_f32; batch];
            let mut out_q_ref = vec![0.0_f32; batch];
            let mut out_i_fast = vec![0.0_f32; batch];
            let mut out_q_fast = vec![0.0_f32; batch];
            group.throughput(Throughput::Elements(n_bits as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("reference/order{order}"), format!("batch{batch}")),
                &bits,
                |b, bits| {
                    b.iter(|| {
                        reference.map_bits(
                            black_box(bits.as_slice()),
                            black_box(out_i_ref.as_mut_slice()),
                            black_box(out_q_ref.as_mut_slice()),
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new(format!("fast/order{order}"), format!("batch{batch}")),
                &bits,
                |b, bits| {
                    b.iter(|| {
                        fast.map_bits(
                            black_box(bits.as_slice()),
                            black_box(out_i_fast.as_mut_slice()),
                            black_box(out_q_fast.as_mut_slice()),
                        );
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_soft_demapper_generic_vs_fast(c: &mut Criterion) {
    let mut group = c.benchmark_group("modem/soft_demapper_generic_vs_fast");
    for &method in &[DemapMethod::MaxLog, DemapMethod::ExactLogMap] {
        let (ref_tag, fast_tag) = match method {
            DemapMethod::MaxLog => ("reference_max_log", "fast_max_log"),
            DemapMethod::ExactLogMap => ("reference_exact_log_map", "fast_exact_log_map"),
        };
        for &order in &QAM_ORDERS {
            let spec = ModemSpec::<f32>::gray_square_qam(order);
            let m = spec.bits_per_symbol() as usize;
            let reference = ReferenceSoftDemapper::new(spec.clone());
            let fast = FastGrayQamDemapper::new(spec);
            for &batch in &BATCH_SIZES {
                let (rx_i, rx_q, noise_var) = deterministic_rx(batch);
                let mut out_ref = vec![Llr::new(0.0); batch * m];
                let mut out_fast = vec![Llr::new(0.0); batch * m];
                group.throughput(Throughput::Elements((batch * m) as u64));

                group.bench_with_input(
                    BenchmarkId::new(format!("{ref_tag}/order{order}"), format!("batch{batch}")),
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
                            reference.demap_llrs(black_box(input), black_box(&mut out_ref));
                        });
                    },
                );

                group.bench_with_input(
                    BenchmarkId::new(format!("{fast_tag}/order{order}"), format!("batch{batch}")),
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
                            fast.demap_llrs(black_box(input), black_box(&mut out_fast));
                        });
                    },
                );
            }
        }
    }
    group.finish();
}

criterion_group!(
    modem_generic_vs_fast_benches,
    bench_mapper_generic_vs_fast,
    bench_soft_demapper_generic_vs_fast,
);
criterion_main!(modem_generic_vs_fast_benches);
