//! [`DvbT2ConcatBatchDecoder`] under the shared batch contract against
//! [`DvbT2Concat::decode_soft_counted`], on a short-frame code of
//! `@/citation/Etsi2015` and on the VV001-CR35 TP04 and TP06 streams
//! (`@/citation/DvbVerification2010`) under `$DVB_TEST_VECTORS_PATH`.

#[path = "support/dvb_t2_concat_batch.rs"]
mod concat_batch;
mod test_vectors;

use concat_batch::{
    assert_batch_decodes_as_single_frame, decoder_on, routes, selectors, single_frame_result,
};
use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::dvb_t2::concat::{DvbT2Concat, DvbT2ConcatBatchDecoder};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::llr::Llr;
use gf2_coding::test_support::ldpc_batch_route_of;
use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::tuning::{self, DecodeSelectors};
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;
use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

const SEED: u64 = 0x7e02_b3c9;
const CAP: usize = 12;

/// Noise deviations of consecutive frames, from clean to undecodable, so that
/// a wave holds frames of each kind.
const SIGMAS: [f64; 6] = [0.3, 0.6, 1.3, 0.55, 1.0, 0.66];

/// The short-frame rate-1/2 codec with the iteration cap [`CAP`].
fn codec() -> DvbT2Concat {
    let mut codec = DvbT2Concat::new(FrameSize::Short, CodeRate::Rate1_2).unwrap();
    codec.set_max_ldpc_iterations(CAP);
    codec
}

/// `count` FECFRAMEs of random BBFRAMEs through BPSK and AWGN, frame `f` at
/// the deviation [`SIGMAS`] gives its index.
fn noisy_frames(codec: &DvbT2Concat, count: usize) -> Vec<Vec<Llr>> {
    let mut rng = StdRng::seed_from_u64(SEED);
    (0..count)
        .map(|f| {
            let fecframe = codec.encode(&BitVec::random(codec.k_bch(), &mut rng));
            let sigma = SIGMAS[f % SIGMAS.len()];
            let channel = AwgnChannel::from_variance(sigma * sigma);
            let scale = 2.0 / channel.variance();
            (0..fecframe.len())
                .map(|i| {
                    let symbol = if fecframe.get(i) { -1.0 } else { 1.0 };
                    Llr::new((scale * channel.transmit(symbol, &mut rng)) as f32)
                })
                .collect()
        })
        .collect()
}

fn noiseless_frame(fecframe: &BitVec) -> Vec<Llr> {
    (0..fecframe.len())
        .map(|i| Llr::new(if fecframe.get(i) { -10.0 } else { 10.0 }))
        .collect()
}

/// Runs the shared batch contract under each setup of [`routes`] whose
/// selectors open the lane route at `lane_min_frames` frames, over two full
/// waves and the start of a third.
fn assert_batch_contract(lane_min_frames: usize) {
    let codec = codec();
    let setups: Vec<_> = routes()
        .into_iter()
        .filter(|(selectors, ..)| selectors.lane_min_frames() == lane_min_frames)
        .collect();
    assert!(!setups.is_empty());
    for (selectors, backend, full_wave) in setups {
        let mut decoder = decoder_on(backend, &codec, selectors);
        let lanes = decoder.batch_granularity();
        assert_eq!(
            decoder.ldpc().batch_route(lanes),
            ldpc_batch_route_of(&codec.ldpc_code(), full_wave)
        );

        let frames = noisy_frames(&codec, 2 * lanes + 1);
        batch_soft_decoder_contract(
            &mut decoder,
            |llrs| single_frame_result(&codec, llrs),
            &frames,
            ConvergenceMix::EarlyLateAndNonConverging,
        );
    }
}

#[test]
fn the_batch_contract_holds_on_the_lane_route_of_each_backend() {
    assert_batch_contract(1);
}

#[test]
fn the_batch_contract_holds_with_short_waves_on_the_single_frame_route() {
    assert_batch_contract(4);
}

#[test]
fn the_batch_contract_holds_on_the_single_frame_route() {
    assert_batch_contract(DecodeSelectors::CONSERVATIVE.lane_min_frames());
}

#[test]
fn the_decoder_reports_the_shape_of_the_codec_and_the_ldpc_granularity() {
    let codec = codec();
    let decoder = DvbT2ConcatBatchDecoder::with_selectors(&codec, selectors(1));
    assert_eq!(decoder.frame_len(), codec.n_ldpc());
    assert_eq!(decoder.decoded_len(), codec.k_bch());
    assert_eq!(
        decoder.batch_granularity(),
        decoder.ldpc().batch_granularity()
    );
    assert_eq!(decoder.ldpc().max_iterations(), CAP);
}

#[test]
fn a_non_converging_frame_yields_its_bbframe_estimate_and_iteration_count() {
    let codec = codec();
    let frames = noisy_frames(&codec, SIGMAS.len());
    let stalled: Vec<usize> = (0..frames.len())
        .filter(|&f| codec.decode_soft_counted(&frames[f]).is_err())
        .collect();
    assert!(!stalled.is_empty() && stalled.len() < frames.len());

    let expected = assert_batch_decodes_as_single_frame(&codec, &frames);
    for (f, result) in expected.iter().enumerate() {
        assert_eq!(result.converged, !stalled.contains(&f), "frame {f}");
        assert_eq!(result.decoded_bits.len(), codec.k_bch(), "frame {f}");
        if !result.converged {
            assert_eq!(result.iterations, CAP, "frame {f}");
        }
    }
}

#[test]
fn the_decoder_takes_the_configuration_and_cap_of_the_codec() {
    let mut codec = codec();
    codec.set_decoder_config(DecoderConfig::new(
        DecoderAlgorithm::NormalizedMinSum(0.75),
        false,
    ));
    codec.set_max_ldpc_iterations(3);
    let frames = noisy_frames(&codec, SIGMAS.len());
    let results = assert_batch_decodes_as_single_frame(&codec, &frames);
    assert!(results.iter().all(|result| result.iterations == 3));

    let default_rule = self::codec();
    assert!(frames
        .iter()
        .zip(&results)
        .any(|(llrs, result)| single_frame_result(&default_rule, llrs) != *result));
}

#[test]
fn the_default_constructor_reads_the_active_coding_section() {
    let decoder = DvbT2ConcatBatchDecoder::new(&codec());
    assert_eq!(
        decoder.ldpc().selectors(),
        tuning::active().section.decode()
    );
}

/// The normal-frame configurations of the codec's unit round trips.
#[test]
fn normal_frames_round_trip_through_the_batch_decoder() {
    for rate in [CodeRate::Rate1_2, CodeRate::Rate2_3, CodeRate::Rate3_4] {
        let codec = DvbT2Concat::new(FrameSize::Normal, rate).unwrap();
        let bbframes: Vec<BitVec> = (0..2)
            .map(|seed| BitVec::random_seeded(codec.k_bch(), SEED + seed))
            .collect();
        let frames: Vec<Vec<Llr>> = bbframes
            .iter()
            .map(|bbframe| noiseless_frame(&codec.encode(bbframe)))
            .collect();
        let results = assert_batch_decodes_as_single_frame(&codec, &frames);
        for (result, bbframe) in results.iter().zip(&bbframes) {
            assert!(result.converged, "{rate:?}");
            assert_eq!(result.decoded_bits, *bbframe, "{rate:?}");
        }
    }
}

fn decode_with_shapes(frame_len: usize, words: usize, outcomes: usize) {
    let mut decoder = DvbT2ConcatBatchDecoder::with_selectors(&codec(), selectors(1));
    let frame = vec![Llr::zero(); frame_len];
    decoder.decode_batch(
        &[&frame, &frame],
        &mut vec![BitVec::new(); words],
        &mut vec![DecodeOutcome::default(); outcomes],
    );
}

#[test]
#[should_panic(expected = "frame 0 has 16199 LLRs, the decoder takes 16200")]
fn a_frame_of_another_length_panics() {
    decode_with_shapes(16199, 2, 2);
}

#[test]
#[should_panic(expected = "a batch needs one decoded word per frame")]
fn a_missing_decoded_word_panics() {
    decode_with_shapes(16200, 1, 2);
}

#[test]
#[should_panic(expected = "a batch needs one outcome per frame")]
fn a_missing_outcome_panics() {
    decode_with_shapes(16200, 2, 3);
}

/// Each TP06 FECFRAME of the first frame decodes to its TP04 BBFRAME through
/// the batch decoder and through `decode_soft_counted`.
#[test]
#[ignore = "slow: reads the VV001-CR35 reference stream from $DVB_TEST_VECTORS_PATH"]
fn vv001_cr35_tp06_decodes_to_tp04() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }
    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");
    let tp04 = vectors.tp04.as_ref().expect("TP04 not found").frame(0);
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found").frame(0);
    assert_eq!(tp04.len(), tp06.len());

    let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate3_5).unwrap();
    let frames: Vec<Vec<Llr>> = tp06
        .iter()
        .map(|block| noiseless_frame(&block.data))
        .collect();
    let results = assert_batch_decodes_as_single_frame(&codec, &frames);
    for (block, (result, bbframe)) in results.iter().zip(tp04).enumerate() {
        assert!(result.converged, "block {block}");
        assert!(result.decoded_bits == bbframe.data, "block {block}");
    }
}
