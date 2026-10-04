//! CPU-vs-GPU identity of the 5G NR LDPC BP hard decision: on identical channel
//! LLRs, [`GpuNr5gDecoder`] recovers the same message bits as the CPU 5G NR
//! decoder. A small BG2 code runs in the fast tier; BG1 with Z = 384 at rate
//! 1/2 runs ignored. Both skip when no GPU is usable.

#![cfg(feature = "hip")]

use gf2_coding::ldpc::nr_5g::lifting_set_index;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, QuasiCyclicLdpc};
use gf2_coding::Llr;
use gf2_core::BitVec;
use gf2_kernels_hip::host::device_mem_info;
use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
use gf2_sim::testutil::AwgnLlrSource;
use gf2_sim::LlrBatch;
use std::sync::Arc;

fn assert_batch_byte_identical(
    dec: &GpuNr5gDecoder,
    decoder: &gf2_kernels_hip::GpuLdpcBp,
    frames: &[Vec<Llr>],
    label: &str,
) {
    let gpu = dec
        .decode_batch(&LlrBatch::new(frames.to_vec()), decoder)
        .expect("gpu nr decode batch");
    assert_eq!(gpu.frames.len(), frames.len(), "{label}: frame count");

    for (frame_idx, (g, channel)) in gpu.frames.iter().zip(frames.iter()).enumerate() {
        let c = dec.cpu_reference_message(channel);
        assert_eq!(
            g.len(),
            c.len(),
            "{label} frame {frame_idx}: message length mismatch ({} vs {})",
            g.len(),
            c.len()
        );
        if *g != c {
            let first = (0..g.len()).find(|&b| g.get(b) != c.get(b));
            panic!(
                "BYTE-IDENTITY VIOLATION {label} frame {frame_idx}: recovered message \
                 differs at first bit {first:?} (gpu={:?}, cpu={:?})",
                first.map(|b| g.get(b)),
                first.map(|b| c.get(b)),
            );
        }
    }
}

#[test]
fn gpu_nr_5g_smoke_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_nr_5g_smoke_byte_identical_to_cpu: no usable GPU");
        return;
    }

    let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let max_iterations = 25usize;
    let dec = GpuNr5gDecoder::new(code.clone(), config, max_iterations);
    let frames_per_batch = 8usize;

    let decoder = dec
        .build_decoder(frames_per_batch)
        .expect("build GPU NR decoder on gfx1030");

    use gf2_coding::traits::BlockEncoder;
    let mut msg = BitVec::with_capacity(121);
    for i in 0..121 {
        msg.push_bit(i % 4 == 1);
    }
    let cw = code.encode(&msg);

    let mut src = AwgnLlrSource::new(0x23D3_525F_0050_0E5E);
    let frames: Vec<Vec<Llr>> = (0..frames_per_batch)
        .map(|_| src.frame_for_codeword(&cw, 0.70))
        .collect();

    assert_batch_byte_identical(&dec, &decoder, &frames, "BG2 n=256 k=121 NMS");
}

#[test]
#[ignore = "sim: 200-frame BG1 Z=384 r1/2 CPU-vs-GPU 5G NR LDPC byte-identity over 3 SNRs (gfx1030-gated)"]
fn gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu: no usable GPU");
        return;
    }

    // 384 = 3 * 2^7: the a = 3 lifting set.
    assert_eq!(
        lifting_set_index(384),
        Some(1),
        "Z = 384 is lifting set i_LS = 1"
    );

    let target_k = 22 * 384;
    let target_n = 2 * target_k;
    let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, target_n, target_k));
    assert_eq!(code.params().lifting_factor, 384, "realised Z = 384");
    assert_eq!(code.params().target_k, target_k);
    assert_eq!(code.params().target_n, target_n);

    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let max_iterations = 25usize;
    let dec = GpuNr5gDecoder::new(code.clone(), config, max_iterations);
    let frames_per_snr = 200usize;

    let decoder = dec
        .build_decoder(frames_per_snr)
        .expect("build GPU NR decoder on gfx1030");

    use gf2_coding::traits::BlockEncoder;
    let mut msg = BitVec::with_capacity(target_k);
    for i in 0..target_k {
        msg.push_bit(i % 7 < 3);
    }
    let cw = code.encode(&msg);

    let sigmas = [0.88_f64, 0.78, 0.68];
    for (snr_idx, &sigma) in sigmas.iter().enumerate() {
        let seed = 0x23D3_525F_0000_0000 ^ (snr_idx as u64);
        let mut src = AwgnLlrSource::new(seed);
        let frames: Vec<Vec<Llr>> = (0..frames_per_snr)
            .map(|_| src.frame_for_codeword(&cw, sigma))
            .collect();
        assert_batch_byte_identical(
            &dec,
            &decoder,
            &frames,
            &format!("BG1 Z=384 r1/2 QPSK snr_idx={snr_idx} sigma={sigma}"),
        );
    }
}
