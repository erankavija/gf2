//! CPU scalar-vs-AVX2 crossover probe for the Gray-PAM distance kernel.
//!
//! Pairs with the GPU/CPU bench at
//! `crates/gf2-kernels-hip/benches/gpu_vs_cpu_gray_qam.rs`, but stays on
//! the CPU side: it measures the `f64` Gray-PAM squared-distance kernel
//! that backs `FastGrayQamDemapper` on the same `(order, batch)` sweep.
//! The full `FastGrayQamDemapper` always auto-dispatches to the best
//! available kernel (AVX2 on x86_64 hosts that advertise it, scalar
//! otherwise), so to observe the dispatch crossover we bench the raw
//! kernel bundles exposed by `gf2_kernels_simd::modem` directly against
//! a scalar reference.
//!
//! Layout choice: the bench lives in `gf2-coding/benches/` rather than
//! `gf2-kernels-simd/benches/` because `gf2-coding` already pulls
//! `criterion` and the `ModemSpec` preset machinery that generates
//! representative PAM level tables. No new dependencies are introduced.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p gf2-coding --bench cpu_dispatch_probe
//! ```
//!
//! The report backing JIT issue `9c37ec8c` reads the throughput figures
//! from this bench alongside the GPU vs CPU bench to decide whether
//! scalar hosts would see a different crossover picture.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use gf2_coding::modem::test_oracle::Lcg;
use gf2_coding::modem::ModemSpec;

use gf2_kernels_simd::modem::{detect_f64, scalar_fns_f64, GrayPamDistanceFnsF64};

/// Returns `(axis_len, pam_levels)` for a given modulation order.
///
/// `order == 2` is BPSK, mapped onto a two-level axis. Orders
/// `{4, 16, 64, 256}` are square-QAM; we read the PAM level table off
/// the Gray-QAM preset so the bench exercises the same level spacing
/// the real demapper sees.
fn axis_for_order(order: usize) -> Vec<f64> {
    if order == 2 {
        vec![-1.0, 1.0]
    } else {
        // Square Gray-QAM preset: I- and Q-axis levels are identical,
        // so read once and cast to f64.
        let spec = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        // The preset constructor validates the axis; we re-derive the
        // axis levels from first-principles here to avoid poking at
        // private fields.
        let m_half = (spec.bits_per_symbol() / 2) as u32;
        let axis_len = 1usize << m_half;
        // Standard unit-energy Gray-PAM levels {±1, ±3, ...} scaled to
        // unit average energy. We use the unscaled odd integers; the
        // kernel is insensitive to overall axis scale for the purposes
        // of the dispatch-cost crossover.
        (0..axis_len)
            .map(|l| (2 * l as isize - axis_len as isize + 1) as f64)
            .collect()
    }
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
    // Match the GPU bench sweep so the resulting tables line up 1:1.
    let orders = [4usize, 16, 64, 256];
    let batches = [256usize, 1_024, 4_096, 16_384];

    let scalar_fns: GrayPamDistanceFnsF64 = scalar_fns_f64();
    let best_fns: GrayPamDistanceFnsF64 = detect_f64();

    for &order in &orders {
        let pam = axis_for_order(order);

        let mut group = c.benchmark_group(format!("pam_sq_distance/order={order}"));
        for &batch in &batches {
            // Throughput is reported per symbol so a single "kernel call"
            // on a batch of `batch` pre-rotated I-axis samples counts as
            // `batch` elements. The demapper actually invokes the kernel
            // twice per batch (I + Q axis) for QAM, but we bench a single
            // axis call here because the AVX2 savings scale linearly with
            // axis calls.
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

criterion_group!(benches, bench_cpu_dispatch);
criterion_main!(benches);
