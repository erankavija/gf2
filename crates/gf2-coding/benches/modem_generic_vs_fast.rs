//! Focused generic-vs-fast comparison benchmarks for the modem
//! mapper and soft demapper hot paths (JIT issue `1663515c`).
//!
//! # Purpose
//!
//! This bench exists to make the **reference / Gray-QAM fast-path
//! ratio** the headline result. The broader throughput matrix lives in
//! `modem_cpu.rs`; the scalar-vs-AVX2 dispatch crossover lives in
//! `cpu_dispatch_probe.rs`. This file registers matched pairs of
//! `reference/...` and `fast/...` bench items at identical input sizes
//! so downstream consumers (criterion HTML, release notes, tuning
//! sessions) can read off the multiplicative speed-up of the fast path
//! with a single glance.
//!
//! # How to read the output
//!
//! Each criterion group contains, for every `(order, batch)` pair, two
//! sibling bench items with a `reference` prefix and a `fast` prefix:
//!
//! ```text
//! modem/mapper_generic_vs_fast/reference/order16/batch1024
//! modem/mapper_generic_vs_fast/fast/order16/batch1024
//! ```
//!
//! Both items report throughput as `Throughput::Elements(batch * m)`
//! (one "element" = one coded bit), matching the convention in
//! `modem_cpu.rs`. Divide the reference timing by the fast timing to
//! obtain the per-configuration speed-up factor; this is the
//! performance baseline future SIMD and accelerator tuning work is
//! measured against.
//!
//! # Sweep
//!
//! Orders `{4, 16, 64, 256}` cover QPSK, 16-QAM, 64-QAM, and 256-QAM —
//! the full family of Gray-square QAM constellations the fast path
//! supports. Batch sizes `{1024, 16384}` are chosen as:
//!
//! * `1024` — large enough to amortize per-call setup so the inner
//!   loop dominates, still small enough to stress cache behaviour
//!   rather than main-memory bandwidth.
//! * `16384` — large enough that SIMD lanes and loop-unrolled inner
//!   kernels reach steady-state throughput, exposing the asymptotic
//!   speed-up a vectorized backend can deliver.
//!
//! Two batch sizes per order (rather than the three in `modem_cpu.rs`)
//! keeps the paired comparison legible and the full bench wall-clock
//! well inside the project's 60 s test-suite budget.
//!
//! # Complements
//!
//! * `modem_cpu.rs` — broader `(order, batch)` throughput matrix,
//!   including the shared-API factory paths.
//! * `cpu_dispatch_probe.rs` — scalar-vs-AVX2 crossover probe for the
//!   CPU dispatch layer.

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
