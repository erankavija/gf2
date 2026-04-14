//! CPU/GPU crossover benchmark for the Gray square-QAM max-log demapper.
//!
//! Compares `FastGrayQamDemapper<f32>` (CPU) against `GpuGrayQamSoftDemapper`
//! (HIP/ROCm) across a range of batch sizes and modulation orders. This is
//! the ready-made driver for the crossover measurement tracked in JIT
//! issue `9c37ec8c`.
//!
//! Run with:
//!
//! ```text
//! cargo bench --manifest-path crates/gf2-kernels-hip/Cargo.toml \
//!     --bench gpu_vs_cpu_gray_qam
//! ```
//!
//! The GPU demapper is warmed up once before timed iterations so the
//! first-launch JIT/driver cost is not attributed to the measurement.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use gf2_coding::llr::Llr;
use gf2_coding::modem::test_oracle::Lcg;
use gf2_coding::modem::{
    BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GpuGrayQamSoftDemapper,
    ModemSpec,
};

fn spec_for_order(order: usize) -> ModemSpec<f32> {
    if order == 2 {
        ModemSpec::<f32>::bpsk()
    } else {
        ModemSpec::<f32>::gray_square_qam(order)
    }
}

fn gen_batch(order: usize, batch: usize, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut rng = Lcg::new(seed ^ (order as u64));
    let mut rx_i = Vec::with_capacity(batch);
    let mut rx_q = Vec::with_capacity(batch);
    let mut nv = Vec::with_capacity(batch);
    for _ in 0..batch {
        rx_i.push(rng.next_unit_f32() * 2.0);
        rx_q.push(rng.next_unit_f32() * 2.0);
        nv.push(rng.next_positive_f32(0.05, 2.0));
    }
    (rx_i, rx_q, nv)
}

fn bench_gpu_vs_cpu(c: &mut Criterion) {
    // Moderate range — the crossover is the point of interest, not a
    // saturated sweep.
    let orders = [4usize, 16, 64, 256];
    let batches = [256usize, 1_024, 4_096, 16_384];

    for &order in &orders {
        let spec = spec_for_order(order);
        let m = spec.bits_per_symbol() as usize;

        let mut group = c.benchmark_group(format!("gray_qam_demap_max_log/order={order}"));
        for &batch in &batches {
            group.throughput(Throughput::Elements(batch as u64));
            let (rx_i, rx_q, nv) = gen_batch(order, batch, 0xBEEF_u64);
            let input = DemapInput::<f32> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::MaxLog,
            };

            let cpu = FastGrayQamDemapper::<f32>::new(spec.clone());
            let mut out_cpu = vec![Llr::new(0.0); batch * m];
            group.bench_with_input(BenchmarkId::new("cpu", batch), &input, |b, input_ref| {
                b.iter(|| {
                    cpu.demap_llrs(*input_ref, &mut out_cpu);
                    black_box(&out_cpu);
                });
            });

            // Construct GPU demapper per configuration; allocation is
            // outside the timed region, matching the CPU pattern.
            let gpu = match GpuGrayQamSoftDemapper::new(spec.clone(), batch) {
                Ok(g) => g,
                Err(e) => {
                    // No device available — skip the GPU leg rather than
                    // fail the whole bench run.
                    eprintln!(
                        "gpu_vs_cpu_gray_qam: skipping GPU (order={order}, batch={batch}): {e:?}"
                    );
                    continue;
                }
            };
            let mut out_gpu = vec![Llr::new(0.0); batch * m];
            // Warm-up pass (untimed).
            gpu.demap_llrs(input, &mut out_gpu);
            group.bench_with_input(BenchmarkId::new("gpu", batch), &input, |b, input_ref| {
                b.iter(|| {
                    gpu.demap_llrs(*input_ref, &mut out_gpu);
                    black_box(&out_gpu);
                });
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_gpu_vs_cpu);
criterion_main!(benches);
