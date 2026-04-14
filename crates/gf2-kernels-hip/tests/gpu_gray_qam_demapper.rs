//! Cross-validation: GPU Gray-QAM max-log demapper vs CPU fast path.
//!
//! The GPU kernel implements the max-log variant. Comparison is against
//! `FastGrayQamDemapper` with `DemapMethod::MaxLog`, which is the CPU
//! numerical oracle for the same algorithm. Tolerance is set tightly
//! (1e-3) because both sides are arithmetically identical up to f32
//! rounding on the distance computation.
//!
//! Throughput probe `bench_gpu_vs_cpu_gray_qam_16qam` records wall-clock
//! time for a representative batch and prints a ratio; it is the
//! prototype hook used by the JIT-9c37ec8c crossover measurement.

use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GpuGrayQamSoftDemapper,
    ModemSpec,
};
use std::time::Instant;

/// Cheap deterministic LCG (matches the pattern used in
/// `fast_gray_qam_demapper.rs` tests).
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }
    fn next_unit(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    fn next_positive(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next_u32() as f32 / u32::MAX as f32) * (hi - lo)
    }
}

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
    // Scale of 2.0 keeps samples well inside [-2, +2] which covers the
    // unit-average-energy Gray-QAM constellations for m up to 8.
    for _ in 0..batch {
        rx_i.push(rng.next_unit() * 2.0);
        rx_q.push(rng.next_unit() * 2.0);
        nv.push(rng.next_positive(0.05, 2.0));
    }
    (rx_i, rx_q, nv)
}

fn assert_close(gpu: &[Llr], cpu: &[Llr], tol: f32, ctx: &str) {
    assert_eq!(gpu.len(), cpu.len(), "{ctx}: length mismatch");
    for (i, (g, c)) in gpu.iter().zip(cpu.iter()).enumerate() {
        let d = (g.value() - c.value()).abs();
        assert!(
            d <= tol,
            "{ctx}: mismatch at {i}: gpu={}, cpu={}, |d|={d}",
            g.value(),
            c.value()
        );
    }
}

#[test]
fn test_gpu_gray_qam_matches_cpu_fast_path_max_log_awgn() {
    // BPSK + every Gray-square-QAM preset.
    for &order in &[2usize, 4, 16, 64, 256] {
        let spec = spec_for_order(order);
        let m = spec.bits_per_symbol() as usize;
        let batch = 128usize;
        let (rx_i, rx_q, nv) = gen_batch(order, batch, 0xA5A5_DEAD_BEEF_u64);

        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::MaxLog,
        };

        let gpu = GpuGrayQamSoftDemapper::new(spec.clone(), batch).unwrap();
        let cpu = FastGrayQamDemapper::<f32>::new(spec);

        let mut out_gpu = vec![Llr::new(0.0); batch * m];
        let mut out_cpu = vec![Llr::new(0.0); batch * m];
        gpu.demap_llrs(input, &mut out_gpu);
        cpu.demap_llrs(input, &mut out_cpu);

        assert_close(
            &out_gpu,
            &out_cpu,
            // Max-log is purely min-of-squared-distances plus one
            // subtraction; host and device do the same f32 arithmetic.
            // A tolerance of 1e-3 absorbs the one-place differences
            // that come out of the f64-vs-f32 distance path on the CPU
            // fast side (CPU does distance math in f64, GPU in f32).
            1e-3,
            &format!("order={order}"),
        );
    }
}

#[test]
fn test_gpu_gray_qam_matches_cpu_fast_path_with_fading_gains() {
    // Complex-gain pre-rotation contract: compare with non-trivial h.
    let order = 16usize;
    let spec = spec_for_order(order);
    let m = spec.bits_per_symbol() as usize;
    let batch = 64usize;
    let (rx_i, rx_q, nv) = gen_batch(order, batch, 0xC0FFEE_u64);
    let mut rng = Lcg::new(0x12345);
    let mut gi = Vec::with_capacity(batch);
    let mut gq = Vec::with_capacity(batch);
    for _ in 0..batch {
        gi.push(rng.next_unit());
        gq.push(rng.next_unit());
    }

    let input = DemapInput::<f32> {
        rx_i: &rx_i,
        rx_q: &rx_q,
        gain_i: Some(&gi),
        gain_q: Some(&gq),
        noise_var: &nv,
        method: DemapMethod::MaxLog,
    };

    let gpu = GpuGrayQamSoftDemapper::new(spec.clone(), batch).unwrap();
    let cpu = FastGrayQamDemapper::<f32>::new(spec);
    let mut out_gpu = vec![Llr::new(0.0); batch * m];
    let mut out_cpu = vec![Llr::new(0.0); batch * m];
    gpu.demap_llrs(input, &mut out_gpu);
    cpu.demap_llrs(input, &mut out_cpu);

    // With random |h| the squared distance grows quadratically in |h|,
    // so absolute LLR magnitudes scale and a looser absolute tolerance
    // is appropriate. 5e-3 comfortably absorbs f32 rounding across the
    // range of |h|^2 seen here.
    assert_close(&out_gpu, &out_cpu, 5e-3, "fading 16-QAM");
}

#[test]
fn test_gpu_gray_qam_empty_batch() {
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let gpu = GpuGrayQamSoftDemapper::new(spec, 16).unwrap();
    let empty: [f32; 0] = [];
    let input = DemapInput::<f32> {
        rx_i: &empty,
        rx_q: &empty,
        gain_i: None,
        gain_q: None,
        noise_var: &empty,
        method: DemapMethod::MaxLog,
    };
    let mut out: Vec<Llr> = Vec::new();
    gpu.demap_llrs(input, &mut out);
    assert!(out.is_empty());
}

/// Throughput probe. Not a correctness test — it just records wall-clock
/// time for GPU and CPU runs of the same 16-QAM batch and prints a ratio
/// so the JIT-9c37ec8c crossover measurement has a ready-made driver.
#[test]
fn bench_gpu_vs_cpu_gray_qam_16qam() {
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let batch = 16_384usize;
    let (rx_i, rx_q, nv) = gen_batch(16, batch, 0xBEEF_u64);

    let input = DemapInput::<f32> {
        rx_i: &rx_i,
        rx_q: &rx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &nv,
        method: DemapMethod::MaxLog,
    };

    let m = 4usize;
    let gpu = GpuGrayQamSoftDemapper::new(spec.clone(), batch).unwrap();
    let cpu = FastGrayQamDemapper::<f32>::new(spec);

    let mut out_gpu = vec![Llr::new(0.0); batch * m];
    let mut out_cpu = vec![Llr::new(0.0); batch * m];

    // Warm up the GPU (first launch pays JIT / driver cost).
    gpu.demap_llrs(input, &mut out_gpu);

    let iters = 20;
    let t_gpu = Instant::now();
    for _ in 0..iters {
        gpu.demap_llrs(input, &mut out_gpu);
    }
    let gpu_elapsed = t_gpu.elapsed();

    let t_cpu = Instant::now();
    for _ in 0..iters {
        cpu.demap_llrs(input, &mut out_cpu);
    }
    let cpu_elapsed = t_cpu.elapsed();

    let gpu_per = gpu_elapsed / iters;
    let cpu_per = cpu_elapsed / iters;
    eprintln!(
        "bench_gpu_vs_cpu_gray_qam_16qam: batch={batch}, iters={iters}\n\
         GPU: {:?} per batch\n\
         CPU: {:?} per batch\n\
         ratio (CPU / GPU): {:.3}x",
        gpu_per,
        cpu_per,
        cpu_elapsed.as_secs_f64() / gpu_elapsed.as_secs_f64(),
    );
}
