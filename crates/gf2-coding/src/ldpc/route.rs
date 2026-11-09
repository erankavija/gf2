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
    /// The code retains quasi-cyclic structure of lifting size
    /// `lifting_size`, and the QC kernels of `backend` decode it.
    Present {
        /// The lifting size $Z$ of the retained structure.
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
/// The lane route on the backend of `capability` is eligible when all of the
/// following hold:
///
/// - `algorithm` is of the min-sum family: [`DecoderAlgorithm::MinSum`],
///   [`DecoderAlgorithm::NormalizedMinSum`] or
///   [`DecoderAlgorithm::OffsetMinSum`];
/// - `capability` is [`LaneCapability::Present`];
/// - `occupied_lanes >= selectors.lane_min_frames()`;
/// - `edges <= selectors.lane_max_edges()`;
/// - `workers <= selectors.lane_max_workers()`.
///
/// With `bounds = selectors.qc_route()`, the QC route on the backend of `qc`
/// is taken when `algorithm` is of the min-sum family, `qc` is
/// [`QcCapability::Present`], its lifting size is at least
/// `bounds.min_lifting_size()`, `workers <= bounds.max_workers()`, and the
/// lane route is ineligible or `occupied_lanes <
/// bounds.lane_claim_min_frames()`. Otherwise an eligible lane route is
/// taken, and the single-frame route otherwise.
///
/// `occupied_lanes` counts the frames of the wave and `edges` the
/// Tanner-graph edges of the code. `workers` is the configured concurrent
/// decode worker count, independent of how many are presently scheduled. The
/// decision reads the decode selector family only; the caller resolves the
/// family from the coding tuning section.
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
    let min_sum_family = !matches!(algorithm, DecoderAlgorithm::SumProduct);
    let lane = match capability {
        LaneCapability::Present(backend)
            if min_sum_family
                && occupied_lanes >= selectors.lane_min_frames()
                && edges <= selectors.lane_max_edges()
                && workers <= selectors.lane_max_workers() =>
        {
            Some(backend)
        }
        _ => None,
    };
    if let QcCapability::Present {
        lifting_size,
        backend,
    } = qc
    {
        let bounds = selectors.qc_route();
        let lane_claims = lane.is_some() && occupied_lanes >= bounds.lane_claim_min_frames();
        if min_sum_family
            && lifting_size >= bounds.min_lifting_size()
            && workers <= bounds.max_workers()
            && !lane_claims
        {
            return BatchRoute::Qc(backend);
        }
    }
    lane.map_or(BatchRoute::SingleFrame, BatchRoute::Lane)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::QcRouteBounds;

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

    fn structure(lifting_size: usize) -> QcCapability {
        QcCapability::Present {
            lifting_size,
            backend: LaneBackend::Avx2,
        }
    }

    const QC: BatchRoute = BatchRoute::Qc(LaneBackend::Avx2);
    const WORKERS: [usize; 6] = [1, 2, 6, 12, 13, 24];

    fn decide_qc(
        s: &DecodeSelectors,
        algorithm: DecoderAlgorithm,
        qc: QcCapability,
        frames: usize,
        workers: usize,
    ) -> BatchRoute {
        decide_batch_route(
            s,
            algorithm,
            PORTABLE,
            qc,
            frames,
            100,
            NonZeroUsize::new(workers).unwrap(),
        )
    }

    /// The route of the same wave without QC capability.
    fn without_qc(
        s: &DecodeSelectors,
        algorithm: DecoderAlgorithm,
        frames: usize,
        workers: usize,
    ) -> BatchRoute {
        decide_qc(s, algorithm, QcCapability::Absent, frames, workers)
    }

    #[test]
    fn open_bounds_take_the_qc_route_for_every_min_sum_wave_of_a_qc_code() {
        let lanes_open = DecodeSelectors::try_new(1, usize::MAX, usize::MAX, 1).unwrap();
        for s in [DecodeSelectors::CONSERVATIVE, lanes_open] {
            assert_eq!(s.qc_route(), QcRouteBounds::OPEN);
            for algorithm in MIN_SUM_FAMILY {
                for lifting_size in [1, 2, 7, 8, 9, 360, 384] {
                    for workers in WORKERS {
                        for frames in 0..=8 {
                            let got =
                                decide_qc(&s, algorithm, structure(lifting_size), frames, workers);
                            assert_eq!(
                                got, QC,
                                "{algorithm:?} Z={lifting_size} {workers}w {frames}f"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn sum_product_and_absent_structure_keep_the_route_without_qc() {
        let s = DecodeSelectors::try_new(2, usize::MAX, 1, 1).unwrap();
        for workers in WORKERS {
            for frames in 0..=8 {
                let got = decide_qc(
                    &s,
                    DecoderAlgorithm::SumProduct,
                    structure(384),
                    frames,
                    workers,
                );
                assert_eq!(got, BatchRoute::SingleFrame);
                for algorithm in MIN_SUM_FAMILY {
                    assert_eq!(
                        decide_qc(&s, algorithm, QcCapability::Absent, frames, workers),
                        without_qc(&s, algorithm, frames, workers)
                    );
                }
            }
        }
    }

    #[test]
    fn min_lifting_size_bounds_the_qc_route_from_below() {
        let bounds = QcRouteBounds::try_new(8, usize::MAX, usize::MAX).unwrap();
        let s = DecodeSelectors::try_new(2, usize::MAX, 1, 1)
            .unwrap()
            .with_qc_route(bounds);
        for algorithm in MIN_SUM_FAMILY {
            for workers in WORKERS {
                for frames in 0..=8 {
                    for lifting_size in [1, 7, 8, 9, 384] {
                        let got =
                            decide_qc(&s, algorithm, structure(lifting_size), frames, workers);
                        let expected = if lifting_size >= 8 {
                            QC
                        } else {
                            without_qc(&s, algorithm, frames, workers)
                        };
                        assert_eq!(got, expected, "Z={lifting_size} {workers}w {frames}f");
                    }
                }
            }
        }
    }

    #[test]
    fn max_workers_bounds_the_qc_route_from_above() {
        let bounds = QcRouteBounds::try_new(0, 12, usize::MAX).unwrap();
        let s = DecodeSelectors::try_new(2, usize::MAX, 1, 1)
            .unwrap()
            .with_qc_route(bounds);
        for algorithm in MIN_SUM_FAMILY {
            for workers in WORKERS {
                for frames in 0..=8 {
                    let got = decide_qc(&s, algorithm, structure(384), frames, workers);
                    let expected = if workers <= 12 {
                        QC
                    } else {
                        without_qc(&s, algorithm, frames, workers)
                    };
                    assert_eq!(got, expected, "{workers}w {frames}f");
                }
            }
        }
    }

    #[test]
    fn an_eligible_lane_route_claims_waves_from_its_bound() {
        let bounds = QcRouteBounds::try_new(0, usize::MAX, 4).unwrap();
        // Lanes are eligible from two frames at up to six workers.
        let s = DecodeSelectors::try_new(2, usize::MAX, 6, 1)
            .unwrap()
            .with_qc_route(bounds);
        for algorithm in MIN_SUM_FAMILY {
            for workers in WORKERS {
                for frames in 0..=8 {
                    let got = decide_qc(&s, algorithm, structure(384), frames, workers);
                    let expected = if workers <= 6 && frames >= 4 {
                        LANE
                    } else {
                        QC
                    };
                    assert_eq!(got, expected, "{workers}w {frames}f");
                }
            }
        }
    }

    #[test]
    fn closed_bounds_keep_the_route_without_qc() {
        let s = DecodeSelectors::try_new(2, usize::MAX, 1, 1)
            .unwrap()
            .with_qc_route(QcRouteBounds::CLOSED);
        for algorithm in MIN_SUM_FAMILY {
            for workers in WORKERS {
                for frames in 0..=8 {
                    for lifting_size in [1, 384, usize::MAX - 1] {
                        assert_eq!(
                            decide_qc(&s, algorithm, structure(lifting_size), frames, workers),
                            without_qc(&s, algorithm, frames, workers)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_qc_route_names_the_backend_of_the_capability() {
        for backend in [LaneBackend::Portable, LaneBackend::Avx2] {
            let got = decide_batch_route(
                &DecodeSelectors::CONSERVATIVE,
                DecoderAlgorithm::NormalizedMinSum(0.75),
                LaneCapability::Absent,
                QcCapability::Present {
                    lifting_size: 384,
                    backend,
                },
                1,
                100,
                NonZeroUsize::MIN,
            );
            assert_eq!(got, BatchRoute::Qc(backend));
        }
    }
}
