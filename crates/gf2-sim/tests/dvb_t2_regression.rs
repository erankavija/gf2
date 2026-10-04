//! Byte-identity regression of the DVB-T2 chain over [`Pipeline::run`] for the
//! six MODCODs `{1/2, 2/3, 3/4}` × `{16-QAM, 64-QAM}`: a 1-worker CPU run (Mode
//! A) against a 24-worker CPU run (Mode B) on `frames`, `errors`, `fer` and
//! `mean_iters`, and against a CPU+GPU run (Mode C, `hip` feature) on `frames`,
//! `errors` and `fer`. The ignored legs run at a waterfall Es/N0 and assert
//! `0 < errors < frames`.

mod common;

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::executor::SnrPointResult;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::Pipeline;

/// The waterfall Es/N0 points are calibrated at this seed.
const SEED: u64 = 0xDE16_0FC5;

const MODE_B_PARALLELISM: usize = 24;

const SLOW_FRAMES: u64 = 50;

const SMOKE_FRAMES: u64 = 2;

/// Above the r1/2 16-QAM waterfall point (6.0 dB).
const SMOKE_ES_N0: f64 = 9.0;

fn slow_frames() -> u64 {
    std::env::var("GF2_SIM_REGRESSION_FRAMES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(SLOW_FRAMES)
}

struct ModcodPoint {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    /// Calibrated at [`SEED`] with SumProduct + ExactLogMap.
    waterfall_es_n0_db: f64,
    label: &'static str,
}

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

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

#[track_caller]
fn assert_four_columns(a: &SnrPointResult, b: &SnrPointResult, label: &str) {
    common::assert_four_columns_byte_identical(
        &common::snr_point_to_counters(b),
        &common::snr_point_to_counters(a),
        label,
    );
}

fn run_regression(point: &ModcodPoint, frames: u64) {
    let ModcodPoint {
        rate,
        modulation,
        waterfall_es_n0_db,
        label,
    } = *point;

    let a_result = build_mode_a(rate, modulation, waterfall_es_n0_db, frames)
        .run()
        .expect("Mode A CPU-only run");
    assert_eq!(
        a_result.per_point.len(),
        1,
        "{label}: Mode A expected 1 SNR point"
    );
    let a = a_result.per_point[0];

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
