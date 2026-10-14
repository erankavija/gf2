//! The LDPC batch decoder's route reporter in a process that installs no
//! tuning profile, with its route decided by the decode selector family
//! alone. Codes of `@/citation/Etsi2015` widen the edge counts.

use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, EdgeLayout, LaneBackend,
    LdpcBatchDecoder, LdpcCode,
};
use gf2_coding::test_support::{ldpc_batch_code, LDPC_BATCH_CAP};
use gf2_coding::tuning::{
    self, DecodeSelectors, GRANULES_PER_TASK, LANE_MAX_EDGES, LANE_MIN_FRAMES,
};
use gf2_coding::CodeRate;
use gf2_core::tuning::SectionResolution;
use gf2_kernels_simd::ldpc_lanes::LANES;

const ALGORITHMS: [DecoderAlgorithm; 4] = [
    DecoderAlgorithm::MinSum,
    DecoderAlgorithm::NormalizedMinSum(0.75),
    DecoderAlgorithm::OffsetMinSum(0.5),
    DecoderAlgorithm::SumProduct,
];

/// Every batch size up to two full waves and one frame of a third.
fn batch_sizes() -> std::ops::RangeInclusive<usize> {
    0..=2 * LANES + 1
}

fn codes() -> Vec<LdpcCode> {
    vec![
        ldpc_batch_code(),
        LdpcCode::dvb_t2_short(CodeRate::Rate1_2),
        LdpcCode::dvb_t2_normal(CodeRate::Rate1_2),
    ]
}

fn edges(code: &LdpcCode) -> usize {
    EdgeLayout::from_parity_check(code.parity_check_matrix()).edges()
}

fn decoder(
    code: &LdpcCode,
    algorithm: DecoderAlgorithm,
    selectors: DecodeSelectors,
) -> LdpcBatchDecoder {
    LdpcBatchDecoder::with_selectors(
        code.clone(),
        DecoderConfig::new(algorithm, true),
        LDPC_BATCH_CAP,
        DecodedWord::Codeword,
        selectors,
    )
}

#[test]
fn without_installation_every_batch_size_takes_the_single_frame_route() {
    for code in codes() {
        for algorithm in ALGORITHMS {
            let decoder = LdpcBatchDecoder::new(
                code.clone(),
                DecoderConfig::new(algorithm, true),
                LDPC_BATCH_CAP,
                DecodedWord::Codeword,
            );
            assert_eq!(decoder.selectors(), &DecodeSelectors::CONSERVATIVE);
            for frames in batch_sizes() {
                assert_eq!(
                    decoder.batch_route(frames),
                    BatchRoute::SingleFrame,
                    "{algorithm:?}, {frames} frames"
                );
            }
        }
    }
    assert!(matches!(
        tuning::active().resolution,
        SectionResolution::FrozenBeforeInstall { .. }
    ));
}

#[test]
fn without_the_lane_capability_the_lane_route_names_the_portable_backend() {
    let capability = cfg!(feature = "simd") && gf2_kernels_simd::ldpc_lanes::detect().is_some();
    let detected = if capability {
        LaneBackend::Avx2
    } else {
        LaneBackend::Portable
    };
    let open = || DecodeSelectors::try_new(0, usize::MAX, 1).unwrap();
    for code in codes() {
        for algorithm in &ALGORITHMS[..3] {
            let decoder = decoder(&code, *algorithm, open());
            for frames in batch_sizes() {
                assert_eq!(decoder.batch_route(frames), BatchRoute::Lane(detected));
            }
            let portable = decoder.with_portable_lanes();
            for frames in batch_sizes() {
                assert_eq!(
                    portable.batch_route(frames),
                    BatchRoute::Lane(LaneBackend::Portable)
                );
            }
        }
    }
}

/// Each route selector is swept across either side of every wave size or of
/// each code's edge count with the other selector open, and the reported
/// route is the one the two selectors give, so a route threshold outside the
/// family moves some cell.
#[test]
fn every_route_threshold_is_a_decode_selector() {
    assert_eq!(
        DecodeSelectors::CONSERVATIVE,
        DecodeSelectors::try_new(LANE_MIN_FRAMES, LANE_MAX_EDGES, GRANULES_PER_TASK.get()).unwrap()
    );
    let backend = LaneBackend::detected();
    for code in codes() {
        let edges = edges(&code);
        let by_frames = (0..=LANES + 1)
            .chain([LANE_MIN_FRAMES])
            .map(|lane_min_frames| (lane_min_frames, usize::MAX));
        let by_edges =
            [LANE_MAX_EDGES, edges - 1, edges, edges + 1].map(|lane_max_edges| (0, lane_max_edges));
        for (lane_min_frames, lane_max_edges) in by_frames.chain(by_edges) {
            let selectors = DecodeSelectors::try_new(lane_min_frames, lane_max_edges, 1).unwrap();
            for algorithm in ALGORITHMS {
                let decoder = decoder(&code, algorithm, selectors.clone());
                for frames in batch_sizes() {
                    let lane = !matches!(algorithm, DecoderAlgorithm::SumProduct)
                        && frames.min(LANES) >= lane_min_frames
                        && edges <= lane_max_edges;
                    let expected = if lane {
                        BatchRoute::Lane(backend)
                    } else {
                        BatchRoute::SingleFrame
                    };
                    assert_eq!(
                        decoder.batch_route(frames),
                        expected,
                        "{algorithm:?}, {selectors:?}, {frames} frames, {edges} edges"
                    );
                }
            }
        }
    }
}
