//! CPU-vs-GPU byte-identity regression for the DVB-T2 chain
//! (issue `0d9cb8e3`, Phase D close; design doc §11).
//!
//! For each of the six in-scope DVB-T2 MODCODs
//! (`rate ∈ {1/2, 2/3, 3/4}` × `modulation ∈ {16-QAM, 64-QAM}`) this suite
//! asserts the §11 determinism contract over the production
//! [`Pipeline::run`](gf2_sim::Pipeline::run) path:
//!
//! * **Mode A** (`parallelism(1)`, CPU-only) vs
//!   **Mode B** (`parallelism(24)`, CPU-parallel) — **four-column** byte-identity
//!   (`fer`, `frames`, `errors`, `mean_iters`; the §11 CPU-only/parallel contract).
//! * **Mode A** (CPU-only) vs
//!   **Mode C** (`with_gpu(true)`, CPU+GPU on gfx1030) — **three-column**
//!   byte-identity (`fer`, `frames`, `errors`; `mean_iters` is LOGGED but NOT
//!   asserted per the §11 CPU-vs-GPU relaxed contract, user-approved Q3
//!   2026-06-07). Mode C is `#[cfg(feature = "hip")]`-gated.
//!
//! # Column conventions
//!
//! The four/three columns come from [`SnrPointResult`] which is derived directly
//! from [`WorkerCounters`] — the §11 byte-identity SSOT:
//!
//! * `frames` — `u64`, integer-exact.
//! * `errors` — `u64`, integer-exact — **frame**-error count (not bit errors).
//! * `fer` — `errors / frames`, `f64`; compared via [`f64::to_bits`].
//! * `mean_iters` — `total_iterations / frames`, `f64`; compared via
//!   [`f64::to_bits`] for Mode A vs B only; LOGGED for Mode C.
//!
//! `ber` / `total_bit_errors` are **excluded entirely** (non-associative f32
//! horizontal reduction; `152388f4`; §11 "Always-excluded"). Do not assert them.
//!
//! # Waterfall operating points (non-vacuous sweep regime)
//!
//! Each MODCOD runs at a per-MODCOD **waterfall** Es/N0 calibrated at seed
//! `0xDE16_0FC5` with SumProduct + ExactLogMap. The waterfall is the steep part
//! of the FER curve, where `0 < errors < frames` — the regime §11 names verbatim
//! ("near the convergence threshold ... the frame's final verdict ... is robust to
//! that drift"). Every slow leg asserts the sweep is non-vacuous. The smoke legs
//! use a fast-converging Es/N0 (well above waterfall) to stay under the 5 s
//! fast-tier cap.
//!
//! # Frame count — AMENDMENT 2026-06-12
//!
//! The slow legs run **50 frames per MODCOD** per the user-approved AMENDMENT
//! 2026-06-12 on this issue (`0d9cb8e3`). Mode A at 50 frames is ~42 s at the
//! waterfall (~1.18 fps); Mode B at 24 workers is ~5 s; Mode C adds seconds on the
//! GPU. Total per MODCOD leg ≈ 50-60 s, well under the 120 s slow-tier cap. The
//! one-time 200-frame off-test completion evidence (per-MODCOD column values) is
//! recorded in `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md`.
//!
//! # Tiers
//!
//! * **Fast smoke #1 (CPU)**: `test_dvb_t2_regression_smoke_cpu_r12_16qam` — 2
//!   frames at a fast-converging Es/N0 (9.0 dB, well above the 6.0 dB waterfall),
//!   Mode A vs B, four-column assert. Runs un-ignored on every `--profile ci` gate.
//! * **Fast smoke #2 (GPU)**: `test_dvb_t2_regression_smoke_gpu_r12_16qam` —
//!   `#[cfg(feature = "hip")]`, 2 frames, Mode A vs C, three-column assert;
//!   runtime-skip when no GPU present.
//! * **Slow** (one `#[ignore = "sim: ..."]` per MODCOD): 50 frames at the
//!   calibrated waterfall point; Mode A run once, compared against both B and C.
//!   Mode C comparison is `#[cfg(feature = "hip")]`-gated within each slow test.

mod common;

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::executor::SnrPointResult;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::Pipeline;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Base seed for all regression tests (the `de160fc5` waterfall seed; the
/// per-MODCOD waterfall Es/N0 points below are calibrated at THIS seed with
/// SumProduct + ExactLogMap).
const SEED: u64 = 0xDE16_0FC5;

/// Parallelism for Mode B (CPU-parallel). The §11 CPU-parallel contract holds
/// for any worker count; 24 is the project's reference count.
const MODE_B_PARALLELISM: usize = 24;

/// Frame count for the slow-tier legs (AMENDMENT 2026-06-12: 50 frames; the
/// one-time 200-frame completion evidence is off-test in receipts).
const SLOW_FRAMES: u64 = 50;

/// Frame count for the fast-tier smoke (2 frames at a fast-converging Es/N0
/// well above the waterfall — keeps wall time under the 5 s cap).
const SMOKE_FRAMES: u64 = 2;

/// Es/N0 for the smoke tests: well above the r1/2 16-QAM waterfall (6.0 dB);
/// at 9.0 dB frames decode fast with few BP iterations.
const SMOKE_ES_N0: f64 = 9.0;

/// Frame count override via environment variable for off-test 200-frame
/// attestation runs. Default (absent env var) is [`SLOW_FRAMES`].
///
/// Usage: `GF2_SIM_REGRESSION_FRAMES=200 cargo test -p gf2-sim --all-features \
///   --release --test dvb_t2_regression -- test_dvb_t2_regression_50f_r12_16qam \
///   --ignored --nocapture`
///
/// Via `cargo test`, NOT nextest: at 200 frames the legs run 189-332 s,
/// beyond the 120 s slow-tier cap nextest enforces (see the receipts file).
fn slow_frames() -> u64 {
    std::env::var("GF2_SIM_REGRESSION_FRAMES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(SLOW_FRAMES)
}

/// One MODCOD configuration for the regression suite.
struct ModcodPoint {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    /// Waterfall Es/N0 in dB for slow-tier legs. Calibrated at [`SEED`] with
    /// SumProduct + ExactLogMap, 50-frame empirical mixes listed on each test fn.
    waterfall_es_n0_db: f64,
    label: &'static str,
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

/// Builds a Mode A (CPU-only, parallelism=1) pipeline.
fn build_mode_a(
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    frames: u64,
) -> Pipeline {
    let mut p = Pipeline::dvb_t2()
        .modcod(Modcod::Normal { rate, modulation })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0_db as f32))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(1).expect("1 is non-zero"))
        .build()
        .expect("in-scope MODCOD builds via preset");
    p.config_mut().esn0_db_points = vec![es_n0_db];
    p.config_mut().max_frames = frames;
    p
}

/// Builds a Mode B (CPU-parallel, parallelism=24) pipeline.
fn build_mode_b(
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    frames: u64,
) -> Pipeline {
    let mut p = Pipeline::dvb_t2()
        .modcod(Modcod::Normal { rate, modulation })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0_db as f32))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(MODE_B_PARALLELISM).expect("24 is non-zero"))
        .build()
        .expect("in-scope MODCOD builds via preset");
    p.config_mut().esn0_db_points = vec![es_n0_db];
    p.config_mut().max_frames = frames;
    p
}

/// Builds a Mode C (CPU+GPU, parallelism=24, with_gpu=true) pipeline.
#[cfg(feature = "hip")]
fn build_mode_c(
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    frames: u64,
) -> Pipeline {
    let mut p = Pipeline::dvb_t2()
        .modcod(Modcod::Normal { rate, modulation })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0_db as f32))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(MODE_B_PARALLELISM).expect("24 is non-zero"))
        .with_gpu(true)
        .build()
        .expect("in-scope MODCOD builds via preset");
    p.config_mut().esn0_db_points = vec![es_n0_db];
    p.config_mut().max_frames = frames;
    p
}

/// Asserts four-column byte-identity between two [`SnrPointResult`]s (design
/// doc §11 CPU-only/parallel contract) by adapting both points back to the
/// SSOT [`WorkerCounters`](gf2_sim::parallel::WorkerCounters) via the shared
/// `tests/common` adapter and delegating to
/// `common::assert_four_columns_byte_identical` — the single source of truth
/// for the four-column set and the BER exclusion. No column logic lives here.
#[track_caller]
fn assert_four_columns(a: &SnrPointResult, b: &SnrPointResult, label: &str) {
    common::assert_four_columns_byte_identical(
        &common::snr_point_to_counters(b),
        &common::snr_point_to_counters(a),
        label,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Core leg runner (slow tier)
// ─────────────────────────────────────────────────────────────────────────────

/// Runs the three-mode regression for one MODCOD at its waterfall Es/N0:
///
/// 1. Mode A (CPU, parallelism=1) — run once.
/// 2. Mode B (CPU, parallelism=24) — four-column assert vs A.
/// 3. Mode C (CPU+GPU) — three-column assert vs A; `mean_iters` logged.
///    Mode C is `#[cfg(feature = "hip")]`-gated and skips at runtime if no GPU.
///
/// Asserts the sweep is non-vacuous: `0 < errors < frames` for the A arm.
fn run_regression(point: &ModcodPoint, frames: u64) {
    let ModcodPoint {
        rate,
        modulation,
        waterfall_es_n0_db,
        label,
    } = *point;

    // Mode A: CPU-only, parallelism=1. Run once; compared against B and C.
    let a_result = build_mode_a(rate, modulation, waterfall_es_n0_db, frames)
        .run()
        .expect("Mode A CPU-only run");
    assert_eq!(
        a_result.per_point.len(),
        1,
        "{label}: Mode A expected 1 SNR point"
    );
    let a = a_result.per_point[0];

    // Non-vacuity of the A arm (§11 regime): `0 < errors < frames`. Without
    // this the `errors`/`fer` columns are informationless (0 == 0) and the
    // three-column CPU-vs-GPU comparison has nothing to exercise.
    assert_eq!(
        a.frames, frames,
        "{label}: Mode A ran {}/{frames} frames",
        a.frames
    );
    assert!(
        a.errors > 0 && a.errors < a.frames,
        "{label}: Mode A sweep is VACUOUS (errors={}/{frames}); \
         re-pin Es/N0 if the chain changes",
        a.errors
    );

    eprintln!(
        "{label} A: frames={} errors={} fer={:.6} mean_iters={:.6}",
        a.frames, a.errors, a.fer, a.mean_iters,
    );

    // Mode B: CPU-parallel, parallelism=24. Four-column assert vs A.
    let b_result = build_mode_b(rate, modulation, waterfall_es_n0_db, frames)
        .run()
        .expect("Mode B CPU-parallel run");
    assert_eq!(
        b_result.per_point.len(),
        1,
        "{label}: Mode B expected 1 SNR point"
    );
    let b = b_result.per_point[0];

    eprintln!(
        "{label} B: frames={} errors={} fer={:.6} mean_iters={:.6}",
        b.frames, b.errors, b.fer, b.mean_iters,
    );

    assert_four_columns(&a, &b, &format!("(A-vs-B) {label}"));

    eprintln!(
        "{label}: Mode A == Mode B (four columns: frames/errors/fer/mean_iters byte-identical)"
    );

    // Mode C: CPU+GPU. Three-column assert vs A; mean_iters logged only.
    // Gated on feature = "hip" and runtime GPU presence.
    #[cfg(feature = "hip")]
    {
        if gf2_kernels_hip::host::device_mem_info().is_err() {
            eprintln!(
                "{label}: skipping Mode C (CPU+GPU) — no usable GPU (device_mem_info failed)"
            );
        } else {
            let c_result = build_mode_c(rate, modulation, waterfall_es_n0_db, frames)
                .run()
                .expect("Mode C CPU+GPU run");
            assert_eq!(
                c_result.per_point.len(),
                1,
                "{label}: Mode C expected 1 SNR point"
            );
            let c = c_result.per_point[0];

            eprintln!(
                "{label} C: frames={} errors={} fer={:.6} mean_iters={:.6}",
                c.frames, c.errors, c.fer, c.mean_iters,
            );

            common::assert_three_columns_byte_identical_log_mean_iters(
                &common::snr_point_to_counters(&c),
                &common::snr_point_to_counters(&a),
                &format!("(A-vs-C) {label}"),
            );

            eprintln!(
                "{label}: Mode A == Mode C (three columns: frames/errors/fer byte-identical; \
                 mean_iters logged)"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fast-tier smoke #1 (CPU): un-ignored, always runs on `--profile ci`
// ─────────────────────────────────────────────────────────────────────────────

/// Fast-tier smoke: 2 frames of r1/2 16-QAM at 9.0 dB (well above the 6.0 dB
/// waterfall; frames converge quickly with low BP iterations). Asserts four-column
/// byte-identity between Mode A (parallelism=1) and Mode B (parallelism=24).
///
/// Does NOT assert non-vacuity — at 9.0 dB above threshold, errors may be 0/2
/// and that is valid. Non-vacuity is the slow legs' job.
///
/// Un-ignored so the `[hard]` four-column A-vs-B criterion runs on every green
/// `cargo-ci` gate within the unmodified 5 s cap.
#[test]
fn test_dvb_t2_regression_smoke_cpu_r12_16qam() {
    let a_result = build_mode_a(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        SMOKE_ES_N0,
        SMOKE_FRAMES,
    )
    .run()
    .expect("smoke Mode A");
    let b_result = build_mode_b(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        SMOKE_ES_N0,
        SMOKE_FRAMES,
    )
    .run()
    .expect("smoke Mode B");

    assert_eq!(a_result.per_point.len(), 1);
    assert_eq!(b_result.per_point.len(), 1);
    let a = a_result.per_point[0];
    let b = b_result.per_point[0];

    eprintln!(
        "smoke A: frames={} errors={} fer={:.6} mean_iters={:.6}",
        a.frames, a.errors, a.fer, a.mean_iters
    );
    eprintln!(
        "smoke B: frames={} errors={} fer={:.6} mean_iters={:.6}",
        b.frames, b.errors, b.fer, b.mean_iters
    );

    assert_four_columns(&a, &b, "smoke CPU r1/2 16-QAM @9.0dB");
}

// ─────────────────────────────────────────────────────────────────────────────
// Fast-tier smoke #2 (GPU): un-ignored, #[cfg(feature = "hip")]
// ─────────────────────────────────────────────────────────────────────────────

/// Fast-tier GPU smoke: 2 frames of r1/2 16-QAM at 9.0 dB. Asserts three-column
/// byte-identity between Mode A (parallelism=1, CPU) and Mode C (with_gpu=true).
/// Runtime-skip when no GPU is present. Compiles only under `feature = "hip"`.
///
/// Un-ignored so the `[hard]` three-column A-vs-C criterion is exercised on the
/// gfx1030 CI job on every gate run, within the 5 s cap.
#[cfg(feature = "hip")]
#[test]
fn test_dvb_t2_regression_smoke_gpu_r12_16qam() {
    if gf2_kernels_hip::host::device_mem_info().is_err() {
        eprintln!("skipping GPU smoke: no usable GPU (device_mem_info failed)");
        return;
    }

    let a_result = build_mode_a(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        SMOKE_ES_N0,
        SMOKE_FRAMES,
    )
    .run()
    .expect("smoke Mode A");
    let c_result = build_mode_c(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        SMOKE_ES_N0,
        SMOKE_FRAMES,
    )
    .run()
    .expect("smoke Mode C");

    assert_eq!(a_result.per_point.len(), 1);
    assert_eq!(c_result.per_point.len(), 1);
    let a = a_result.per_point[0];
    let c = c_result.per_point[0];

    eprintln!(
        "smoke A: frames={} errors={} fer={:.6} mean_iters={:.6}",
        a.frames, a.errors, a.fer, a.mean_iters
    );
    eprintln!(
        "smoke C: frames={} errors={} fer={:.6} mean_iters={:.6}",
        c.frames, c.errors, c.fer, c.mean_iters
    );

    common::assert_three_columns_byte_identical_log_mean_iters(
        &common::snr_point_to_counters(&c),
        &common::snr_point_to_counters(&a),
        "smoke GPU r1/2 16-QAM @9.0dB",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Slow-tier legs: one per MODCOD, 50 frames at the calibrated waterfall
// (AMENDMENT 2026-06-12). Non-vacuity of the A arm asserted in each.
//
// Calibration evidence (seed 0xDE16_0FC5, SumProduct + ExactLogMap, 50 frames
// at the listed waterfall Es/N0; errors counted from Mode A):
//   r1/2 16-QAM @6.0dB  → 12/50 errored (per D.1 precedent)
//   r1/2 64-QAM @10.3dB → 21/50 errored (per D.1 precedent)
//   r2/3 16-QAM @8.8dB  → 27/50 errored (per D.1 precedent)
//   r2/3 64-QAM @13.8dB → 32/50 errored (per D.1 precedent)
//   r3/4 16-QAM @10.0dB → 31/50 errored (per D.1 precedent)
//   r3/4 64-QAM @15.4dB → 17/50 errored (per D.1 precedent)
// ─────────────────────────────────────────────────────────────────────────────

/// Slow: 50 frames, r1/2 16-QAM at the 6.0 dB waterfall.
/// Mode A vs B: four-column assert. Mode A vs C: three-column assert (GPU
/// arm compiled under `#[cfg(feature = "hip")]`, runtime-skip if no GPU).
/// Non-vacuous: ~12/50 errored frames expected.
#[test]
#[ignore = "sim: 50-frame DVB-T2 regression, r1/2 16-QAM waterfall (0d9cb8e3)"]
fn test_dvb_t2_regression_50f_r12_16qam() {
    run_regression(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            waterfall_es_n0_db: 6.0,
            label: "r1/2 16-QAM @6.0dB",
        },
        slow_frames(),
    );
}

/// Slow: 50 frames, r1/2 64-QAM at the 10.3 dB waterfall.
/// Non-vacuous: ~21/50 errored frames expected.
#[test]
#[ignore = "sim: 50-frame DVB-T2 regression, r1/2 64-QAM waterfall (0d9cb8e3)"]
fn test_dvb_t2_regression_50f_r12_64qam() {
    run_regression(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam64,
            waterfall_es_n0_db: 10.3,
            label: "r1/2 64-QAM @10.3dB",
        },
        slow_frames(),
    );
}

/// Slow: 50 frames, r2/3 16-QAM at the 8.8 dB waterfall.
/// Non-vacuous: ~27/50 errored frames expected.
#[test]
#[ignore = "sim: 50-frame DVB-T2 regression, r2/3 16-QAM waterfall (0d9cb8e3)"]
fn test_dvb_t2_regression_50f_r23_16qam() {
    run_regression(
        &ModcodPoint {
            rate: CodeRate::Rate2_3,
            modulation: DvbT2Modulation::Qam16,
            waterfall_es_n0_db: 8.8,
            label: "r2/3 16-QAM @8.8dB",
        },
        slow_frames(),
    );
}

/// Slow: 50 frames, r2/3 64-QAM at the 13.8 dB waterfall.
/// Non-vacuous: ~32/50 errored frames expected.
#[test]
#[ignore = "sim: 50-frame DVB-T2 regression, r2/3 64-QAM waterfall (0d9cb8e3)"]
fn test_dvb_t2_regression_50f_r23_64qam() {
    run_regression(
        &ModcodPoint {
            rate: CodeRate::Rate2_3,
            modulation: DvbT2Modulation::Qam64,
            waterfall_es_n0_db: 13.8,
            label: "r2/3 64-QAM @13.8dB",
        },
        slow_frames(),
    );
}

/// Slow: 50 frames, r3/4 16-QAM at the 10.0 dB waterfall.
/// Non-vacuous: ~31/50 errored frames expected.
#[test]
#[ignore = "sim: 50-frame DVB-T2 regression, r3/4 16-QAM waterfall (0d9cb8e3)"]
fn test_dvb_t2_regression_50f_r34_16qam() {
    run_regression(
        &ModcodPoint {
            rate: CodeRate::Rate3_4,
            modulation: DvbT2Modulation::Qam16,
            waterfall_es_n0_db: 10.0,
            label: "r3/4 16-QAM @10.0dB",
        },
        slow_frames(),
    );
}

/// Slow: 50 frames, r3/4 64-QAM at the 15.4 dB waterfall.
/// Non-vacuous: ~17/50 errored frames expected.
#[test]
#[ignore = "sim: 50-frame DVB-T2 regression, r3/4 64-QAM waterfall (0d9cb8e3)"]
fn test_dvb_t2_regression_50f_r34_64qam() {
    run_regression(
        &ModcodPoint {
            rate: CodeRate::Rate3_4,
            modulation: DvbT2Modulation::Qam64,
            waterfall_es_n0_db: 15.4,
            label: "r3/4 64-QAM @15.4dB",
        },
        slow_frames(),
    );
}
