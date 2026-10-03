//! Cross-validation: GPU Gray-QAM max-log demapper vs the CPU
//! `FastGrayQamDemapper` with `DemapMethod::MaxLog`.

use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GpuGrayQamSoftDemapper,
    ModemSpec,
};
use gf2_core::rng::Lcg;

#[path = "gpu_bench_support.rs"]
mod gpu_bench_support;
use gpu_bench_support::{gen_batch, spec_for_order};

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
            // The CPU fast path computes distances in f64 and the GPU in f32.
            1e-3,
            &format!("order={order}"),
        );
    }
}

#[test]
fn test_gpu_gray_qam_matches_cpu_fast_path_with_fading_gains() {
    let order = 16usize;
    let spec = spec_for_order(order);
    let m = spec.bits_per_symbol() as usize;
    let batch = 64usize;
    let (rx_i, rx_q, nv) = gen_batch(order, batch, 0xC0FFEE_u64);
    let mut rng = Lcg::new(0x12345);
    let mut gi = Vec::with_capacity(batch);
    let mut gq = Vec::with_capacity(batch);
    for _ in 0..batch {
        gi.push(rng.next_unit_f32());
        gq.push(rng.next_unit_f32());
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

/// The adapter narrows its advertised capabilities, so the shared
/// `validate_demap_input` pre-flight rejects `ExactLogMap`.
#[test]
#[should_panic(expected = "spec does not advertise ExactLogMap support")]
fn test_gpu_gray_qam_rejects_exact_log_map() {
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let batch = 8usize;
    let (rx_i, rx_q, nv) = gen_batch(16, batch, 0xF00D_u64);
    let input = DemapInput::<f32> {
        rx_i: &rx_i,
        rx_q: &rx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &nv,
        method: DemapMethod::ExactLogMap,
    };
    let gpu = GpuGrayQamSoftDemapper::new(spec, batch).unwrap();
    let mut out = vec![Llr::new(0.0); batch * 4];
    gpu.demap_llrs(input, &mut out);
}

#[test]
fn test_gpu_gray_qam_spec_capabilities_advertise_max_log_only() {
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let gpu = GpuGrayQamSoftDemapper::new(spec, 16).unwrap();
    let caps = gpu.spec().capabilities();
    assert!(
        !caps.supports_exact_log_map,
        "GPU adapter must not advertise ExactLogMap"
    );
    assert!(caps.supports_max_log, "GPU adapter must advertise MaxLog");
}
