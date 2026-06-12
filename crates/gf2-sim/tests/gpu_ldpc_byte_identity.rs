//! CPU-vs-GPU byte-identity of the LDPC belief-propagation hard decision
//! (issue `a930be7f`, criterion 1; design doc §11).
//!
//! For DVB-T2 r1/2 (n = 64800) at a fixed seed, 200 frames at each of three
//! SNRs, the GPU [`GpuLdpcBp`](gf2_sim::gpu::ldpc_bp::GpuLdpcBp) hard-decision
//! codeword must equal the CPU
//! [`LdpcDecoder::decode_to_codeword`](gf2_coding::ldpc::LdpcDecoder)
//! hard-decision codeword **bit-for-bit**, across MinSum, NormalizedMinSum(0.75),
//! and SumProduct. The hard-decision verdict is robust to the 1-3 ULP RDNA2
//! transcendental drift (design §11), so bit-for-bit holds even for SumProduct's
//! `tanh`/`atanh` box-plus.
//!
//! The test feeds the **same** channel LLRs to both paths (so the comparison is
//! purely decode-vs-decode) and is gated on GPU presence — it skips cleanly with
//! no usable GPU, like the other `gf2-sim` GPU tests.
//!
//! Performance: the GPU decodes each 200-frame SNR set in one batched call (one
//! set of per-iteration kernel launches over the whole batch); the CPU reference
//! runs the same 200 frames across the rayon pool (per-frame independent
//! `LdpcDecoder`s — the per-frame outcome is deterministic regardless of which
//! thread runs it). This keeps the full 3-algorithm × 3-SNR × 200-frame sweep
//! within the 120 s slow-tier budget. It carries `#[ignore]` per the CLAUDE.md
//! test-tier rules; run command in the receipt.

#![cfg(feature = "hip")]

use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder};
use gf2_coding::{CodeRate, Llr};
use gf2_core::BitVec;
use gf2_kernels_hip::host::device_mem_info;
use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
// The deterministic SplitMix64 + Box-Muller AWGN LLR source is the shared
// `testutil::AwgnLlrSource` (SSOT; review F3, jit:23d3525f) — bit-identical
// draw sequence to the original in-file copy, so the pinned per-seed LLR
// frames (and thus the byte-identity outcomes) are unchanged.
use gf2_sim::testutil::AwgnLlrSource;
use gf2_sim::LlrBatch;
use rayon::prelude::*;

/// A stable small tag per algorithm for seeding (so each algorithm's frame
/// population is distinct and reproducible).
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

    // Three SNRs spanning the waterfall: a noisy point (≈50 BP iterations / some
    // frames at the floor), a waterfall point (≈26 iterations, successful
    // decode), and a clean point (fast convergence). Sigmas for the all-zero
    // BPSK signal; the mix produces a variety of decode outcomes so the
    // comparison is non-vacuous across the early-termination depth.
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
            // Fixed per-(algorithm, SNR) seed so the LLR frames are reproducible
            // and identical between the CPU and GPU passes.
            let seed = 0xA930_BE7F_0000_0000
                ^ ((algorithm_tag(algorithm) as u64) << 32)
                ^ (snr_idx as u64);
            let mut src = AwgnLlrSource::new(seed);
            let frames: Vec<Vec<Llr>> = (0..frames_per_snr)
                .map(|_| src.frame_all_zero(n, sigma))
                .collect();

            // CPU reference: full n-bit hard-decision codeword per frame, run
            // across the rayon pool (each frame's outcome is a deterministic pure
            // function of its LLRs, independent of thread).
            let cpu: Vec<BitVec> = frames
                .par_iter()
                .map(|llrs| {
                    let mut dec = LdpcDecoder::with_config(code.clone(), config);
                    dec.decode_to_codeword(llrs, max_iterations).decoded_bits
                })
                .collect();

            // GPU: the same LLR frames in one batched decode.
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
                    // A single differing bit means a decision-boundary flip — the
                    // lead must hear exactly where (do NOT relax criterion 1).
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
