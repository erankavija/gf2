//! Each GPU stage declares a `Stage::CpuFallback` and returns it from
//! `cpu_fallback()`. Construction and fallback access use no device, so these
//! tests need the `hip` feature but no GPU.

#![cfg(feature = "hip")]

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderConfig, LdpcCode};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::gpu::awgn::GpuAwgn;
use gf2_sim::gpu::demap::GpuGrayQamDemapper;
use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
use gf2_sim::stage::{ExecutionClass, Stage};

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

#[test]
fn test_gpu_awgn_execution_class_is_gpu_only() {
    let stage = GpuAwgn::new(6.25, 4);
    assert_eq!(
        stage.execution_class(),
        ExecutionClass::GpuOnly,
        "GpuAwgn must report GpuOnly"
    );
}

#[test]
fn test_gpu_awgn_fallback_survives_seek_and_device_setters() {
    let stage = GpuAwgn::new(5.0, 6).with_seek(99, 2, 1).on_device(0);
    let fb = stage
        .cpu_fallback()
        .expect("GpuAwgn with seek must still have a CPU fallback");
    assert_eq!(fb.es_n0_db(), 5.0);
    assert_eq!(fb.bits_per_symbol(), 6);
}

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

#[test]
fn test_cpu_ldpc_bp_is_its_own_fallback() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let stage = GpuLdpcBp::new(code, DecoderConfig::default(), 10);
    let fb = stage
        .cpu_fallback()
        .expect("GpuLdpcBp has a CpuLdpcBp fallback");
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

#[test]
fn test_gpu_gray_qam_demapper_max_log_execution_class_is_gpu_only() {
    let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam64, DemapMethod::MaxLog, 0.5);
    assert_eq!(
        stage.execution_class(),
        ExecutionClass::GpuOnly,
        "MaxLog GpuGrayQamDemapper must report GpuOnly"
    );
}

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

#[test]
fn test_gpu_gray_qam_demapper_qam64_max_log_fallback_m() {
    let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam64, DemapMethod::MaxLog, 0.7);
    let fb = stage
        .cpu_fallback()
        .expect("GpuGrayQamDemapper (64-QAM MaxLog) must have a CPU fallback");
    assert_eq!(fb.bits_per_symbol(), 6, "64-QAM fallback must report m=6");
}
