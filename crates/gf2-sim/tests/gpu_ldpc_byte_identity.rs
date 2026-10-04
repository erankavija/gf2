//! CPU-vs-GPU identity of the LDPC BP hard decision: for DVB-T2 r1/2
//! (n = 64800) on identical channel LLRs, the [`GpuLdpcBp`] hard-decision
//! codeword equals the CPU [`LdpcDecoder::decode_to_codeword`] codeword bit for
//! bit under MinSum, NormalizedMinSum(0.75) and SumProduct. Skips when no GPU
//! is usable.

#![cfg(feature = "hip")]

use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder};
use gf2_coding::{CodeRate, Llr};
use gf2_core::BitVec;
use gf2_kernels_hip::host::device_mem_info;
use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
use gf2_sim::testutil::AwgnLlrSource;
use gf2_sim::LlrBatch;
use rayon::prelude::*;

fn algorithm_tag(alg: DecoderAlgorithm) -> u32 {
    match alg {
        DecoderAlgorithm::MinSum => 0,
        DecoderAlgorithm::NormalizedMinSum(_) => 1,
        DecoderAlgorithm::OffsetMinSum(_) => 2,
        DecoderAlgorithm::SumProduct => 3,
    }
}

#[test]
#[ignore = "sim: 200-frame n=64800 CPU-vs-GPU LDPC BP byte-identity over 3 SNRs x 3 algorithms (gfx1030-gated)"]
fn gpu_ldpc_hard_decision_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!(
            "skipping gpu_ldpc_hard_decision_byte_identical_to_cpu: no usable GPU \
             (device_mem_info failed)"
        );
        return;
    }

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let n = code.n();
    let max_iterations = 50usize;
    let frames_per_snr = 200usize;
    let sigmas = [0.95_f64, 0.80, 0.65];

    let algorithms = [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::SumProduct,
    ];

    for &algorithm in &algorithms {
        let config = DecoderConfig::new(algorithm, true);
        let stage = GpuLdpcBp::new(code.clone(), config, max_iterations);
        let decoder = stage
            .build_decoder(frames_per_snr)
            .expect("build GPU LDPC decoder on gfx1030");

        for (snr_idx, &sigma) in sigmas.iter().enumerate() {
            let seed = 0xA930_BE7F_0000_0000
                ^ ((algorithm_tag(algorithm) as u64) << 32)
                ^ (snr_idx as u64);
            let mut src = AwgnLlrSource::new(seed);
            let frames: Vec<Vec<Llr>> = (0..frames_per_snr)
                .map(|_| src.frame_all_zero(n, sigma))
                .collect();

            let cpu: Vec<BitVec> = frames
                .par_iter()
                .map(|llrs| {
                    let mut dec = LdpcDecoder::with_config(code.clone(), config);
                    dec.decode_to_codeword(llrs, max_iterations).decoded_bits
                })
                .collect();

            let gpu_batch = stage
                .decode_batch(&LlrBatch::new(frames.clone()), &decoder)
                .expect("gpu decode batch");
            assert_eq!(gpu_batch.frames.len(), frames_per_snr);

            for (frame_idx, (g, c)) in gpu_batch.frames.iter().zip(cpu.iter()).enumerate() {
                assert_eq!(
                    g.len(),
                    c.len(),
                    "alg={algorithm:?} snr_idx={snr_idx} frame={frame_idx}: \
                     length mismatch ({} vs {})",
                    g.len(),
                    c.len()
                );
                if g != c {
                    let first = (0..n).find(|&b| g.get(b) != c.get(b));
                    panic!(
                        "BYTE-IDENTITY VIOLATION alg={algorithm:?} snr_idx={snr_idx} \
                         (sigma={sigma}) frame={frame_idx}: hard decision differs at \
                         first bit {first:?} (gpu={:?}, cpu={:?})",
                        first.map(|b| g.get(b)),
                        first.map(|b| c.get(b)),
                    );
                }
            }
        }
    }
}
