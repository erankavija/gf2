//! F_3 wave-cooperative Ryser control.
//!
//! The device implementation in `hip/wave_gf3_equivalence.hip` gives one
//! block to each matrix and partitions that matrix's full Gray-index range
//! among the active lanes of one wave.  This Rust implementation is its
//! host-executable correctness mirror: it deliberately uses the same range
//! partition and initializes each local packed accumulator from the canonical
//! `gray_code_index_to_subset` image of that lane's first index.
//!
//! The partition is field-agnostic.  A future packed field can reuse
//! [`RangePartition`] while supplying only its packed accumulator and scalar
//! field operations; it must not invent another interval convention.

#[cfg(feature = "fixture-oracle")]
use gf2_algebra::packed::{Bipedal3, PackedField};
#[cfg(feature = "fixture-oracle")]
use gf2_core::gfp::Fp;

use crate::DispatchResult;
#[cfg(feature = "fixture-oracle")]
use crate::Unsupported;
#[cfg(feature = "fixture-oracle")]
use crate::{fixtures::Fixture, EvaluationResult};

/// RDNA2 wave32 is the fixed execution partition for this prototype.
///
/// The device launch chooses `min(32, 2^n)` active lanes.  This makes the
/// geometry a pure function of matrix order and the caller's batch size; no
/// occupancy query or other device-state input participates in the decision.
pub(crate) const WAVE_LANES: u64 = 32;

/// Exhaustive fixture evaluation stays deliberately below the fast-tier
/// budget.  Larger corpus cells remain visible through the canonical oracle
/// report as explicitly unavailable rather than silently being skipped.
#[cfg(feature = "fixture-oracle")]
pub const MAX_HOST_FIXTURE_ORDER: usize = 16;

/// One contiguous, half-open interval of sequential Gray-code indices.
///
/// The canonical subset at `start` is
/// `gray_code_index_to_subset(start)`.  Intervals partition `[0, total)`;
/// index zero is intentionally present because its zero row-sum term is part
/// of the full Ryser sum and makes the empty-matrix case explicit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GrayInterval {
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// Field-independent balanced partition of a finite index range.
///
/// At most one index separates any two lane intervals.  In this F_3 mapping
/// `total` is a power of two and the lane count is either that whole small
/// range or 32, but retaining the exact balanced formula makes the mapping
/// reusable by another packed arithmetic representation without duplicating
/// partition code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RangePartition {
    total: u64,
    lanes: u64,
}

impl RangePartition {
    pub(crate) fn new(total: u64, requested_lanes: u64) -> Self {
        assert!(total > 0, "a Gray range must contain its empty subset");
        assert!(requested_lanes > 0, "a wave partition needs an active lane");
        Self {
            total,
            lanes: total.min(requested_lanes),
        }
    }

    pub(crate) const fn lanes(self) -> u64 {
        self.lanes
    }

    pub(crate) fn interval(self, lane: u64) -> GrayInterval {
        assert!(lane < self.lanes, "lane belongs to this partition");
        let base = self.total / self.lanes;
        let remainder = self.total % self.lanes;
        let extra_before = lane.min(remainder);
        let start = lane * base + extra_before;
        let width = base + u64::from(lane < remainder);
        GrayInterval {
            start,
            end: start + width,
        }
    }
}

/// Return the deterministic lane/range geometry for a supported Gray walk.
pub(crate) fn partition_for_order(n: usize) -> RangePartition {
    assert!(n <= 63, "F_3 wave prototype follows the n <= 63 Gray bound");
    RangePartition::new(1_u64 << n, WAVE_LANES)
}

/// Canonical binary-reflected Gray subset image `g(k) = k ^ (k >> 1)`.
///
/// This crate-local spelling keeps the field-parametric mapping usable by the
/// registry-only no-default build. Its equality to
/// `gf2_algebra::gray::gray_code_index_to_subset` is asserted in the one
/// canonical mapping test below.
pub(crate) const fn gray_subset(index: u64) -> u64 {
    index ^ (index >> 1)
}

/// Arithmetic supplied by a packed field to the reusable wave mapping.
///
/// This is a data-only bundle rather than a trait: candidate fields provide
/// their existing packed add/subtract, local product, and scalar operations
/// without acquiring a second arithmetic abstraction.
#[derive(Clone, Copy)]
pub(crate) struct WaveOps<P, Scalar> {
    pub(crate) packed_zero: P,
    pub(crate) scalar_zero: Scalar,
    pub(crate) packed_add: fn(P, P) -> P,
    pub(crate) packed_sub: fn(P, P) -> P,
    pub(crate) product: fn(P, usize) -> Scalar,
    pub(crate) scalar_add: fn(Scalar, Scalar) -> Scalar,
    pub(crate) scalar_sub: fn(Scalar, Scalar) -> Scalar,
    pub(crate) scalar_neg: fn(Scalar) -> Scalar,
}

/// The registry dispatch confirms that the candidate's host/device boundary
/// is present.  Device execution remains selected by the crate's opt-in HIP
/// test feature, so ordinary registry users do not need a ROCm runtime.
pub(crate) fn run() -> DispatchResult {
    Ok(())
}

/// Evaluate the F_3 candidate through the canonical fixture/oracle path.
///
/// The full permanent walk is intentionally bounded for this host evidence.
/// The corpus's `n > 16` structural fixtures are retained by the oracle with
/// this explicit reason; no candidate status or fixture cell disappears.
#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate(fixture: &Fixture) -> EvaluationResult {
    if fixture.q() != 3 {
        return Err(Unsupported::new(
            "F_3 wave-cooperative control only accepts F_3 fixtures",
        ));
    }
    if fixture.n() > 63 {
        return Err(Unsupported::new(
            "F_3 wave-cooperative control follows Ryser's n <= 63 Gray-index bound",
        ));
    }
    if fixture.n() > MAX_HOST_FIXTURE_ORDER {
        return Err(Unsupported::new(
            "F_3 wave-cooperative full-permanent fixture evidence is bounded at n <= 16 because exhaustive Ryser equivalence is exponential",
        ));
    }

    Ok(permanent_wave_gf3(fixture).value())
}

/// Evaluate the exact lane partition in lane-index reduction order.
///
/// This is intentionally a direct mirror of the device mapping, not a call to
/// a serial permanent implementation.  Each interval reconstructs its own
/// Bipedal3 `(magnitude, sign)` accumulator from its first canonical Gray
/// subset, walks only its own range, and returns one scalar partial sum.
#[cfg(feature = "fixture-oracle")]
fn permanent_wave_gf3(fixture: &Fixture) -> Fp<3> {
    let n = fixture.n();
    if n == 0 {
        return Fp::<3>::new(1);
    }

    let columns = packed_columns(fixture);
    let partition = partition_for_order(n);
    evaluate_partitioned(
        &columns,
        n,
        partition,
        WaveOps {
            packed_zero: <Bipedal3 as PackedField<Fp<3>>>::zero(),
            scalar_zero: Fp::<3>::new(0),
            packed_add: bipedal3_add,
            packed_sub: bipedal3_sub,
            product: bipedal3_product,
            scalar_add: fp3_add,
            scalar_sub: fp3_sub,
            scalar_neg: fp3_neg,
        },
    )
}

#[cfg(feature = "fixture-oracle")]
fn packed_columns(fixture: &Fixture) -> Vec<Bipedal3> {
    let n = fixture.n();
    (0..n)
        .map(|column| {
            let mut packed = <Bipedal3 as PackedField<Fp<3>>>::zero();
            for row in 0..n {
                packed = packed.with_lane(
                    row,
                    Fp::<3>::new(u64::from(fixture.matrix_bytes()[row * n + column])),
                );
            }
            packed
        })
        .collect()
}

pub(crate) fn evaluate_partitioned<P, Scalar>(
    columns: &[P],
    n: usize,
    partition: RangePartition,
    ops: WaveOps<P, Scalar>,
) -> Scalar
where
    P: Copy,
    Scalar: Copy,
{
    let mut total = ops.scalar_zero;
    for lane in 0..partition.lanes() {
        let partial = walk_interval(columns, n, partition.interval(lane), ops);
        // The device reduction reads lanes 0..active_lanes in this same order.
        total = (ops.scalar_add)(total, partial);
    }

    // Ryser's outer (-1)^n is applied once after the lane partials combine.
    if n % 2 == 1 {
        (ops.scalar_neg)(total)
    } else {
        total
    }
}

/// Walk one interval using caller-supplied packed arithmetic and scalar field
/// operations. This is the field-parametric surface shared by later candidate
/// fields; it deliberately does not prescribe their packed column layout.
pub(crate) fn walk_interval<P, Scalar>(
    columns: &[P],
    n: usize,
    interval: GrayInterval,
    ops: WaveOps<P, Scalar>,
) -> Scalar
where
    P: Copy,
    Scalar: Copy,
{
    debug_assert!(interval.start < interval.end);
    let start_subset = gray_subset(interval.start);
    let mut sums = ops.packed_zero;
    for (column, &packed) in columns.iter().enumerate() {
        if (start_subset >> column) & 1 == 1 {
            sums = (ops.packed_add)(sums, packed);
        }
    }

    let mut partial = signed_product(sums, n, start_subset, ops);
    for index in (interval.start + 1)..interval.end {
        let flipped_column = index.trailing_zeros() as usize;
        let subset = gray_subset(index);
        sums = if (subset >> flipped_column) & 1 == 1 {
            (ops.packed_add)(sums, columns[flipped_column])
        } else {
            (ops.packed_sub)(sums, columns[flipped_column])
        };
        partial = (ops.scalar_add)(partial, signed_product(sums, n, subset, ops));
    }
    partial
}

fn signed_product<P, Scalar>(sums: P, n: usize, subset: u64, ops: WaveOps<P, Scalar>) -> Scalar
where
    P: Copy,
    Scalar: Copy,
{
    let product = (ops.product)(sums, n);
    if subset.count_ones() % 2 == 1 {
        (ops.scalar_sub)(ops.scalar_zero, product)
    } else {
        (ops.scalar_add)(ops.scalar_zero, product)
    }
}

#[cfg(feature = "fixture-oracle")]
fn bipedal3_add(left: Bipedal3, right: Bipedal3) -> Bipedal3 {
    left.add(right)
}

#[cfg(feature = "fixture-oracle")]
fn bipedal3_sub(left: Bipedal3, right: Bipedal3) -> Bipedal3 {
    left.sub(right)
}

#[cfg(feature = "fixture-oracle")]
fn bipedal3_product(sums: Bipedal3, n: usize) -> Fp<3> {
    sums.fold_mul_first_n(n)
}

#[cfg(feature = "fixture-oracle")]
fn fp3_add(left: Fp<3>, right: Fp<3>) -> Fp<3> {
    left + right
}

#[cfg(feature = "fixture-oracle")]
fn fp3_sub(left: Fp<3>, right: Fp<3>) -> Fp<3> {
    left - right
}

#[cfg(feature = "fixture-oracle")]
fn fp3_neg(value: Fp<3>) -> Fp<3> {
    -value
}

#[cfg(all(test, feature = "fixture-oracle"))]
mod tests {
    use gf2_algebra::gray::{gray_code_index_to_subset, gray_code_iter};
    use gf2_algebra::packed::Bipedal3;
    use gf2_algebra::permanent::permanent_ryser;

    use super::*;
    use crate::fixtures::{FixtureCorpus, DEFAULT_FIXTURE_SEED};

    #[test]
    fn ranges_are_contiguous_disjoint_and_exhaustive() {
        for (total, requested_lanes) in [(1, 32), (2, 32), (17, 8), (32, 32), (97, 32)] {
            let partition = RangePartition::new(total, requested_lanes);
            let mut next = 0;
            for lane in 0..partition.lanes() {
                let interval = partition.interval(lane);
                assert_eq!(
                    interval.start, next,
                    "lane {lane} must begin at the prior end"
                );
                assert!(interval.start < interval.end, "each active lane owns work");
                next = interval.end;
            }
            assert_eq!(next, total, "all indices must be assigned exactly once");
        }

        let large = partition_for_order(63);
        assert_eq!(large.lanes(), WAVE_LANES);
        let lane_width = 1_u64 << 58;
        let mut next = 0;
        for lane in 0..large.lanes() {
            let interval = large.interval(lane);
            assert_eq!(interval.start, next, "n=63 lane {lane} start");
            assert_eq!(
                interval.end - interval.start,
                lane_width,
                "n=63 lane {lane} width"
            );
            next = interval.end;
        }
        assert_eq!(next, 1_u64 << 63, "n=63 ranges must cover [0, 2^63)");
        assert_eq!(
            gray_subset(large.interval(1).start),
            (1_u64 << 58) | (1_u64 << 57),
            "the first nonzero n=63 interval must initialize from g(2^58)"
        );
        assert_eq!(
            gray_subset(large.interval(31).start),
            (1_u64 << 62) | (1_u64 << 57),
            "the final n=63 interval must retain canonical Gray-index semantics"
        );
    }

    #[test]
    fn interval_starts_use_canonical_gray_subsets_and_both_directions() {
        let partition = partition_for_order(6);
        for lane in 0..partition.lanes() {
            let start = partition.interval(lane).start;
            let mut replayed = 0_u64;
            for index in 1..=start {
                replayed ^= 1_u64 << index.trailing_zeros();
            }
            assert_eq!(
                gray_subset(start),
                replayed,
                "lane {lane} must initialize from its canonical interval-start subset"
            );
            assert_eq!(
                gray_subset(start),
                gray_code_index_to_subset(start),
                "the reusable mapping must remain equal to gf2_algebra's canonical function"
            );
        }

        let transitions: Vec<_> = gray_code_iter(4).collect();
        assert_eq!(transitions[7], (3, 1), "index 8 must add column 3");
        assert_eq!(transitions[14], (0, -1), "index 15 must remove column 0");
    }

    #[test]
    fn partial_word_product_masks_lane_63_on_the_host() {
        let active = (1_u64 << 63) - 1;
        assert_eq!(
            Bipedal3::from_raw(active, 1_u64 << 63).fold_mul_first_n(63),
            Fp::<3>::new(1),
            "the inactive tail lane must be neutralized before the product"
        );
        assert_eq!(
            Bipedal3::from_raw(active & !(1_u64 << 62), 0).fold_mul_first_n(63),
            Fp::<3>::new(0),
            "an active zero must not be masked away"
        );
    }

    #[test]
    fn f3_fixture_values_match_ryser_through_the_registered_mapping() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        for fixture in corpus
            .fixtures()
            .iter()
            .filter(|fixture| fixture.q() == 3 && fixture.n() <= MAX_HOST_FIXTURE_ORDER)
        {
            let expected = permanent_ryser::<Fp<3>>(
                &fixture
                    .matrix_bytes()
                    .iter()
                    .map(|&value| Fp::<3>::new(u64::from(value)))
                    .collect::<Vec<_>>(),
                fixture.n(),
            )
            .value();
            assert_eq!(
                evaluate(fixture),
                Ok(expected),
                "{} must use the canonical Gray subset mapping",
                fixture.id()
            );
        }
    }

    #[test]
    fn large_structural_fixture_stays_explicitly_unavailable() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let partial_word = corpus
            .fixtures()
            .iter()
            .find(|fixture| fixture.q() == 3 && fixture.n() == 63)
            .expect("the committed F_3 partial-word fixture must remain present");
        let error = evaluate(partial_word).expect_err("n=63 is not an exhaustive test case");
        assert!(error.reason().contains("n <= 16"));
        assert!(error.reason().contains("exponential"));
    }
}
