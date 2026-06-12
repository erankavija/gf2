//! CPU-vs-GPU byte-identity of the 5G NR LDPC belief-propagation hard decision
//! (issue `23d3525f`, deliverable 4 / criterion 2; design doc §11).
//!
//! This is the end-to-end proof that the **host-side 5G NR flat-layout builder**
//! ([`GpuNr5gDecoder`](gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder)) — which expands
//! a 5G NR base graph + per-`i_LS` shift table into the flat
//! [`LdpcGraphLayout`](gf2_kernels_hip::launch_ldpc_bp::LdpcGraphLayout) via the
//! **existing, unchanged** GPU kernel — decodes a real 5G NR lifted code
//! byte-identically to the CPU 5G NR
//! [`Nr5gRateMatchedDecoder`](gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder).
//! "Same kernel parameterises both standards" (design §6), proven end-to-end.
//!
//! Two legs (the established fast-smoke + slow-substantive pair):
//!
//! * [`gpu_nr_5g_smoke_byte_identical_to_cpu`] — **un-ignored**, GPU-gated: a
//!   small BG2 rate-matched code over a handful of frames. Keeps the [hard]
//!   byte-identity criterion under a fast-tier-visible guard (an `#[ignore]`-only
//!   proof of a [hard] criterion fails review).
//! * [`gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu`] — `#[ignore]`d slow leg:
//!   the **canonical** headline configuration (BG1, `i_LS` = 1, Z = 384,
//!   rate 1/2, QPSK) over 200 frames at three waterfall Es/N0 points. This is
//!   the configuration the throughput target is measured at.
//!
//! Both legs feed the **same** channel LLRs to both paths (so the comparison is
//! purely decode-vs-decode) and skip cleanly with no usable GPU, like the other
//! `gf2-sim` GPU tests. Per design §11 the hard-decision verdict is robust to
//! the 1-3 ULP RDNA2 transcendental drift, so the recovered message bits are
//! bit-for-bit identical even though `mean_iters` may differ across paths.

#![cfg(feature = "hip")]

use gf2_coding::ldpc::nr_5g::lifting_set_index;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, QuasiCyclicLdpc};
use gf2_coding::Llr;
use gf2_core::BitVec;
use gf2_kernels_hip::host::device_mem_info;
use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
use gf2_sim::LlrBatch;
use std::sync::Arc;

/// A self-contained deterministic AWGN LLR source over the **transmitted**
/// 5G NR codeword (cribbed from `gpu_ldpc_byte_identity.rs`'s `LlrSource`): a
/// SplitMix64 stream drives a Box-Muller cosine transform for N(0, 1) noise,
/// added to a BPSK-mapped codeword (bit b -> 1 - 2b). The channel LLR is
/// `2 * r / N0`. Both paths see the **identical** LLRs, so the exact
/// distribution is irrelevant to the byte-identity comparison — only that the
/// inputs are varied and reproducible.
struct LlrSource {
    state: u64,
}

impl LlrSource {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    fn next_normal(&mut self) -> f64 {
        let mut u1 = self.next_uniform();
        let u2 = self.next_uniform();
        if u1 < 1e-15 {
            u1 = 1e-15;
        }
        let r = (-2.0 * u1.ln()).sqrt();
        r * (std::f64::consts::TAU * u2).cos()
    }

    /// One frame of channel LLRs over a transmitted codeword `cw` at noise std
    /// `sigma`. BPSK: bit b -> 1 - 2b (+1 for 0, -1 for 1).
    fn frame(&mut self, cw: &BitVec, sigma: f64) -> Vec<Llr> {
        let n0 = 2.0 * sigma * sigma;
        (0..cw.len())
            .map(|i| {
                let s = if cw.get(i) { -1.0 } else { 1.0 };
                let noise = self.next_normal() * sigma;
                let r = s + noise;
                Llr::new((2.0 * r / n0) as f32)
            })
            .collect()
    }
}

/// Compares the GPU and CPU recovered messages for a batch of channel-LLR
/// frames, panicking on the first differing bit (a decision-boundary flip — the
/// lead must hear exactly where; do NOT relax the criterion).
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

/// Fast-tier-visible smoke: a small BG2 rate-matched code (n = 256, k = 121)
/// over a few frames, NormalizedMinSum(0.75) — the GPU recovered message must
/// equal the CPU 5G NR decoder bit-for-bit. Un-ignored so the [hard]
/// byte-identity criterion has a fast guard; skips cleanly with no GPU.
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

    // Transmit a fixed nonzero message, encode, then add noise at a comfortable
    // Es/N0 so most frames decode (the comparison is decode-vs-decode either way).
    use gf2_coding::traits::BlockEncoder;
    let mut msg = BitVec::with_capacity(121);
    for i in 0..121 {
        msg.push_bit(i % 4 == 1);
    }
    let cw = code.encode(&msg);

    let mut src = LlrSource::new(0x23D3_525F_0050_0E5E);
    let frames: Vec<Vec<Llr>> = (0..frames_per_batch)
        .map(|_| src.frame(&cw, 0.70))
        .collect();

    assert_batch_byte_identical(&dec, &decoder, &frames, "BG2 n=256 k=121 NMS");
}

/// Slow substantive leg: the canonical headline configuration — BG1, `i_LS` = 1
/// (Z = 384), rate 1/2, QPSK (n = 16896, k = 8448), NormalizedMinSum(0.75),
/// 200 frames at each of three waterfall Es/N0 points. The GPU recovered
/// message must equal the CPU 5G NR decoder bit-for-bit. This is the exact
/// configuration the >= 200 Mbps throughput target is measured at.
#[test]
#[ignore = "sim: 200-frame BG1 Z=384 r1/2 CPU-vs-GPU 5G NR LDPC byte-identity over 3 SNRs (gfx1030-gated)"]
fn gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu: no usable GPU");
        return;
    }

    // Z = 384 belongs to lifting set i_LS = 1 (the a = 3 set: 384 = 3 * 2^7).
    assert_eq!(
        lifting_set_index(384),
        Some(1),
        "Z = 384 is lifting set i_LS = 1"
    );

    // BG1 r1/2 QPSK: k = 22 * 384 = 8448, n = 2k = 16896.
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

    // A fixed nonzero transmitted message, encoded once; three sigmas spanning
    // the waterfall give a mix of converged / floored frames so the byte-identity
    // comparison is non-vacuous across the early-termination depth.
    use gf2_coding::traits::BlockEncoder;
    let mut msg = BitVec::with_capacity(target_k);
    for i in 0..target_k {
        msg.push_bit(i % 7 < 3);
    }
    let cw = code.encode(&msg);

    let sigmas = [0.88_f64, 0.78, 0.68];
    for (snr_idx, &sigma) in sigmas.iter().enumerate() {
        let seed = 0x23D3_525F_0000_0000 ^ (snr_idx as u64);
        let mut src = LlrSource::new(seed);
        let frames: Vec<Vec<Llr>> = (0..frames_per_snr).map(|_| src.frame(&cw, sigma)).collect();
        assert_batch_byte_identical(
            &dec,
            &decoder,
            &frames,
            &format!("BG1 Z=384 r1/2 QPSK snr_idx={snr_idx} sigma={sigma}"),
        );
    }
}
