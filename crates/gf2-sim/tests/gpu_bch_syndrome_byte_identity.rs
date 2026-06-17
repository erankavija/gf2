//! CPU-vs-GPU byte-identity of the BCH syndrome evaluator + decode-equivalence
//! (issue `9012f8a0`, correctness ladder rungs 4-5; design doc §10).
//!
//! Rung 4 — DVB-T2 Short (GF(2^14)) and Normal (GF(2^16)) syndrome
//!   byte-identity: 200 frames per config at a fixed seed, MIXED valid
//!   codewords (all-zero syndromes), `<= t` correctable errors, and `> t`
//!   uncorrectable errors. All `2t` u16 syndromes equal the CPU
//!   `BchDecoder::compute_syndromes` with ZERO tolerance (exact integer GF
//!   arithmetic — no ULP drift, unlike LDPC).
//! Rung 5 — decode-equivalence: GPU syndromes fed into the CPU
//!   Berlekamp-Massey + Chien pipeline (`decode_batch_gpu`) decode the SAME
//!   messages as the CPU-only pipeline (`decode_batch`) on the same frames.
//!
//! Gated on GPU presence — skips cleanly when `device_mem_info().is_err()`,
//! like the other `gf2-sim` GPU tests. Carries `#[ignore]` per the CLAUDE.md
//! test-tier rules; run command in the receipt.

#![cfg(feature = "hip")]

use gf2_coding::bch::dvb_t2::FrameSize;
use gf2_coding::bch::{BchCode, BchDecoder, BchEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use gf2_kernels_hip::host::device_mem_info;

/// Deterministic SplitMix64 — a self-contained PRNG so the fixture frames (and
/// thus the byte-identity outcomes) are reproducible without an external dep.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A uniform `usize` in `0..bound`.
    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// One fixture frame: a valid codeword with a chosen number of bit errors
/// injected at distinct random positions.
fn build_frame(
    encoder: &BchEncoder,
    k: usize,
    n: usize,
    errors: usize,
    rng: &mut SplitMix64,
) -> BitVec {
    // Random message, systematic encode -> valid codeword.
    let mut msg = BitVec::zeros(k);
    for i in 0..k {
        if rng.next_u64() & 1 == 1 {
            msg.set(i, true);
        }
    }
    let mut cw = encoder.encode(&msg);
    // Inject `errors` flips at distinct positions.
    let mut flipped = std::collections::HashSet::new();
    while flipped.len() < errors {
        let pos = rng.below(n);
        if flipped.insert(pos) {
            cw.set(pos, !cw.get(pos));
        }
    }
    cw
}

/// Builds the 200-frame mixed population for one config: ~1/3 valid, ~1/3
/// `<= t` errors, ~1/3 `> t` errors (deterministic per seed).
fn mixed_population(
    encoder: &BchEncoder,
    k: usize,
    n: usize,
    t: usize,
    frames: usize,
    seed: u64,
) -> Vec<BitVec> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let errors = match f % 3 {
            0 => 0,                          // valid codeword
            1 => 1 + rng.below(t),           // 1..=t correctable errors
            _ => (t + 1) + rng.below(t + 1), // t+1..=2t+1 uncorrectable errors
        };
        out.push(build_frame(encoder, k, n, errors, &mut rng));
    }
    out
}

fn run_config(frame_size: FrameSize, label: &str) {
    let code = BchCode::dvb_t2(frame_size, CodeRate::Rate1_2);
    let n = code.n();
    let k = code.k();
    let t = code.t();
    let two_t = 2 * t;
    let encoder = BchEncoder::new(code.clone());
    let decoder = BchDecoder::new(code);

    let frames = 200usize;
    let seed = 0x9012_F8A0_0000_0001 ^ (label.len() as u64);
    let population = mixed_population(&encoder, k, n, t, frames, seed);

    // Rung 4: GPU syndromes == CPU syndromes, every frame, zero tolerance.
    let gpu_syndromes = decoder
        .compute_syndromes_batch_gpu(&population)
        .expect("GPU syndrome batch");
    assert_eq!(gpu_syndromes.len(), frames);
    for (f, frame) in population.iter().enumerate() {
        let cpu = decoder.compute_syndromes(frame);
        let gpu = &gpu_syndromes[f];
        assert_eq!(gpu.len(), two_t, "{label} frame {f}: syndrome count");
        for i in 0..two_t {
            assert_eq!(
                gpu[i].value(),
                cpu[i].value(),
                "{label} frame {f}: S_{} GPU {} != CPU {}",
                i + 1,
                gpu[i].value(),
                cpu[i].value()
            );
        }
    }

    // Rung 5: GPU-syndrome decode == CPU-only decode, every frame.
    let gpu_decoded = decoder
        .decode_batch_gpu(&population)
        .expect("GPU-syndrome decode batch");
    let cpu_decoded = decoder.decode_batch(&population);
    assert_eq!(gpu_decoded.len(), cpu_decoded.len());
    for (f, (g, c)) in gpu_decoded.iter().zip(cpu_decoded.iter()).enumerate() {
        assert_eq!(g, c, "{label} frame {f}: GPU-syndrome decode != CPU decode");
    }

    eprintln!("{label}: {frames} frames, {two_t} syndromes/frame, byte-identical (CPU==GPU) + decode-equivalent");
}

#[test]
#[ignore = "sim: 200-frame DVB-T2 Short BCH GF(2^14) CPU-vs-GPU syndrome byte-identity + decode-equiv (gfx1030-gated)"]
fn gpu_bch_syndrome_short_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_bch_syndrome_short_byte_identical_to_cpu: no usable GPU");
        return;
    }
    run_config(FrameSize::Short, "dvb-t2-short-r1/2-gf14");
}

#[test]
#[ignore = "sim: 200-frame DVB-T2 Normal BCH GF(2^16) CPU-vs-GPU syndrome byte-identity + decode-equiv (gfx1030-gated)"]
fn gpu_bch_syndrome_normal_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_bch_syndrome_normal_byte_identical_to_cpu: no usable GPU");
        return;
    }
    run_config(FrameSize::Normal, "dvb-t2-normal-r1/2-gf16");
}
