//! Route decision of LDPC batch decoding.

use super::DecoderAlgorithm;
use crate::tuning::DecodeSelectors;

/// Availability of a lane kernel backend for the lane route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneCapability {
    /// A lane kernel backend is available.
    Present,
    /// No lane kernel backend is available.
    Absent,
}

/// Route that decodes one wave of frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRoute {
    /// Each frame decodes on the single-frame decoder.
    SingleFrame,
    /// The wave decodes in eight lanes, one frame per lane.
    Lane,
}

/// Decides the route of one wave of frames.
///
/// The lane route is taken when all of the following hold, and the
/// single-frame route otherwise:
///
/// - `algorithm` is of the min-sum family: [`DecoderAlgorithm::MinSum`],
///   [`DecoderAlgorithm::NormalizedMinSum`] or
///   [`DecoderAlgorithm::OffsetMinSum`];
/// - `capability` is [`LaneCapability::Present`];
/// - `occupied_lanes >= selectors.lane_min_frames()`;
/// - `edges <= selectors.lane_max_edges()`.
///
/// `occupied_lanes` counts the frames of the wave and `edges` the
/// Tanner-graph edges of the code. The decision reads the decode selector
/// family only; the caller resolves the family from the coding tuning
/// section.
#[must_use]
pub fn decide_batch_route(
    selectors: &DecodeSelectors,
    algorithm: DecoderAlgorithm,
    capability: LaneCapability,
    occupied_lanes: usize,
    edges: usize,
) -> BatchRoute {
    let min_sum_family = !matches!(algorithm, DecoderAlgorithm::SumProduct);
    if min_sum_family
        && capability == LaneCapability::Present
        && occupied_lanes >= selectors.lane_min_frames()
        && edges <= selectors.lane_max_edges()
    {
        BatchRoute::Lane
    } else {
        BatchRoute::SingleFrame
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
        DecodeSelectors::try_new(min_frames, max_edges, 1).unwrap()
    }

    fn decide(s: &DecodeSelectors, lanes: usize, edges: usize) -> BatchRoute {
        decide_batch_route(
            s,
            DecoderAlgorithm::MinSum,
            LaneCapability::Present,
            lanes,
            edges,
        )
    }

    #[test]
    fn lane_min_frames_bounds_occupied_lanes_from_below() {
        let s = selectors(4, 1000);
        assert_eq!(decide(&s, 3, 1000), BatchRoute::SingleFrame);
        assert_eq!(decide(&s, 4, 1000), BatchRoute::Lane);
        assert_eq!(decide(&s, 8, 1000), BatchRoute::Lane);
    }

    #[test]
    fn lane_max_edges_bounds_edge_count_from_above() {
        let s = selectors(1, 1000);
        assert_eq!(decide(&s, 8, 999), BatchRoute::Lane);
        assert_eq!(decide(&s, 8, 1000), BatchRoute::Lane);
        assert_eq!(decide(&s, 8, 1001), BatchRoute::SingleFrame);
    }

    #[test]
    fn min_sum_family_takes_the_lane_route() {
        let s = selectors(1, 1000);
        for algorithm in MIN_SUM_FAMILY {
            let route = decide_batch_route(&s, algorithm, LaneCapability::Present, 8, 100);
            assert_eq!(route, BatchRoute::Lane, "{algorithm:?}");
        }
    }

    #[test]
    fn sum_product_is_single_frame() {
        let s = selectors(0, usize::MAX);
        let route = decide_batch_route(
            &s,
            DecoderAlgorithm::SumProduct,
            LaneCapability::Present,
            8,
            0,
        );
        assert_eq!(route, BatchRoute::SingleFrame);
    }

    #[test]
    fn absent_capability_is_single_frame() {
        let s = selectors(0, usize::MAX);
        for algorithm in MIN_SUM_FAMILY {
            let route = decide_batch_route(&s, algorithm, LaneCapability::Absent, 8, 0);
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
