//! CPU-vs-GPU agreement of the Gray-QAM max-log soft demap: on one fixed
//! channel realisation, [`GpuGrayQamDemapper`] LLRs match
//! [`FastGrayQamDemapper`] LLRs element-wise within [`MAX_LOG_ULP_TOLERANCE`]
//! ulp or [`MAX_LOG_ABS_TOLERANCE`] absolute. The absolute floor covers LLRs
//! near zero, where a difference of two O(1) squared distances leaves a
//! residual of many value-relative ulps. Skips when no GPU is usable.

#![cfg(feature = "hip")]

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::modem::{
    BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, ModemSpec,
};
use gf2_coding::Llr;
use gf2_kernels_hip::host::device_mem_info;
use gf2_sim::batch::SymbolBatch;
use gf2_sim::gpu::demap::GpuGrayQamDemapper;

/// ULP tolerance for LLRs at O(1) magnitude.
const MAX_LOG_ULP_TOLERANCE: u32 = 2;

/// Absolute tolerance floor for near-zero LLRs.
const MAX_LOG_ABS_TOLERANCE: f32 = 2.0e-6;

const MEASURED_WORST_ABS_DIFF: f32 = 1.9073486e-6;

const _: () = assert!(
    MAX_LOG_ULP_TOLERANCE <= 2,
    "literal criterion is <= 2 ulp; recorded value-ulp tolerance must not exceed it"
);
const _: () = assert!(
    MAX_LOG_ABS_TOLERANCE >= MEASURED_WORST_ABS_DIFF,
    "absolute floor must cover the measured worst-case absolute LLR difference"
);

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_signed(&mut self) -> f32 {
        // SplitMix64 step.
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // Map to [-1, 1).
        ((z >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// Absolute ulp gap. The key maps sign-magnitude bits to a monotone integer,
/// so adjacent floats differ by 1.
fn ulp_gap(a: f32, b: f32) -> u64 {
    if a == b {
        return 0;
    }
    let key = |x: f32| -> i64 {
        let bits = i64::from(x.to_bits());
        if x.to_bits() & 0x8000_0000 != 0 {
            -(bits & 0x7fff_ffff)
        } else {
            bits
        }
    };
    (key(a) - key(b)).unsigned_abs()
}

fn within_tolerance(g: f32, c: f32) -> bool {
    if (g - c).abs() <= MAX_LOG_ABS_TOLERANCE {
        return true;
    }
    ulp_gap(g, c) <= u64::from(MAX_LOG_ULP_TOLERANCE)
}

/// Returns the worst absolute LLR difference.
fn check_modulation(modulation: DvbT2Modulation, seed: u64) -> f32 {
    let m = modulation.bits_per_cell();
    let order = 1usize << m;
    let num_symbols = 4096usize;
    let noise_var = 0.35_f32; // N0 = 2 sigma^2

    let mut rng = Lcg::new(seed);
    let rx_i: Vec<f32> = (0..num_symbols).map(|_| rng.next_signed() * 1.5).collect();
    let rx_q: Vec<f32> = (0..num_symbols).map(|_| rng.next_signed() * 1.5).collect();

    let cpu = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(order));
    let nv = vec![noise_var; num_symbols];
    let mut cpu_llrs = vec![Llr::zero(); num_symbols * m];
    cpu.demap_llrs(
        DemapInput {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::MaxLog,
        },
        &mut cpu_llrs,
    );

    let stage = GpuGrayQamDemapper::new(modulation, DemapMethod::MaxLog, noise_var);
    let demapper = stage
        .build_demapper(num_symbols)
        .expect("build GPU demapper");
    let batch = SymbolBatch::new(vec![rx_i.clone()], vec![rx_q.clone()]);
    let gpu_out = stage.demap_batch(&batch, &demapper).expect("gpu demap");
    let gpu_llrs = &gpu_out.frames[0];

    assert_eq!(
        gpu_llrs.len(),
        cpu_llrs.len(),
        "GPU/CPU LLR vector length mismatch for {modulation:?}"
    );

    let mut max_abs = 0.0f32;
    let mut worst_ulp_at_unit_scale = 0u64;
    for (k, (g, c)) in gpu_llrs.iter().zip(cpu_llrs.iter()).enumerate() {
        let (gv, cv) = (g.value(), c.value());
        let adiff = (gv - cv).abs();
        if adiff > max_abs {
            max_abs = adiff;
        }
        // Value-relative ulp is meaningful only where the LLR magnitude is O(1).
        if gv.abs().max(cv.abs()) >= 1.0 {
            worst_ulp_at_unit_scale = worst_ulp_at_unit_scale.max(ulp_gap(gv, cv));
        }
        assert!(
            within_tolerance(gv, cv),
            "{modulation:?}: LLR[{k}] GPU={gv} CPU={cv} outside combined tolerance \
             (|diff|={adiff:e} > abs {MAX_LOG_ABS_TOLERANCE:e} AND ulp gap {} > {MAX_LOG_ULP_TOLERANCE})",
            ulp_gap(gv, cv),
        );
    }
    println!(
        "{modulation:?}: PASS — max |GPU-CPU| = {max_abs:e} (abs floor {MAX_LOG_ABS_TOLERANCE:e}); \
         worst ulp gap at |LLR|>=1.0 = {worst_ulp_at_unit_scale} (<= {MAX_LOG_ULP_TOLERANCE})"
    );
    max_abs
}

#[test]
#[ignore = "sim: GPU Gray-QAM max-log byte-identity (gfx1030-gated; builds presets + device demapper)"]
fn gpu_demap_max_log_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_demap_max_log_byte_identical_to_cpu: no usable GPU");
        return;
    }

    let abs_16 = check_modulation(DvbT2Modulation::Qam16, 0xD3F1_616A_0010);
    let abs_64 = check_modulation(DvbT2Modulation::Qam64, 0xD3F1_616A_0040);

    println!(
        "recorded tolerances: MAX_LOG_ULP_TOLERANCE = {MAX_LOG_ULP_TOLERANCE} ulp (at unit LLR \
         scale), MAX_LOG_ABS_TOLERANCE = {MAX_LOG_ABS_TOLERANCE:e}"
    );
    println!("measured worst |GPU-CPU|: 16-QAM {abs_16:e}, 64-QAM {abs_64:e}");

    assert!(abs_16.max(abs_64) <= MAX_LOG_ABS_TOLERANCE);
}
