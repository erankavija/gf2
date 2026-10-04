//! Noiseless DVB-T2 BICM roundtrip through the [`gf2_sim::stages`] wrappers:
//! the forward and inverse chains with no channel recover a seeded
//! pseudo-random BBFRAME bit-identically, both through the typed
//! `Stage::process` API and through the [`dvb_t2_bicm_stages`] factory on the
//! erased `AnyStage::process_any` path.

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
use std::sync::Arc;

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch, SymbolBatch};
use gf2_sim::stage::{AnyScratch, AnyStage, TypedBatch};
use gf2_sim::stages::{
    dvb_t2_bicm_stages, BitDeinterleave, BitInterleave, DecodeScratch, DvbT2Decode, DvbT2Encode,
    GrayQamDemap, GrayQamMap, DEFAULT_DEMAP_NOISE_VAR,
};
use gf2_sim::Stage;

fn random_bbframe(k: usize, seed: u64) -> BitVec {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut bb = BitVec::with_capacity(k);
    for _ in 0..k {
        bb.push_bit(rng.random::<bool>());
    }
    bb
}

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

fn assert_factory_roundtrip(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
    // No channel: GrayQamMap connects straight to GrayQamDemap, so the demapper
    // uses the default N0.
    let stages = dvb_t2_bicm_stages(
        rate,
        modulation,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
        DemapMethod::ExactLogMap,
        DEFAULT_DEMAP_NOISE_VAR,
    );

    let bbframe = random_bbframe(stages.codec.k_bch(), seed);
    let input: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![bbframe.clone()]));

    let after_forward = run_erased_chain(&stages.forward, input);

    let after_inverse = run_erased_chain(&stages.inverse, after_forward);

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

fn assert_noiseless_roundtrip(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
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

    let coded = encode.process(&input, &mut ()).expect("encode");
    assert_eq!(coded.frames[0].len(), codec.n_ldpc(), "FECFRAME length");

    let interleaved = interleave.process(&coded, &mut ()).expect("interleave");
    let symbols: SymbolBatch = map.process(&interleaved, &mut ()).expect("map");

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
    assert_eq!(
        decode_scratch.iterations,
        vec![1],
        "noiseless decode converges in one BP iteration"
    );
}

#[test]
fn test_factory_roundtrip_r1_2_16qam() {
    assert_factory_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam16, 0xC0DE_F00D);
}

#[test]
fn test_factory_roundtrip_r1_2_64qam() {
    assert_factory_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam64, 0x5EED_1234);
}

#[test]
fn test_noiseless_roundtrip_r1_2_16qam() {
    assert_noiseless_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam16, 0xC0DE_F00D);
}

#[test]
fn test_noiseless_roundtrip_r1_2_64qam() {
    assert_noiseless_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam64, 0x5EED_1234);
}
