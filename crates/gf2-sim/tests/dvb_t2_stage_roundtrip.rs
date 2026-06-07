//! Noiseless DVB-T2 BICM stage roundtrip integration test.
//!
//! Drives the [`gf2_sim::stages`] wrappers through the full forward + inverse
//! BICM chain with **no channel** and asserts that the recovered BBFRAME is
//! bit-identical to a seeded pseudo-random input, for at least the two required
//! MODCODs (Normal × 1/2 × 16-QAM and Normal × 1/2 × 64-QAM).
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
//!   → DvbT2Decode      (Llr → BitPacked, recovered BBFRAME)
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

use gf2_sim::batch::{BitPackedBatch, SymbolBatch};
use gf2_sim::stages::{
    BitDeinterleave, BitInterleave, DvbT2Decode, DvbT2Encode, GrayQamDemap, GrayQamMap,
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

/// Run the noiseless forward + inverse BICM chain on a single seeded BBFRAME
/// and assert bit-exact recovery for the given MODCOD.
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
    let recovered = decode.process(&deinterleaved, &mut ()).expect("decode");

    assert_eq!(
        recovered.frames[0], bbframe,
        "noiseless roundtrip must reconstruct BBFRAME bit-exactly for {rate:?} / {modulation:?}"
    );
}

#[test]
fn test_noiseless_roundtrip_r1_2_16qam() {
    assert_noiseless_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam16, 0xC0DE_F00D);
}

#[test]
fn test_noiseless_roundtrip_r1_2_64qam() {
    assert_noiseless_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam64, 0x5EED_1234);
}
