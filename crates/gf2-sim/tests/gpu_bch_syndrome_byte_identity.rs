//! CPU-vs-GPU byte-identity of the BCH syndrome evaluator and outcome
//! identity of GPU-assisted decoding (issue `9012f8a0`, correctness ladder
//! rungs 4-5; design doc §10).
//!
//! Rung 4 — DVB-T2 Short (GF(2^14)) and Normal (GF(2^16)) syndrome
//!   byte-identity: 200 frames per config at a fixed seed, MIXED valid
//!   codewords (all-zero syndromes), `<= t` correctable errors, and `> t`
//!   uncorrectable errors. All `2t` u16 syndromes equal the CPU
//!   `BchDecoder::compute_syndromes` with ZERO tolerance (exact integer GF
//!   arithmetic — no ULP drift, unlike LDPC).
//! Rung 5 — outcome identity: over the canonical construction model,
//!   `BinaryBchDecoder::correct_batch_gpu` reports the same
//!   `BchDecodeOutcome` and leaves the same corrected word as the per-word CPU
//!   `BinaryBchDecoder::correct_in_place`, frame for frame. Its two codes are
//!   the primitive narrow-sense mother codes of the two DVB-T2 configurations
//!   of rung 4: the same field presentation and the same radius 12, so the
//!   same 168- and 192-bit parity structure, over the full primitive length.
//!
//! Gated on GPU presence — skips cleanly when `device_mem_info().is_err()`,
//! like the other `gf2-sim` GPU tests. Carries `#[ignore]` per the AGENTS.md
//! test-tier rules; run command in the receipt.

#![cfg(feature = "hip")]

use gf2_coding::bch::dvb_t2::{DvbBchParams, FrameSize};
use gf2_coding::bch::spec::{BchSpec, DesignedDistance};
use gf2_coding::bch::{
    BchCode, BchDecodeOutcome, BchDecoder, BchEncoder, BinaryBchCode, BinaryBchDecoder,
};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::Gf2mField;
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

/// Injects `errors` flips at distinct random coordinates of `word`.
fn inject(word: &mut BitVec, errors: usize, rng: &mut SplitMix64) {
    let mut flipped = std::collections::HashSet::new();
    while flipped.len() < errors {
        let position = rng.below(word.len());
        if flipped.insert(position) {
            word.set(position, !word.get(position));
        }
    }
}

/// The error count of frame `f` in a mixed population: valid, `1..=t`
/// correctable, or `t+1..=2t+1` uncorrectable, cycling in that order.
fn mixed_error_count(f: usize, t: usize, rng: &mut SplitMix64) -> usize {
    match f % 3 {
        0 => 0,
        1 => 1 + rng.below(t),
        _ => (t + 1) + rng.below(t + 1),
    }
}

// ---------------------------------------------------------------------------
// Rung 4 — the device syndrome evaluator, on the DVB-T2 configurations
// ---------------------------------------------------------------------------

/// One fixture frame: a valid codeword with a chosen number of bit errors
/// injected at distinct random positions.
fn build_frame(encoder: &BchEncoder, k: usize, errors: usize, rng: &mut SplitMix64) -> BitVec {
    // Random message, systematic encode -> valid codeword.
    let mut msg = BitVec::zeros(k);
    for i in 0..k {
        if rng.next_u64() & 1 == 1 {
            msg.set(i, true);
        }
    }
    let mut cw = encoder.encode(&msg);
    inject(&mut cw, errors, rng);
    cw
}

/// Builds the 200-frame mixed population for one config: ~1/3 valid, ~1/3
/// `<= t` errors, ~1/3 `> t` errors (deterministic per seed).
fn mixed_population(
    encoder: &BchEncoder,
    k: usize,
    t: usize,
    frames: usize,
    seed: u64,
) -> Vec<BitVec> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let errors = mixed_error_count(f, t, &mut rng);
        out.push(build_frame(encoder, k, errors, &mut rng));
    }
    out
}

fn run_syndrome_identity(frame_size: FrameSize, label: &str) {
    let code = BchCode::dvb_t2(frame_size, CodeRate::Rate1_2);
    let k = code.k();
    let t = code.t();
    let two_t = 2 * t;
    let encoder = BchEncoder::new(code.clone());
    let decoder = BchDecoder::new(code);

    let frames = 200usize;
    let seed = 0x9012_F8A0_0000_0001 ^ (label.len() as u64);
    let population = mixed_population(&encoder, k, t, frames, seed);

    // GPU syndromes == CPU syndromes, every frame, zero tolerance.
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

    eprintln!("{label}: {frames} frames, {two_t} syndromes/frame, byte-identical (CPU==GPU)");
}

// ---------------------------------------------------------------------------
// Rung 5 — outcome identity of GPU-assisted decoding, canonical model
// ---------------------------------------------------------------------------

/// The primitive narrow-sense mother code of one DVB-T2 configuration: the
/// same splitting-field presentation and the same radius, so the same parity
/// structure, over the full primitive block length the model constructs.
fn mother_code(params: &DvbBchParams) -> BinaryBchCode {
    let field = Gf2mField::new(params.field_m, params.primitive_poly).with_tables();
    let code = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension: BinaryPrimeExt::new(field).expect("the DVB-T2 modulus is primitive"),
        designed_distance: DesignedDistance::try_from(2 * params.t as u64 + 1)
            .expect("a positive designed distance"),
    })
    .expect("a valid primitive narrow-sense spec");
    assert_eq!(code.correction_radius(), params.t);
    assert_eq!(
        code.n() - code.k(),
        params.n - params.k,
        "the mother code carries the DVB-T2 parity structure"
    );
    code
}

/// A canonical codeword: a sum of `shifts` random shifts of the generator, and
/// therefore a multiple of it. Building one this way costs `O(shifts * deg g)`
/// instead of the `O(k * deg g)` of an explicit message multiplication.
fn random_codeword(
    length: usize,
    generator_terms: &[usize],
    shifts: usize,
    rng: &mut SplitMix64,
) -> BitVec {
    let degree = *generator_terms.last().expect("a nonzero generator");
    let mut word = BitVec::zeros(length);
    for _ in 0..shifts {
        let offset = rng.below(length - degree);
        for term in generator_terms {
            let position = offset + term;
            word.set(position, !word.get(position));
        }
    }
    word
}

fn run_outcome_identity(frame_size: FrameSize, label: &str) {
    let params = DvbBchParams::for_code(frame_size, CodeRate::Rate1_2);
    let code = mother_code(&params);
    let decoder = BinaryBchDecoder::new(&code);
    let t = code.correction_radius();

    let generator = code.generator();
    let generator_terms: Vec<usize> = (0..=generator.degree().expect("a nonzero generator"))
        .filter(|&index| generator.coeff(index).is_one())
        .collect();

    let frames = 200usize;
    let seed = 0xC3CC_5226_0000_0001 ^ (label.len() as u64);
    let mut rng = SplitMix64::new(seed);
    let population: Vec<BitVec> = (0..frames)
        .map(|f| {
            let mut word = random_codeword(code.n(), &generator_terms, 64, &mut rng);
            inject(&mut word, mixed_error_count(f, t, &mut rng), &mut rng);
            word
        })
        .collect();

    // The per-word CPU fast path is the oracle.
    let mut workspace = decoder.workspace();
    let mut expected_words = population.clone();
    let expected: Vec<BchDecodeOutcome> = expected_words
        .iter_mut()
        .map(|word| {
            decoder
                .correct_in_place(word, &mut workspace)
                .expect("the received word has length n")
        })
        .collect();

    // GPU-assisted: device syndromes, CPU locator search, device verification.
    let mut words = population.clone();
    let outcomes = decoder
        .correct_batch_gpu(&mut words)
        .expect("GPU-assisted decode batch");

    assert_eq!(outcomes.len(), frames);
    for f in 0..frames {
        assert_eq!(
            outcomes[f], expected[f],
            "{label} frame {f}: GPU-assisted outcome != CPU outcome"
        );
        assert_eq!(
            words[f], expected_words[f],
            "{label} frame {f}: GPU-assisted corrected word != CPU corrected word"
        );
    }

    // The population must reach all three outcomes, or the identity above
    // would be pinning only the easy arm.
    let count = |wanted: BchDecodeOutcome| outcomes.iter().filter(|o| **o == wanted).count();
    let corrected = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, BchDecodeOutcome::Corrected { .. }))
        .count();
    let clean = count(BchDecodeOutcome::NoErrors);
    let uncorrectable = count(BchDecodeOutcome::Uncorrectable);
    assert!(
        clean > 0 && corrected > 0 && uncorrectable > 0,
        "{label}: outcomes {clean} clean, {corrected} corrected, {uncorrectable} uncorrectable"
    );

    eprintln!(
        "{label}: {frames} frames over BCH({}, {}), t = {t}, outcome-identical (CPU==GPU): \
         {clean} clean, {corrected} corrected, {uncorrectable} uncorrectable",
        code.n(),
        code.k()
    );
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

#[test]
#[ignore = "sim: 200-frame DVB-T2 Short BCH GF(2^14) CPU-vs-GPU syndrome byte-identity (gfx1030-gated)"]
fn test_gpu_bch_syndrome_short_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping test_gpu_bch_syndrome_short_byte_identical_to_cpu: no usable GPU");
        return;
    }
    run_syndrome_identity(FrameSize::Short, "dvb-t2-short-r1/2-gf14");
}

#[test]
#[ignore = "sim: 200-frame DVB-T2 Normal BCH GF(2^16) CPU-vs-GPU syndrome byte-identity (gfx1030-gated)"]
fn test_gpu_bch_syndrome_normal_byte_identical_to_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping test_gpu_bch_syndrome_normal_byte_identical_to_cpu: no usable GPU");
        return;
    }
    run_syndrome_identity(FrameSize::Normal, "dvb-t2-normal-r1/2-gf16");
}

#[test]
#[ignore = "sim: 200-frame GF(2^14) mother-code CPU-vs-GPU decode-outcome identity (gfx1030-gated)"]
fn test_gpu_assisted_decode_outcome_identical_to_cpu_gf14() {
    if device_mem_info().is_err() {
        eprintln!("skipping test_gpu_assisted_decode_outcome_identical_to_cpu_gf14: no usable GPU");
        return;
    }
    run_outcome_identity(FrameSize::Short, "mother-gf14-t12");
}

#[test]
#[ignore = "sim: 200-frame GF(2^16) mother-code CPU-vs-GPU decode-outcome identity (gfx1030-gated)"]
fn test_gpu_assisted_decode_outcome_identical_to_cpu_gf16() {
    if device_mem_info().is_err() {
        eprintln!("skipping test_gpu_assisted_decode_outcome_identical_to_cpu_gf16: no usable GPU");
        return;
    }
    run_outcome_identity(FrameSize::Normal, "mother-gf16-t12");
}
