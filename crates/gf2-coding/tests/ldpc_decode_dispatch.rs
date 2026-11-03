//! The LDPC batch decoder's route reporter in a process that installs no
//! tuning profile, with its route decided by the decode selector family
//! alone. Codes of `@/citation/Etsi2015` widen the edge counts.

use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, EdgeLayout, LaneBackend,
    LdpcBatchDecoder, LdpcCode, LdpcDecoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::test_support::{ldpc_batch_code, ldpc_lane_backends, LDPC_BATCH_CAP};
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::tuning::{
    self, DecodeSelectors, QcRouteCase, GRANULES_PER_TASK, LANE_MAX_EDGES, LANE_MIN_FRAMES,
};
use gf2_coding::CodeRate;
use gf2_core::tuning::SectionResolution;
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_lanes::LANES;
use std::num::NonZeroUsize;

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
    let open = || DecodeSelectors::try_new(0, usize::MAX, 1, 1).unwrap();
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
        DecodeSelectors::try_new(LANE_MIN_FRAMES, LANE_MAX_EDGES, 1, GRANULES_PER_TASK.get())
            .unwrap()
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
            let selectors =
                DecodeSelectors::try_new(lane_min_frames, lane_max_edges, 1, 1).unwrap();
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

/// The selection of `qc-go-rule` on NR BG1 Z=384 with lanes open up to one
/// worker from two frames.
fn qc_selectors() -> DecodeSelectors {
    let case = |workers, frames| QcRouteCase::try_new(1, 384, workers, frames).unwrap();
    DecodeSelectors::try_new(2, usize::MAX, 1, 1)
        .unwrap()
        .with_qc_route_cases(vec![case(1, 1), case(12, 8)])
}

fn nr_bg1_z384() -> LdpcCode {
    LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384))
}

fn workers(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).unwrap()
}

#[test]
fn the_decoder_selects_the_qc_route_on_the_selected_structure_and_cases_only() {
    let code = nr_bg1_z384();
    let nms = DecoderAlgorithm::NormalizedMinSum(0.75);
    let selected = decoder(&code, nms, qc_selectors());
    let qc = BatchRoute::Qc(LaneBackend::detected());
    let lane = BatchRoute::Lane(LaneBackend::detected());
    assert_eq!(selected.batch_route(1), qc);
    assert_eq!(selected.batch_route_with_workers(LANES, workers(12)), qc);
    assert_eq!(selected.batch_route(LANES), lane);
    assert_eq!(
        selected.batch_route_with_workers(1, workers(12)),
        BatchRoute::SingleFrame
    );
    // A longer batch is no case of the selection, whatever its wave sizes.
    assert_eq!(selected.batch_route(LANES + 1), lane);
    assert_eq!(
        selected.batch_route_with_workers(2 * LANES, workers(12)),
        BatchRoute::SingleFrame
    );
    assert_eq!(
        selected.with_portable_lanes().batch_route(1),
        BatchRoute::Qc(LaneBackend::Portable)
    );

    for algorithm in [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::OffsetMinSum(0.5),
        DecoderAlgorithm::SumProduct,
    ] {
        let other = decoder(&code, algorithm, qc_selectors());
        assert_eq!(
            other.batch_route(1),
            BatchRoute::SingleFrame,
            "{algorithm:?}"
        );
        assert_eq!(
            other.batch_route_with_workers(LANES, workers(12)),
            BatchRoute::SingleFrame,
            "{algorithm:?}"
        );
    }

    let qc_description = QuasiCyclicLdpc::nr_5g(1, 384);
    let unstructured = LdpcCode::from_edges(
        qc_description.expanded_rows(),
        qc_description.expanded_cols(),
        &qc_description.to_edges(),
    );
    let closed = [
        decoder(&unstructured, nms, qc_selectors()),
        decoder(&code, nms, DecodeSelectors::CONSERVATIVE),
        decoder(
            &LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 352)),
            nms,
            qc_selectors(),
        ),
    ];
    for decoder in closed {
        assert_eq!(decoder.batch_route(1), BatchRoute::SingleFrame);
        assert_eq!(
            decoder.batch_route_with_workers(LANES, workers(12)),
            BatchRoute::SingleFrame
        );
    }
}

#[test]
fn a_longer_batch_runs_no_wave_on_the_qc_route() {
    let frame = vec![Llr::new(2.0); nr_bg1_z384().n()];
    let mut decoder = decoder(
        &nr_bg1_z384(),
        DecoderAlgorithm::NormalizedMinSum(0.75),
        qc_selectors(),
    );
    let mut decode = |frames: usize| {
        let views = vec![frame.as_slice(); frames];
        decoder.decode_batch(
            &views,
            &mut vec![BitVec::new(); frames],
            &mut vec![DecodeOutcome::default(); frames],
        );
        decoder.last_wave_route()
    };
    assert_eq!(decode(1), Some(BatchRoute::Qc(LaneBackend::detected())));
    assert_eq!(decode(LANES + 1), Some(BatchRoute::SingleFrame));
}

/// Each decode call on the selected cases equals the single-frame decoder at
/// the frozen structure, on both message and codeword words.
#[test]
fn the_selected_cases_decode_as_the_single_frame_decoder() {
    let code = nr_bg1_z384();
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let frames: Vec<Vec<Llr>> = (0..LANES)
        .map(|seed| {
            (0..code.n())
                .map(|i| Llr::new(if (i + seed) % 97 == 0 { -1.0 } else { 2.0 }))
                .collect()
        })
        .collect();
    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
    let mut single = LdpcDecoder::with_config(code.clone(), config);
    for backend in ldpc_lane_backends() {
        for (count, frames) in [(1, 1), (12, LANES)] {
            let mut decoder = LdpcBatchDecoder::with_selectors(
                code.clone(),
                config,
                3,
                DecodedWord::Codeword,
                qc_selectors(),
            );
            if backend == LaneBackend::Portable {
                decoder = decoder.with_portable_lanes();
            }
            let mut decoded = vec![BitVec::new(); frames];
            let mut outcomes = vec![DecodeOutcome::default(); frames];
            decoder.decode_batch_with_workers(
                &views[..frames],
                &mut decoded,
                &mut outcomes,
                workers(count),
            );
            assert_eq!(decoder.last_wave_route(), Some(BatchRoute::Qc(backend)));
            for (f, view) in views[..frames].iter().enumerate() {
                let expected = single.decode_to_codeword(view, 3);
                assert_eq!(decoded[f], expected.decoded_bits, "frame {f}");
                assert_eq!(outcomes[f], expected.outcome(), "frame {f}");
            }
        }
    }
}
