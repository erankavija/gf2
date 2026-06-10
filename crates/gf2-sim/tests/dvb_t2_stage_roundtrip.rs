//! Noiseless DVB-T2 BICM stage roundtrip integration test.
//!
//! Drives the [`gf2_sim::stages`] wrappers through the full forward + inverse
//! BICM chain with **no channel** and asserts that the recovered BBFRAME is
//! bit-identical to a seeded pseudo-random input, for at least the two required
//! MODCODs (Normal × 1/2 × 16-QAM and Normal × 1/2 × 64-QAM).
//!
//! Two test families are provided:
//!
//! 1. **Manual-chain tests** (`test_noiseless_roundtrip_*`): construct each
//!    stage explicitly and drive them via the typed `Stage::process` API.
//!
//! 2. **Factory-driven tests** (`test_factory_roundtrip_*`): build the chain
//!    via [`dvb_t2_bicm_stages`] and drive it through the erased
//!    `AnyStage::process_any` path — the same path the executor and downstream
//!    waves consume. This is the primary criterion test: the factory is the
//!    documented single shared wiring source for downstream waves
//!    (`3fcb7025`/`c09d3e95`/`81d05bab`) and must be exercised so regressions
//!    in the factory's stage ordering or type threading surface immediately.
//!
//! Chain under test (per the stage inventory in `gf2_sim::stages`):
//!
//! ```text
//! BBFRAME bits
//!   → DvbT2Encode      (BitPacked → BitPacked, FECFRAME coded bits)
//!   → BitInterleave    (BitPacked → BitPacked)
//!   → GrayQamMap       (BitPacked → Symbol)
//!   --- noiseless: symbols pass straight through, no channel stage ---
//!   → GrayQamDemap     (Symbol → Llr)
//!   → BitDeinterleave  (Llr → Llr)
//!   → DvbT2Decode      (Llr → HardDecision, recovered BBFRAME)
//! ```
//!
//! These are full encode + LDPC-decode roundtrips on the Normal (n=64800)
//! FECFRAME. On noiseless input the LDPC belief propagation early-terminates
//! after one iteration and the DVB-T2 LDPC encoder is the linear-time IRA
//! staircase accumulator, so each roundtrip runs in well under the 5 s
//! fast-tier per-test budget (~0.07 s measured); they therefore run in the
//! default fast tier and need no `#[ignore]`.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
// `Rng` is in scope for `random::<bool>()` (rand 0.9 API).
use std::sync::Arc;

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch, SymbolBatch};
use gf2_sim::stage::{AnyScratch, AnyStage, TypedBatch};
use gf2_sim::stages::{
    dvb_t2_bicm_stages, BitDeinterleave, BitInterleave, DecodeScratch, DvbT2Decode, DvbT2Encode,
    GrayQamDemap,
    GrayQamMap, DEFAULT_DEMAP_NOISE_VAR,
};
use gf2_sim::Stage;

/// Build one seeded pseudo-random BBFRAME of `k` bits.
fn random_bbframe(k: usize, seed: u64) -> BitVec {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut bb = BitVec::with_capacity(k);
    for _ in 0..k {
        bb.push_bit(rng.random::<bool>());
    }
    bb
}

/// Drive a chain of erased stages sequentially via `process_any`.
///
/// Each stage's scratch is allocated through its own
/// [`AnyStage::default_scratch`] hook (the decode stage carries a
/// `DecodeScratch`; the rest use `()`). Returns the final type-erased output
/// batch.
fn run_erased_chain(
    stages: &[Box<dyn AnyStage>],
    initial: Box<dyn TypedBatch>,
) -> Box<dyn TypedBatch> {
    stages.iter().fold(initial, |batch, stage| {
        let mut scratch: Box<dyn AnyScratch> = stage.default_scratch();
        stage
            .process_any(batch.as_ref(), scratch.as_mut())
            .expect("process_any must succeed in noiseless chain")
    })
}

/// Run the noiseless forward + inverse BICM chain via `dvb_t2_bicm_stages`
/// (the erased `process_any` path) and assert bit-exact BBFRAME recovery.
fn assert_factory_roundtrip(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
    // Noiseless chain: GrayQamMap connects straight to GrayQamDemap with no
    // channel, so the demapper uses the default placeholder N0.
    let stages = dvb_t2_bicm_stages(
        rate,
        modulation,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
        DemapMethod::ExactLogMap,
        DEFAULT_DEMAP_NOISE_VAR,
    );

    let bbframe = random_bbframe(stages.codec.k_bch(), seed);
    let input: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![bbframe.clone()]));

    // Forward chain: BitPackedBatch → BitPackedBatch → BitPackedBatch → SymbolBatch.
    let after_forward = run_erased_chain(&stages.forward, input);

    // Noiseless channel: the SymbolBatch feeds straight into the inverse chain.
    // Inverse chain: SymbolBatch → LlrBatch → LlrBatch → HardDecisionBatch.
    let after_inverse = run_erased_chain(&stages.inverse, after_forward);

    // The terminal output is HardDecisionBatch; downcast and compare.
    let recovered = after_inverse
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("factory inverse chain must produce HardDecisionBatch");

    assert_eq!(
        recovered.frames[0], bbframe,
        "factory noiseless roundtrip must reconstruct BBFRAME bit-exactly \
         for {rate:?} / {modulation:?}"
    );
}

/// Run the noiseless forward + inverse BICM chain on a single seeded BBFRAME
/// and assert bit-exact recovery for the given MODCOD (manual-chain variant).
fn assert_noiseless_roundtrip(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
    // Shared codec + interleaver (Normal frame, n=64800).
    let mut concat = DvbT2Concat::new(FrameSize::Normal, rate).expect("codec construction");
    concat.set_decoder_config(DecoderConfig::new(DecoderAlgorithm::SumProduct, true));
    let codec = Arc::new(concat);
    let interleaver = Arc::new(DvbT2BitInterleaver::new(DvbT2Modcod::new(
        FrameSize::Normal,
        rate,
        modulation,
    )));

    let encode = DvbT2Encode::new(codec.clone());
    let interleave = BitInterleave::new(interleaver.clone());
    let map = GrayQamMap::new(modulation);
    let demap = GrayQamDemap::new(modulation, DemapMethod::ExactLogMap);
    let deinterleave = BitDeinterleave::new(interleaver.clone());
    let decode = DvbT2Decode::new(codec.clone());

    let bbframe = random_bbframe(codec.k_bch(), seed);
    let input = BitPackedBatch::new(vec![bbframe.clone()]);

    // Forward chain.
    let coded = encode.process(&input, &mut ()).expect("encode");
    assert_eq!(coded.frames[0].len(), codec.n_ldpc(), "FECFRAME length");

    let interleaved = interleave.process(&coded, &mut ()).expect("interleave");
    let symbols: SymbolBatch = map.process(&interleaved, &mut ()).expect("map");

    // Noiseless channel: symbols feed straight into the demapper.
    let llrs = demap.process(&symbols, &mut ()).expect("demap");
    let deinterleaved = deinterleave.process(&llrs, &mut ()).expect("deinterleave");
    let mut decode_scratch = DecodeScratch::default();
    let recovered: HardDecisionBatch = decode
        .process(&deinterleaved, &mut decode_scratch)
        .expect("decode");

    assert_eq!(
        recovered.frames[0], bbframe,
        "noiseless roundtrip must reconstruct BBFRAME bit-exactly for {rate:?} / {modulation:?}"
    );
    // The decode stage records the genuine BP iteration count per frame; a
    // noiseless decode converges on the first BP pass.
    assert_eq!(
        decode_scratch.iterations,
        vec![1],
        "noiseless decode converges in one BP iteration"
    );
}

// ---------------------------------------------------------------------------
// Factory-driven roundtrip tests (primary criterion: exercises dvb_t2_bicm_stages)
// ---------------------------------------------------------------------------

#[test]
fn test_factory_roundtrip_r1_2_16qam() {
    assert_factory_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam16, 0xC0DE_F00D);
}

#[test]
fn test_factory_roundtrip_r1_2_64qam() {
    assert_factory_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam64, 0x5EED_1234);
}

// ---------------------------------------------------------------------------
// Manual-chain roundtrip tests (kept as a typed-API sanity check)
// ---------------------------------------------------------------------------

#[test]
fn test_noiseless_roundtrip_r1_2_16qam() {
    assert_noiseless_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam16, 0xC0DE_F00D);
}

#[test]
fn test_noiseless_roundtrip_r1_2_64qam() {
    assert_noiseless_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam64, 0x5EED_1234);
}
