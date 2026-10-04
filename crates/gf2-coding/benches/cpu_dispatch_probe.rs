//! CPU scalar-vs-AVX2 probe of the `f64` Gray-PAM squared-distance kernel
//! behind `FastGrayQamDemapper`, on the `(order, batch)` sweep of
//! `crates/gf2-kernels-hip/benches/gpu_vs_cpu_gray_qam.rs`. The demapper
//! auto-dispatches, so the raw `gf2_kernels_simd::modem` kernel bundles are
//! benched against a scalar reference.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use gf2_coding::llr::Llr;
use gf2_coding::modem::test_oracle::Lcg;
use gf2_coding::modem::{
    BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, ModemSpec,
};

use gf2_kernels_simd::modem::{detect_f64, scalar_fns_f64, GrayPamDistanceFnsF64};

/// PAM level table of `order`, read off the Gray-QAM fast-path demapper so
/// both kernels see the production axis.
fn axis_for_order(order: usize) -> Vec<f64> {
    let spec = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
    let demapper = FastGrayQamDemapper::<f64>::new(spec);
    demapper.pam_levels().to_vec()
}

fn gen_batch_f64(batch: usize, seed: u64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut rng = Lcg::new(seed);
    let mut z = Vec::with_capacity(batch);
    let mut g = Vec::with_capacity(batch);
    let mut inv = Vec::with_capacity(batch);
    for _ in 0..batch {
        z.push((rng.next_unit_f32() as f64) * 2.0);
        g.push(1.0);
        inv.push(1.0 / (rng.next_positive_f32(0.05, 2.0) as f64));
    }
    (z, g, inv)
}

fn bench_cpu_dispatch(c: &mut Criterion) {
    let orders = [4usize, 16, 64, 256];
    let batches = [256usize, 1_024, 4_096, 16_384];

    let scalar_fns: GrayPamDistanceFnsF64 = scalar_fns_f64();
    let best_fns: GrayPamDistanceFnsF64 = detect_f64();

    for &order in &orders {
        let pam = axis_for_order(order);

        let mut group = c.benchmark_group(format!("pam_sq_distance/order={order}"));
        for &batch in &batches {
            // One element is one sample of a single-axis kernel call.
            group.throughput(Throughput::Elements(batch as u64));
            let (z, g, inv) = gen_batch_f64(batch, 0xBEEF_u64 ^ order as u64);
            let mut out = vec![0.0f64; batch * pam.len()];

            group.bench_with_input(BenchmarkId::new("scalar", batch), &batch, |b, _| {
                b.iter(|| {
                    (scalar_fns.pam_sq_distances_fn)(&z, &g, &inv, &pam, &mut out);
                    black_box(&out);
                });
            });

            group.bench_with_input(BenchmarkId::new("best", batch), &batch, |b, _| {
                b.iter(|| {
                    (best_fns.pam_sq_distances_fn)(&z, &g, &inv, &pam, &mut out);
                    black_box(&out);
                });
            });
        }
        group.finish();
    }
}

/// `demap_llrs` end to end on two `FastGrayQamDemapper<f32>` instances of one
/// spec, one pinned to the scalar PAM distance kernel and one auto-dispatched.
fn bench_full_demapper_scalar_vs_best(c: &mut Criterion) {
    let orders = [4usize, 16, 64, 256];
    let batches = [256usize, 1_024, 4_096, 16_384];

    for &order in &orders {
        let spec = ModemSpec::<f32>::gray_square_qam(order);
        let bits_per_symbol = spec.bits_per_symbol() as usize;
        let demap_best = FastGrayQamDemapper::<f32>::new(spec.clone());
        let demap_scalar = FastGrayQamDemapper::<f32>::new_with_scalar_kernel(spec);

        let mut group = c.benchmark_group(format!("full_demapper/order={order}"));
        for &batch in &batches {
            group.throughput(Throughput::Elements((batch * bits_per_symbol) as u64));
            let mut rng = Lcg::new(0xDECAF_u64 ^ order as u64);
            let rx_i: Vec<f32> = (0..batch).map(|_| rng.next_unit_f32()).collect();
            let rx_q: Vec<f32> = (0..batch).map(|_| rng.next_unit_f32()).collect();
            let noise_var = vec![0.25_f32; batch];
            let mut out = vec![Llr::new(0.0); batch * bits_per_symbol];

            group.bench_with_input(BenchmarkId::new("scalar", batch), &batch, |b, _| {
                b.iter(|| {
                    let input = DemapInput::<f32> {
                        rx_i: &rx_i,
                        rx_q: &rx_q,
                        gain_i: None,
                        gain_q: None,
                        noise_var: &noise_var,
                        method: DemapMethod::MaxLog,
                    };
                    demap_scalar.demap_llrs(input, &mut out);
                    black_box(&out);
                });
            });

            group.bench_with_input(BenchmarkId::new("best", batch), &batch, |b, _| {
                b.iter(|| {
                    let input = DemapInput::<f32> {
                        rx_i: &rx_i,
                        rx_q: &rx_q,
                        gain_i: None,
                        gain_q: None,
                        noise_var: &noise_var,
                        method: DemapMethod::MaxLog,
                    };
                    demap_best.demap_llrs(input, &mut out);
                    black_box(&out);
                });
            });
        }
        group.finish();
    }
}

criterion_group!(
    benches,
    bench_cpu_dispatch,
    bench_full_demapper_scalar_vs_best
);
criterion_main!(benches);
