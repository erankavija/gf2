//! Route decision of LDPC batch decoding.

use std::num::NonZeroUsize;

use super::DecoderAlgorithm;
use crate::tuning::DecodeSelectors;
use gf2_kernels_simd::ldpc_lanes::LdpcLaneFns;

/// A backend of the lane kernels. Every backend writes the bit patterns of
/// [`LdpcLaneFns::portable`].
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

/// Route that decodes one wave of frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRoute {
    /// Each frame decodes on the single-frame decoder.
    SingleFrame,
    /// The wave decodes in eight lanes, one frame per lane, in the kernels of
    /// the named backend.
    Lane(LaneBackend),
}

/// Decides the route of one wave of frames.
///
/// The lane route on the backend of `capability` is taken when all of the
/// following hold, and the single-frame route otherwise:
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
    occupied_lanes: usize,
    edges: usize,
    workers: NonZeroUsize,
) -> BatchRoute {
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
            let route = decide_batch_route(&s, algorithm, PORTABLE, 8, 100, NonZeroUsize::MIN);
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
}
