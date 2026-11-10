//! The LDPC batch decoder's route reporter in a process that installs no
//! tuning profile, with its route decided by the decode selector family
//! alone. Codes of `@/citation/Etsi2015` widen the edge counts.

use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, EdgeLayout, LaneBackend,
    LdpcBatchDecoder, LdpcCode, LdpcDecoder, QcRouteBounds, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::test_support::{
    ldpc_batch_code, ldpc_dvb_t2_codes, ldpc_lane_backends, LDPC_BATCH_CAP,
};
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::tuning::{
    self, DecodeSelectors, GRANULES_PER_TASK, LANE_MAX_EDGES, LANE_MIN_FRAMES,
};
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
    std::iter::once(ldpc_batch_code())
        .chain(ldpc_dvb_t2_codes().map(|(_, _, code)| code))
        .collect()
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
fn without_installation_qc_codes_take_the_qc_route_and_others_the_single_frame_route() {
    for code in codes() {
        for algorithm in ALGORITHMS {
            let decoder = LdpcBatchDecoder::new(
                code.clone(),
                DecoderConfig::new(algorithm, true),
                LDPC_BATCH_CAP,
                DecodedWord::Codeword,
            );
            assert_eq!(decoder.selectors(), &DecodeSelectors::CONSERVATIVE);
            let expected = if code.quasi_cyclic().is_some()
                && !matches!(algorithm, DecoderAlgorithm::SumProduct)
            {
                BatchRoute::Qc(LaneBackend::detected())
            } else {
                BatchRoute::SingleFrame
            };
            for frames in batch_sizes() {
                assert_eq!(
                    decoder.batch_route(frames),
                    expected,
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
    let open = || {
        DecodeSelectors::try_new(0, usize::MAX, 1, 1)
            .unwrap()
            .with_qc_route(QcRouteBounds::CLOSED)
    };
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
            let selectors = DecodeSelectors::try_new(lane_min_frames, lane_max_edges, 1, 1)
                .unwrap()
                .with_qc_route(QcRouteBounds::CLOSED);
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

const MIN_SUM: [DecoderAlgorithm; 3] = [ALGORITHMS[0], ALGORITHMS[1], ALGORITHMS[2]];

/// Worker counts on both sides of the lane bound of [`lane_selectors`].
const WORKERS: [usize; 5] = [1, 2, 6, 12, 24];

/// Lanes open from two frames up to one worker, as in a calibrated profile.
fn lane_selectors() -> DecodeSelectors {
    DecodeSelectors::try_new(2, usize::MAX, 1, 1).unwrap()
}

/// Codes with retained QC structure: both 5G NR base graphs at lifting sizes
/// below one AVX2 vector, with a remainder modulo eight, and the largest, and
/// every DVB-T2 code.
fn qc_codes() -> Vec<LdpcCode> {
    let codes: Vec<LdpcCode> = [(1, 384), (2, 384), (1, 13), (2, 6)]
        .into_iter()
        .map(|(graph, z)| LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(graph, z)))
        .chain(std::iter::once(ldpc_batch_code()))
        .chain(ldpc_dvb_t2_codes().map(|(_, _, code)| code))
        .collect();
    assert!(codes.iter().all(|code| code.quasi_cyclic().is_some()));
    codes
}

fn workers(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).unwrap()
}

/// A decoder of `code` under `selectors` with the QC route closed.
fn without_qc(
    code: &LdpcCode,
    algorithm: DecoderAlgorithm,
    selectors: &DecodeSelectors,
) -> LdpcBatchDecoder {
    decoder(
        code,
        algorithm,
        selectors.clone().with_qc_route(QcRouteBounds::CLOSED),
    )
}

/// Without a QC override, a min-sum wave of a QC code takes the QC route
/// unless the eligible lane route claims it: a full wave at a lifting size of
/// at most the structural ceiling of the QC backend.
#[test]
fn every_min_sum_wave_of_a_qc_code_takes_the_qc_route_unless_lanes_claim_it() {
    let backend = LaneBackend::detected();
    let ceiling = QcRouteBounds::structural(backend).lane_claim_max_lifting_size();
    for code in qc_codes() {
        let z = code.quasi_cyclic().unwrap().expansion_factor();
        for algorithm in MIN_SUM {
            let decoder = decoder(&code, algorithm, lane_selectors());
            for count in WORKERS {
                for frames in batch_sizes() {
                    let claims = z <= ceiling && count == 1 && frames.min(LANES) == LANES;
                    let expected = if claims {
                        BatchRoute::Lane(backend)
                    } else {
                        BatchRoute::Qc(backend)
                    };
                    assert_eq!(
                        decoder.batch_route_with_workers(frames, workers(count)),
                        expected,
                        "Z={z}, {algorithm:?}, {count} workers, {frames} frames"
                    );
                }
            }
            assert_eq!(
                decoder.with_portable_lanes().batch_route(1),
                BatchRoute::Qc(LaneBackend::Portable)
            );
        }
        let sum_product = decoder(&code, DecoderAlgorithm::SumProduct, lane_selectors());
        for frames in batch_sizes() {
            assert_eq!(sum_product.batch_route(frames), BatchRoute::SingleFrame);
        }
    }

    let description = QuasiCyclicLdpc::nr_5g(1, 384);
    let unstructured = LdpcCode::from_edges(
        description.expanded_rows(),
        description.expanded_cols(),
        &description.to_edges(),
    );
    for algorithm in MIN_SUM {
        let open = decoder(&unstructured, algorithm, lane_selectors());
        let closed = without_qc(&unstructured, algorithm, &lane_selectors());
        for frames in batch_sizes() {
            assert_eq!(open.batch_route(frames), closed.batch_route(frames));
        }
    }
}

/// Each QC bound is set on either side of a code's lifting size, a worker
/// count or a wave size, and the region beyond it takes the route without QC.
#[test]
fn each_qc_bound_moves_its_region_to_the_route_without_qc() {
    let qc = BatchRoute::Qc(LaneBackend::detected());
    for code in qc_codes() {
        let z = code.quasi_cyclic().unwrap().expansion_factor();
        let cases = [
            (
                QcRouteBounds::new(z, NonZeroUsize::MAX, usize::MAX, usize::MAX),
                "at Z",
            ),
            (
                QcRouteBounds::new(z + 1, NonZeroUsize::MAX, usize::MAX, usize::MAX),
                "above Z",
            ),
            (
                QcRouteBounds::new(0, NonZeroUsize::new(6).unwrap(), usize::MAX, usize::MAX),
                "six workers",
            ),
            (
                QcRouteBounds::new(0, NonZeroUsize::MAX, 4, usize::MAX),
                "lanes from four",
            ),
        ];
        for (bounds, name) in cases {
            for algorithm in MIN_SUM {
                let selectors = lane_selectors().with_qc_route(bounds);
                let decoder = decoder(&code, algorithm, selectors.clone());
                let reference = without_qc(&code, algorithm, &selectors);
                for count in WORKERS {
                    for frames in batch_sizes() {
                        let wave = frames.min(LANES);
                        let lane_eligible = count == 1 && wave >= 2;
                        let on_qc = z >= bounds.min_lifting_size()
                            && count <= bounds.max_workers().get()
                            && !(lane_eligible && wave >= bounds.lane_claim_min_frames());
                        let expected = if on_qc {
                            qc
                        } else {
                            reference.batch_route_with_workers(frames, workers(count))
                        };
                        assert_eq!(
                            decoder.batch_route_with_workers(frames, workers(count)),
                            expected,
                            "Z={z}, {name}, {algorithm:?}, {count} workers, {frames} frames"
                        );
                    }
                }
            }
        }
    }
}

fn nr_bg1_z384() -> LdpcCode {
    LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384))
}

#[test]
fn every_wave_of_a_longer_batch_takes_the_qc_route() {
    let frame = vec![Llr::new(2.0); nr_bg1_z384().n()];
    let mut decoder = decoder(
        &nr_bg1_z384(),
        DecoderAlgorithm::NormalizedMinSum(0.75),
        lane_selectors().with_qc_route(QcRouteBounds::new(0, NonZeroUsize::MAX, 4, usize::MAX)),
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
    let qc = Some(BatchRoute::Qc(LaneBackend::detected()));
    assert_eq!(decode(1), qc);
    assert_eq!(
        decode(LANES),
        Some(BatchRoute::Lane(LaneBackend::detected()))
    );
    assert_eq!(decode(LANES + 1), qc);
    assert_eq!(decode(LANES + 3), qc);
    assert_eq!(
        decode(LANES + 4),
        Some(BatchRoute::Lane(LaneBackend::detected()))
    );
}

/// QC waves at one and twelve workers, and the final wave of a longer batch,
/// equal the single-frame decoder under every min-sum rule.
#[test]
fn qc_waves_decode_as_the_single_frame_decoder() {
    let code = nr_bg1_z384();
    let frames: Vec<Vec<Llr>> = (0..LANES + 1)
        .map(|seed| {
            (0..code.n())
                .map(|i| Llr::new(if (i + seed) % 97 == 0 { -1.0 } else { 2.0 }))
                .collect()
        })
        .collect();
    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
    for algorithm in MIN_SUM {
        let config = DecoderConfig::new(algorithm, true);
        let mut single = LdpcDecoder::with_config(code.clone(), config);
        for backend in ldpc_lane_backends() {
            for (count, frames) in [(1, 1), (12, LANES), (12, LANES + 1)] {
                let mut decoder = LdpcBatchDecoder::with_selectors(
                    code.clone(),
                    config,
                    3,
                    DecodedWord::Codeword,
                    lane_selectors(),
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
                    assert_eq!(
                        decoded[f], expected.decoded_bits,
                        "frame {f}, {algorithm:?}"
                    );
                    assert_eq!(outcomes[f], expected.outcome(), "frame {f}, {algorithm:?}");
                }
            }
        }
    }
}
