//! Verifies that every Phase B GPU stage has its `Stage::CpuFallback` properly
//! declared and `cpu_fallback()` returns `Some(&self.fallback)` (issue
//! `ed575f15`, deliverable 2; design doc §8).
//!
//! All three GPU stages are constructible without a real HIP device — they build
//! their CPU fallback eagerly at construction time and defer device-specific
//! resources to the throughput path. This test file therefore requires
//! `feature = "hip"` for the type definitions but DOES NOT REQUIRE a real GPU:
//! construction and fallback access are pure host operations.
//!
//! Stages verified:
//! - [`GpuAwgn`] — `CpuFallback = channels::Awgn` (in-crate)
//! - [`GpuLdpcBp`] — `CpuFallback = CpuLdpcBp` (orphan-rule wrapper)
//! - [`GpuGrayQamDemapper`] — `CpuFallback = CpuGrayQamDemapper` (orphan-rule
//!   wrapper); also covers the `ExactLogMap` → `CpuOnly` execution-class path.

#![cfg(feature = "hip")]

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderConfig, LdpcCode};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::gpu::awgn::GpuAwgn;
use gf2_sim::gpu::demap::GpuGrayQamDemapper;
use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
use gf2_sim::stage::{ExecutionClass, Stage};

// ────────────────────────────────────────────────────────────────────────────
// GpuAwgn — CpuFallback = channels::Awgn
// ────────────────────────────────────────────────────────────────────────────

/// `GpuAwgn::cpu_fallback()` must return `Some` carrying the eagerly-built
/// CPU `Awgn` stage with matching parameters (no GPU required).
#[test]
fn test_gpu_awgn_cpu_fallback_is_some() {
    let stage = GpuAwgn::new(6.25, 4);
    let fb = stage
        .cpu_fallback()
        .expect("GpuAwgn must have a CPU fallback");
    assert_eq!(
        fb.es_n0_db(),
        stage.es_n0_db(),
        "fallback Es/N0 must match the GPU stage"
    );
    assert_eq!(
        fb.bits_per_symbol(),
        stage.bits_per_symbol(),
        "fallback bits_per_symbol must match the GPU stage"
    );
    assert_eq!(
        fb.sigma(),
        stage.sigma(),
        "fallback sigma must match the GPU stage (same SSOT formula)"
    );
}

/// `GpuAwgn` is `ExecutionClass::GpuOnly`.
#[test]
fn test_gpu_awgn_execution_class_is_gpu_only() {
    let stage = GpuAwgn::new(6.25, 4);
    assert_eq!(
        stage.execution_class(),
        ExecutionClass::GpuOnly,
        "GpuAwgn must report GpuOnly"
    );
}

/// The fallback stage from a seek-parameterised `GpuAwgn` is still present.
#[test]
fn test_gpu_awgn_fallback_survives_seek_and_device_setters() {
    let stage = GpuAwgn::new(5.0, 6).with_seek(99, 2, 1).on_device(0);
    let fb = stage
        .cpu_fallback()
        .expect("GpuAwgn with seek must still have a CPU fallback");
    assert_eq!(fb.es_n0_db(), 5.0);
    assert_eq!(fb.bits_per_symbol(), 6);
}

// ────────────────────────────────────────────────────────────────────────────
// GpuLdpcBp — CpuFallback = CpuLdpcBp (orphan-rule wrapper around LdpcDecoder)
// ────────────────────────────────────────────────────────────────────────────

/// `GpuLdpcBp::cpu_fallback()` must return `Some` carrying a `CpuLdpcBp`
/// with the same code dimensions and iteration cap. No GPU required.
#[test]
fn test_gpu_ldpc_bp_cpu_fallback_is_some() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let n = code.n();
    let stage = GpuLdpcBp::new(code, DecoderConfig::default(), 50);

    let fb = stage
        .cpu_fallback()
        .expect("GpuLdpcBp must have a CPU fallback");
    assert_eq!(
        fb.n(),
        n,
        "fallback CpuLdpcBp must report the same codeword length"
    );
    assert_eq!(
        fb.max_iterations(),
        50,
        "fallback CpuLdpcBp must report the same iteration cap"
    );
    assert_eq!(
        fb.config(),
        DecoderConfig::default(),
        "fallback CpuLdpcBp must report the same decoder config"
    );
}

/// `GpuLdpcBp` is `ExecutionClass::GpuOnly`.
#[test]
fn test_gpu_ldpc_bp_execution_class_is_gpu_only() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let stage = GpuLdpcBp::new(code, DecoderConfig::default(), 50);
    assert_eq!(
        stage.execution_class(),
        ExecutionClass::GpuOnly,
        "GpuLdpcBp must report GpuOnly"
    );
}

/// The `CpuLdpcBp` wrapper (the registered fallback for `GpuLdpcBp`) must
/// itself return `Some(&self)` from `cpu_fallback` (it is its own fallback —
/// a CPU-only stage per design §8).
#[test]
fn test_cpu_ldpc_bp_is_its_own_fallback() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let stage = GpuLdpcBp::new(code, DecoderConfig::default(), 10);
    let fb = stage
        .cpu_fallback()
        .expect("GpuLdpcBp has a CpuLdpcBp fallback");
    // CpuLdpcBp is its own fallback (cpu_fallback returns Some(&self)).
    assert!(
        fb.cpu_fallback().is_some(),
        "CpuLdpcBp must be its own cpu_fallback (CpuOnly stage)"
    );
    assert_eq!(
        fb.execution_class(),
        ExecutionClass::CpuOnly,
        "CpuLdpcBp must report CpuOnly"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// GpuGrayQamDemapper (MaxLog) — CpuFallback = CpuGrayQamDemapper
// ────────────────────────────────────────────────────────────────────────────

/// `GpuGrayQamDemapper` in MaxLog mode: `cpu_fallback()` must return `Some`
/// carrying a `CpuGrayQamDemapper` with matching parameters. No GPU required.
#[test]
fn test_gpu_gray_qam_demapper_max_log_cpu_fallback_is_some() {
    let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25);

    let fb = stage
        .cpu_fallback()
        .expect("GpuGrayQamDemapper (MaxLog) must have a CPU fallback");
    assert_eq!(
        fb.bits_per_symbol(),
        4,
        "fallback CpuGrayQamDemapper must report m=4 for 16-QAM"
    );
    assert_eq!(
        fb.method(),
        DemapMethod::MaxLog,
        "fallback must carry the MaxLog method"
    );
    assert_eq!(
        fb.noise_var(),
        0.25,
        "fallback noise_var must match the GPU stage"
    );
}

/// `GpuGrayQamDemapper` in MaxLog mode is `ExecutionClass::GpuOnly`.
#[test]
fn test_gpu_gray_qam_demapper_max_log_execution_class_is_gpu_only() {
    let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam64, DemapMethod::MaxLog, 0.5);
    assert_eq!(
        stage.execution_class(),
        ExecutionClass::GpuOnly,
        "MaxLog GpuGrayQamDemapper must report GpuOnly"
    );
}

/// The `CpuGrayQamDemapper` wrapper must be its own fallback (CPU-only stage).
#[test]
fn test_cpu_gray_qam_demapper_is_its_own_fallback() {
    let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25);
    let fb = stage
        .cpu_fallback()
        .expect("GpuGrayQamDemapper has a CpuGrayQamDemapper fallback");
    assert!(
        fb.cpu_fallback().is_some(),
        "CpuGrayQamDemapper must be its own cpu_fallback (CpuOnly stage)"
    );
    assert_eq!(
        fb.execution_class(),
        ExecutionClass::CpuOnly,
        "CpuGrayQamDemapper must report CpuOnly"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// GpuGrayQamDemapper (ExactLogMap) — routes to CPU, fallback still present
// ────────────────────────────────────────────────────────────────────────────

/// `GpuGrayQamDemapper` constructed for `ExactLogMap` has no GPU exact-log-map
/// kernel; it reports `CpuOnly` and routes `process` through the CPU fallback
/// (design doc §8: `ExactLogMap → ExecutionClass::CpuOnly`). The `cpu_fallback`
/// still returns `Some` so the executor can always access the fallback path.
#[test]
fn test_gpu_gray_qam_demapper_exact_log_map_is_cpu_only_with_fallback() {
    let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::ExactLogMap, 0.3);
    assert_eq!(
        stage.execution_class(),
        ExecutionClass::CpuOnly,
        "ExactLogMap GpuGrayQamDemapper must report CpuOnly (no GPU exact-log-map kernel)"
    );
    let fb = stage
        .cpu_fallback()
        .expect("ExactLogMap GpuGrayQamDemapper must still expose a CPU fallback");
    assert_eq!(
        fb.method(),
        DemapMethod::ExactLogMap,
        "ExactLogMap fallback must carry the ExactLogMap method"
    );
}

/// 64-QAM MaxLog fallback has the correct `m`.
#[test]
fn test_gpu_gray_qam_demapper_qam64_max_log_fallback_m() {
    let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam64, DemapMethod::MaxLog, 0.7);
    let fb = stage
        .cpu_fallback()
        .expect("GpuGrayQamDemapper (64-QAM MaxLog) must have a CPU fallback");
    assert_eq!(fb.bits_per_symbol(), 6, "64-QAM fallback must report m=6");
}
