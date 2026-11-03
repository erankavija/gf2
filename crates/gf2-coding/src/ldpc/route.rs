//! Route decision of LDPC batch decoding.

use std::num::NonZeroUsize;

use super::DecoderAlgorithm;
use crate::tuning::DecodeSelectors;
use gf2_kernels_simd::ldpc_lanes::LdpcLaneFns;

/// A backend of the lane and QC kernels. Every backend writes the bit patterns
/// of the portable kernels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneBackend {
    /// The safe scalar kernels, available in every build and on every host.
    Portable,
    /// The AVX2 kernels.
    Avx2,
}

impl LaneBackend {
    /// The backend of this build and host: [`Self::Avx2`] when `gf2-coding`
    /// is built with its `simd` cargo feature and AVX2 is detected at run
    /// time, [`Self::Portable`] otherwise. Detection runs once per process.
    #[must_use]
    pub fn detected() -> Self {
        detected_lanes().0
    }
}

/// [`LaneBackend::detected`] with its kernels.
pub(super) fn detected_lanes() -> (LaneBackend, LdpcLaneFns) {
    #[cfg(feature = "simd")]
    {
        use std::sync::OnceLock;
        static AVX2: OnceLock<Option<LdpcLaneFns>> = OnceLock::new();
        if let Some(fns) = *AVX2.get_or_init(gf2_kernels_simd::ldpc_lanes::detect) {
            return (LaneBackend::Avx2, fns);
        }
    }
    (LaneBackend::Portable, LdpcLaneFns::portable())
}

/// Availability of a lane kernel backend for the lane route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneCapability {
    /// The named lane kernel backend is available.
    Present(LaneBackend),
    /// No lane kernel backend is available.
    Absent,
}

/// The QC structure and kernel backend of a decoder, for the QC route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QcCapability {
    /// The code carries the 5G NR structure of `base_graph` and
    /// `lifting_size`, and the kernels of `backend` decode it.
    Present {
        /// The 5G NR base graph.
        base_graph: u8,
        /// The 5G NR lifting size.
        lifting_size: usize,
        /// The backend of the QC kernels.
        backend: LaneBackend,
    },
    /// The QC route is unavailable to this decode.
    Absent,
}

/// Route that decodes one wave of frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRoute {
    /// Each frame decodes on the single-frame decoder.
    SingleFrame,
    /// The wave decodes in eight lanes, one frame per lane, in the kernels of
    /// the named backend.
    Lane(LaneBackend),
    /// Each frame decodes on [`QcDecoder`](super::QcDecoder), in the kernels of
    /// the named backend.
    Qc(LaneBackend),
}

/// Decides the route of one wave of frames.
///
/// The QC route on the backend of `qc` is taken when `algorithm` is
/// [`DecoderAlgorithm::NormalizedMinSum`], `qc` is [`QcCapability::Present`]
/// and a [`QcRouteCase`](crate::tuning::QcRouteCase) of `selectors` names its
/// structure, `workers` and `occupied_lanes` exactly.
///
/// Otherwise the lane route on the backend of `capability` is taken when all of
/// the following hold, and the single-frame route otherwise:
///
/// - `algorithm` is of the min-sum family: [`DecoderAlgorithm::MinSum`],
///   [`DecoderAlgorithm::NormalizedMinSum`] or
///   [`DecoderAlgorithm::OffsetMinSum`];
/// - `capability` is [`LaneCapability::Present`];
/// - `occupied_lanes >= selectors.lane_min_frames()`;
/// - `edges <= selectors.lane_max_edges()`;
/// - `workers <= selectors.lane_max_workers()`.
///
/// `occupied_lanes` counts the frames of the wave and `edges` the
/// Tanner-graph edges of the code. `workers` is the configured concurrent
/// decode worker count, independent of how many are presently scheduled. The decision reads the decode selector
/// family only; the caller resolves the family from the coding tuning
/// section.
#[must_use]
pub fn decide_batch_route(
    selectors: &DecodeSelectors,
    algorithm: DecoderAlgorithm,
    capability: LaneCapability,
    qc: QcCapability,
    occupied_lanes: usize,
    edges: usize,
    workers: NonZeroUsize,
) -> BatchRoute {
    if let QcCapability::Present {
        base_graph,
        lifting_size,
        backend,
    } = qc
    {
        let selected = matches!(algorithm, DecoderAlgorithm::NormalizedMinSum(_))
            && selectors.qc_route_cases().iter().any(|case| {
                case.base_graph() == base_graph
                    && case.lifting_size() == lifting_size
                    && case.workers() == workers
                    && case.frames().get() == occupied_lanes
            });
        if selected {
            return BatchRoute::Qc(backend);
        }
    }
    let min_sum_family = !matches!(algorithm, DecoderAlgorithm::SumProduct);
    match capability {
        LaneCapability::Present(backend)
            if min_sum_family
                && occupied_lanes >= selectors.lane_min_frames()
                && edges <= selectors.lane_max_edges()
                && workers <= selectors.lane_max_workers() =>
        {
            BatchRoute::Lane(backend)
        }
        _ => BatchRoute::SingleFrame,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::QcRouteCase;

    const MIN_SUM_FAMILY: [DecoderAlgorithm; 3] = [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::OffsetMinSum(0.5),
    ];

    fn selectors(min_frames: usize, max_edges: usize) -> DecodeSelectors {
        DecodeSelectors::try_new(min_frames, max_edges, 1, 1).unwrap()
    }

    const PORTABLE: LaneCapability = LaneCapability::Present(LaneBackend::Portable);
    const LANE: BatchRoute = BatchRoute::Lane(LaneBackend::Portable);

    fn decide(s: &DecodeSelectors, lanes: usize, edges: usize) -> BatchRoute {
        decide_batch_route(
            s,
            DecoderAlgorithm::MinSum,
            PORTABLE,
            QcCapability::Absent,
            lanes,
            edges,
            NonZeroUsize::MIN,
        )
    }

    #[test]
    fn worker_limit_bounds_configured_decode_concurrency() {
        let s = DecodeSelectors::try_new(1, 1000, 2, 1).unwrap();
        for workers in [1, 2, 3, 6, 12, 24] {
            let got = decide_batch_route(
                &s,
                DecoderAlgorithm::MinSum,
                PORTABLE,
                QcCapability::Absent,
                8,
                100,
                std::num::NonZeroUsize::new(workers).unwrap(),
            );
            assert_eq!(
                got,
                if workers <= 2 {
                    LANE
                } else {
                    BatchRoute::SingleFrame
                }
            );
        }
    }

    #[test]
    fn lane_min_frames_bounds_occupied_lanes_from_below() {
        let s = selectors(4, 1000);
        assert_eq!(decide(&s, 3, 1000), BatchRoute::SingleFrame);
        assert_eq!(decide(&s, 4, 1000), LANE);
        assert_eq!(decide(&s, 8, 1000), LANE);
    }

    #[test]
    fn lane_max_edges_bounds_edge_count_from_above() {
        let s = selectors(1, 1000);
        assert_eq!(decide(&s, 8, 999), LANE);
        assert_eq!(decide(&s, 8, 1000), LANE);
        assert_eq!(decide(&s, 8, 1001), BatchRoute::SingleFrame);
    }

    #[test]
    fn min_sum_family_takes_the_lane_route() {
        let s = selectors(1, 1000);
        for algorithm in MIN_SUM_FAMILY {
            let route = decide_batch_route(
                &s,
                algorithm,
                PORTABLE,
                QcCapability::Absent,
                8,
                100,
                NonZeroUsize::MIN,
            );
            assert_eq!(route, LANE, "{algorithm:?}");
        }
    }

    #[test]
    fn the_lane_route_names_the_backend_of_the_capability() {
        let s = selectors(1, 1000);
        for backend in [LaneBackend::Portable, LaneBackend::Avx2] {
            let capability = LaneCapability::Present(backend);
            let route = decide_batch_route(
                &s,
                DecoderAlgorithm::MinSum,
                capability,
                QcCapability::Absent,
                8,
                100,
                NonZeroUsize::MIN,
            );
            assert_eq!(route, BatchRoute::Lane(backend));
        }
    }

    #[test]
    fn the_detected_backend_follows_the_simd_feature_and_the_host() {
        let avx2 = cfg!(feature = "simd") && gf2_kernels_simd::ldpc_lanes::detect().is_some();
        let expected = if avx2 {
            LaneBackend::Avx2
        } else {
            LaneBackend::Portable
        };
        assert_eq!(LaneBackend::detected(), expected);
    }

    #[test]
    fn sum_product_is_single_frame() {
        let s = selectors(0, usize::MAX);
        let route = decide_batch_route(
            &s,
            DecoderAlgorithm::SumProduct,
            PORTABLE,
            QcCapability::Absent,
            8,
            0,
            NonZeroUsize::MIN,
        );
        assert_eq!(route, BatchRoute::SingleFrame);
    }

    #[test]
    fn absent_capability_is_single_frame() {
        let s = selectors(0, usize::MAX);
        for algorithm in MIN_SUM_FAMILY {
            let route = decide_batch_route(
                &s,
                algorithm,
                LaneCapability::Absent,
                QcCapability::Absent,
                8,
                0,
                NonZeroUsize::MIN,
            );
            assert_eq!(route, BatchRoute::SingleFrame, "{algorithm:?}");
        }
    }

    #[test]
    fn conservative_family_closes_the_lane_route() {
        let s = DecodeSelectors::CONSERVATIVE;
        for lanes in 0..=8 {
            for edges in [0, 1, 1000, usize::MAX] {
                assert_eq!(decide(&s, lanes, edges), BatchRoute::SingleFrame);
            }
        }
    }

    /// The selection predicate of `qc-go-rule` in
    /// `dev/active/55c3e433-aff3ct-ldpc-gaps/plan.md`, with lanes open from
    /// two frames for one worker.
    fn qc_selectors() -> DecodeSelectors {
        let case = |workers, frames| QcRouteCase::try_new(1, 384, workers, frames).unwrap();
        DecodeSelectors::try_new(2, usize::MAX, 1, 1)
            .unwrap()
            .with_qc_route_cases(vec![case(1, 1), case(12, 8)])
    }

    fn structure(base_graph: u8, lifting_size: usize) -> QcCapability {
        QcCapability::Present {
            base_graph,
            lifting_size,
            backend: LaneBackend::Avx2,
        }
    }

    /// The route without QC capability, the existing route of a case.
    fn existing(algorithm: DecoderAlgorithm, frames: usize, workers: NonZeroUsize) -> BatchRoute {
        decide_batch_route(
            &qc_selectors(),
            algorithm,
            PORTABLE,
            QcCapability::Absent,
            frames,
            100,
            workers,
        )
    }

    #[test]
    fn the_qc_route_takes_exactly_the_selected_cases_and_others_keep_their_route() {
        let algorithm = DecoderAlgorithm::NormalizedMinSum(0.75);
        for workers in [1, 2, 6, 11, 12, 13, 24] {
            let workers = NonZeroUsize::new(workers).unwrap();
            for frames in 0..=8 {
                let got = decide_batch_route(
                    &qc_selectors(),
                    algorithm,
                    PORTABLE,
                    structure(1, 384),
                    frames,
                    100,
                    workers,
                );
                let selected = matches!((workers.get(), frames), (1, 1) | (12, 8));
                let expected = if selected {
                    BatchRoute::Qc(LaneBackend::Avx2)
                } else {
                    existing(algorithm, frames, workers)
                };
                assert_eq!(got, expected, "{workers} workers, {frames} frames");
            }
        }
        assert_eq!(existing(algorithm, 8, NonZeroUsize::MIN), LANE);
    }

    #[test]
    fn every_other_algorithm_keeps_its_route_on_the_selected_cases() {
        let twelve = NonZeroUsize::new(12).unwrap();
        for algorithm in [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::OffsetMinSum(0.5),
            DecoderAlgorithm::SumProduct,
        ] {
            for (workers, frames) in [(NonZeroUsize::MIN, 1), (twelve, 8)] {
                let got = decide_batch_route(
                    &qc_selectors(),
                    algorithm,
                    PORTABLE,
                    structure(1, 384),
                    frames,
                    100,
                    workers,
                );
                assert_eq!(got, existing(algorithm, frames, workers), "{algorithm:?}");
            }
        }
    }

    #[test]
    fn another_structure_or_no_case_keeps_the_existing_route() {
        let algorithm = DecoderAlgorithm::NormalizedMinSum(0.75);
        let decide = |selectors: &DecodeSelectors, qc| {
            decide_batch_route(
                selectors,
                algorithm,
                PORTABLE,
                qc,
                1,
                100,
                NonZeroUsize::MIN,
            )
        };
        for qc in [structure(2, 384), structure(1, 352), QcCapability::Absent] {
            assert_eq!(decide(&qc_selectors(), qc), BatchRoute::SingleFrame);
        }
        assert_eq!(
            decide(&DecodeSelectors::CONSERVATIVE, structure(1, 384)),
            BatchRoute::SingleFrame
        );
    }

    #[test]
    fn the_qc_route_names_the_backend_of_the_capability() {
        let got = decide_batch_route(
            &qc_selectors(),
            DecoderAlgorithm::NormalizedMinSum(0.75),
            LaneCapability::Absent,
            QcCapability::Present {
                base_graph: 1,
                lifting_size: 384,
                backend: LaneBackend::Portable,
            },
            1,
            100,
            NonZeroUsize::MIN,
        );
        assert_eq!(got, BatchRoute::Qc(LaneBackend::Portable));
    }
}
