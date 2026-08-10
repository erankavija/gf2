//! Host-executable F_7 three-plane accumulator for the wave study.
//!
//! The state stores the canonical binary value of each row in three bit
//! planes.  Bit `i` of `(b0, b1, b2)` is row `i`, so one triple covers the
//! 64-row range without the sixteen-nibble bound of `Packed7`.  The update
//! circuit is the Mersenne fold from the archived `f7_packing` candidate D:
//! `7 = 2^3 - 1`, so a three-bit carry folds back as one.
//!
//! This is deliberately an accumulator-level candidate. Its opt-in HIP test
//! validates the arithmetic circuits on device, but it does not claim a
//! permanent-shaped HIP kernel or its device-resource evidence; those paths
//! are explicitly owned by issue `cc162697`. Keeping the host permanent driver
//! executable makes that boundary falsifiable rather than silently dropping
//! the candidate from the study registry.
//!
//! The native-modular control below is a paired correctness control for this
//! accumulator, not a second measurement path.  In particular,
//! `F7LookupTableControl` already names the later permanent-shaped kernel and
//! remains in `wave_gf7` for `cc162697`; registering this control there would
//! misrepresent a host accumulator as that device experiment.

#[cfg(feature = "fixture-oracle")]
use gf2_algebra::gray::gray_code_iter;

use crate::DispatchResult;
#[cfg(feature = "fixture-oracle")]
use crate::{fixtures::Fixture, EvaluationResult, Unsupported};

#[cfg(feature = "fixture-oracle")]
const FIELD_ORDER: u64 = 7;
#[cfg(feature = "fixture-oracle")]
const MAX_ROWS: usize = 63;
#[cfg(all(test, feature = "fixture-oracle"))]
const DEVICE_UNAVAILABLE: &str =
    "F_7 three-plane accumulator has no permanent-shaped HIP kernel; cc162697 owns it";

/// One canonical F_7 word, bit-sliced across up to 64 row lanes.
///
/// The all-ones bit pattern is never a canonical lane value.  Constructors
/// accept only `0..=6`, and the Mersenne add/subtract circuits preserve that
/// representation when their inputs are canonical.
#[cfg(feature = "fixture-oracle")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThreePlane {
    b0: u64,
    b1: u64,
    b2: u64,
}

#[cfg(feature = "fixture-oracle")]
impl ThreePlane {
    const ZERO: Self = Self {
        b0: 0,
        b1: 0,
        b2: 0,
    };

    fn from_values(values: &[u8]) -> Self {
        assert!(values.len() <= 64, "three-plane state has at most 64 lanes");
        let mut state = Self::ZERO;
        for (lane, &value) in values.iter().enumerate() {
            assert!(value < FIELD_ORDER as u8, "F_7 lanes must be canonical");
            let mask = 1_u64 << lane;
            if value & 1 != 0 {
                state.b0 |= mask;
            }
            if value & 2 != 0 {
                state.b1 |= mask;
            }
            if value & 4 != 0 {
                state.b2 |= mask;
            }
        }
        state
    }

    #[cfg(test)]
    fn lane(self, lane: usize) -> u8 {
        assert!(lane < 64, "three-plane lane must be below 64");
        let value = (((self.b0 >> lane) & 1)
            | (((self.b1 >> lane) & 1) << 1)
            | (((self.b2 >> lane) & 1) << 2)) as u8;
        assert!(
            value < FIELD_ORDER as u8,
            "three-plane state must stay canonical"
        );
        value
    }

    /// Add canonical F_7 planes with a Mersenne carry fold.
    #[must_use]
    fn add(self, rhs: Self) -> Self {
        // Three-bit ripple add, producing a fourth carry plane.
        let s0 = self.b0 ^ rhs.b0;
        let c1 = self.b0 & rhs.b0;
        let axb1 = self.b1 ^ rhs.b1;
        let s1 = axb1 ^ c1;
        let c2 = (self.b1 & rhs.b1) | (c1 & axb1);
        let axb2 = self.b2 ^ rhs.b2;
        let s2 = axb2 ^ c2;
        let c3 = (self.b2 & rhs.b2) | (c2 & axb2);

        // A low result of seven folds to zero; a carry folds as `low + 1`.
        let s1_and_s0 = s1 & s0;
        let is_seven = s1_and_s0 & s2;
        let no_seven = !is_seven;
        let no_carry = !c3;

        let b0 = ((s0 & no_seven) & no_carry) | ((!s0) & c3);
        let b1 = ((s1 & no_seven) & no_carry) | ((s1 ^ s0) & c3);
        let b2 = ((s2 & no_seven) & no_carry) | ((s2 ^ s1_and_s0) & c3);
        Self { b0, b1, b2 }
    }

    /// Subtract canonical F_7 planes by adding the canonical additive inverse.
    #[must_use]
    fn sub(self, rhs: Self) -> Self {
        let nonzero = rhs.b0 | rhs.b1 | rhs.b2;
        let negated = Self {
            b0: (!rhs.b0) & nonzero,
            b1: (!rhs.b1) & nonzero,
            b2: (!rhs.b2) & nonzero,
        };
        self.add(negated)
    }

    /// Product of the active row lanes, using the C6 log-popcount reduction.
    #[must_use]
    fn horizontal_product(self, active: u64) -> u8 {
        debug_assert_eq!(self.b0 & self.b1 & self.b2 & active, 0);
        let b0 = self.b0 & active;
        let b1 = self.b1 & active;
        let b2 = self.b2 & active;
        let zero_mask = !(b0 | b1 | b2) & active;
        if zero_mask != 0 {
            return 0;
        }

        // 3 is a generator of F_7*: 1, 3, 2, 6, 4, 5 correspond to 0..5.
        // Every nonzero selector has a positive plane, so the input masking
        // above also masks these complements and no tail lane contributes.
        let not_b0 = !b0;
        let not_b1 = !b1;
        let not_b2 = !b2;
        let exponent = (b0 & b1 & not_b2).count_ones()
            + 2 * (not_b0 & b1 & not_b2).count_ones()
            + 3 * (not_b0 & b1 & b2).count_ones()
            + 4 * (not_b0 & not_b1 & b2).count_ones()
            + 5 * (b0 & not_b1 & b2).count_ones();
        const EXPONENT_TO_VALUE: [u8; 6] = [1, 3, 2, 6, 4, 5];
        EXPONENT_TO_VALUE[(exponent % 6) as usize]
    }
}

/// The registry dispatch confirms the host accumulator is compiled and ready.
pub(crate) fn run() -> DispatchResult {
    Ok(())
}

/// Evaluate the three-plane accumulator through its registered candidate path.
#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate(fixture: &Fixture) -> EvaluationResult {
    if fixture.q() != FIELD_ORDER {
        return Err(Unsupported::new(
            "F_7 three-plane accumulator only accepts F_7 fixtures",
        ));
    }
    if fixture.n() > MAX_ROWS {
        return Err(Unsupported::new(
            "F_7 three-plane accumulator follows Ryser's n <= 63 Gray-index bound",
        ));
    }

    // Return the candidate itself, rather than a paired-control assertion, so
    // `check_registered_candidates` can retain any candidate/Ryser mismatch
    // with its fixture identifier in the canonical report.
    Ok(u64::from(permanent_three_plane(fixture)))
}

/// Explicit permanent-kernel falsification for this accumulator-level issue.
///
/// The opt-in HIP test validates arithmetic only; this keeps the absence of a
/// device permanent path inspectable while the host implementation remains
/// reachable through [`evaluate`].
#[cfg(all(test, feature = "fixture-oracle"))]
#[must_use]
fn device_falsification() -> Unsupported {
    Unsupported::new(DEVICE_UNAVAILABLE)
}

#[cfg(feature = "fixture-oracle")]
fn permanent_three_plane(fixture: &Fixture) -> u8 {
    let n = fixture.n();
    if n == 0 {
        return 1;
    }
    let columns = bit_sliced_columns(fixture);
    let active = (1_u64 << n) - 1;
    let mut row_sums = ThreePlane::ZERO;
    let mut total = 0_u8;
    let mut odd_subset = false;

    for (column, direction) in gray_code_iter(n) {
        // Each Gray transition changes the subset cardinality by exactly one.
        odd_subset = !odd_subset;
        if direction == 1 {
            row_sums = row_sums.add(columns[column]);
        } else {
            row_sums = row_sums.sub(columns[column]);
        }
        let term = row_sums.horizontal_product(active);
        total = if odd_subset {
            sub_mod(total, term)
        } else {
            add_mod(total, term)
        };
    }

    if n % 2 == 1 {
        sub_mod(0, total)
    } else {
        total
    }
}

/// Native-modular representation control with the same Gray walk and signs.
#[cfg(all(test, feature = "fixture-oracle"))]
fn permanent_native_modular(fixture: &Fixture) -> u8 {
    let n = fixture.n();
    if n == 0 {
        return 1;
    }
    let mut row_sums = vec![0_u8; n];
    let mut total = 0_u8;
    let mut odd_subset = false;

    for (column, direction) in gray_code_iter(n) {
        // Each Gray transition changes the subset cardinality by exactly one.
        odd_subset = !odd_subset;
        if direction == 1 {
            for (row, sum) in row_sums.iter_mut().enumerate() {
                *sum = add_mod(*sum, fixture.matrix_bytes()[row * n + column]);
            }
        } else {
            for (row, sum) in row_sums.iter_mut().enumerate() {
                *sum = sub_mod(*sum, fixture.matrix_bytes()[row * n + column]);
            }
        }
        let term = row_sums.iter().copied().fold(1, mul_mod);
        total = if odd_subset {
            sub_mod(total, term)
        } else {
            add_mod(total, term)
        };
    }

    if n % 2 == 1 {
        sub_mod(0, total)
    } else {
        total
    }
}

#[cfg(feature = "fixture-oracle")]
fn bit_sliced_columns(fixture: &Fixture) -> Vec<ThreePlane> {
    let n = fixture.n();
    (0..n)
        .map(|column| {
            let values: Vec<_> = (0..n)
                .map(|row| fixture.matrix_bytes()[row * n + column])
                .collect();
            ThreePlane::from_values(&values)
        })
        .collect()
}

#[cfg(feature = "fixture-oracle")]
#[inline]
fn add_mod(left: u8, right: u8) -> u8 {
    (left + right) % FIELD_ORDER as u8
}

#[cfg(feature = "fixture-oracle")]
#[inline]
fn sub_mod(left: u8, right: u8) -> u8 {
    (left + FIELD_ORDER as u8 - right) % FIELD_ORDER as u8
}

#[inline]
#[cfg(all(test, feature = "fixture-oracle"))]
fn mul_mod(left: u8, right: u8) -> u8 {
    (left * right) % FIELD_ORDER as u8
}

#[cfg(all(test, feature = "fixture-oracle"))]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::fixtures::{Fixture, FixtureCorpus, FixtureRequirement, DEFAULT_FIXTURE_SEED};

    fn f7_parity_shard(corpus: &FixtureCorpus, shard: usize) -> Vec<&Fixture> {
        corpus
            .fixtures()
            .iter()
            .enumerate()
            .filter_map(|(index, fixture)| {
                (index % 2 == shard && fixture.q() == FIELD_ORDER).then_some(fixture)
            })
            .collect()
    }

    fn assert_f7_parity_shards_partition_the_corpus(corpus: &FixtureCorpus) {
        let first: BTreeSet<_> = f7_parity_shard(corpus, 0)
            .into_iter()
            .map(Fixture::id)
            .collect();
        let second: BTreeSet<_> = f7_parity_shard(corpus, 1)
            .into_iter()
            .map(Fixture::id)
            .collect();
        let complete: BTreeSet<_> = corpus
            .fixtures()
            .iter()
            .filter(|fixture| fixture.q() == FIELD_ORDER)
            .map(Fixture::id)
            .collect();

        assert!(first.is_disjoint(&second));
        assert_eq!(first.len() + second.len(), complete.len());
        assert_eq!(
            first.union(&second).copied().collect::<BTreeSet<_>>(),
            complete
        );
    }

    fn assert_native_control_parity_shard(shard: usize) {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        assert_f7_parity_shards_partition_the_corpus(&corpus);
        for fixture in f7_parity_shard(&corpus, shard) {
            assert_eq!(
                permanent_three_plane(fixture),
                permanent_native_modular(fixture),
                "native control diverged for {}",
                fixture.id()
            );
        }
    }

    #[test]
    fn mersenne_add_and_subtract_are_exhaustive_over_ordered_lane_pairs() {
        for left in 0..FIELD_ORDER as u8 {
            for right in 0..FIELD_ORDER as u8 {
                let left_planes = ThreePlane::from_values(&[left]);
                let right_planes = ThreePlane::from_values(&[right]);
                assert_eq!(
                    left_planes.add(right_planes).lane(0),
                    add_mod(left, right),
                    "add {left} + {right}"
                );
                assert_eq!(
                    left_planes.sub(right_planes).lane(0),
                    sub_mod(left, right),
                    "sub {left} - {right}"
                );
            }
        }
    }

    #[test]
    fn c6_log_population_product_is_exhaustive_for_three_lanes() {
        for first in 0..FIELD_ORDER as u8 {
            for second in 0..FIELD_ORDER as u8 {
                for third in 0..FIELD_ORDER as u8 {
                    let state = ThreePlane::from_values(&[first, second, third]);
                    let expected = mul_mod(mul_mod(first, second), third);
                    assert_eq!(
                        state.horizontal_product(0b111),
                        expected,
                        "product ({first}, {second}, {third})"
                    );
                }
            }
        }
    }

    #[test]
    fn native_control_even_shard_matches_three_planes_on_all_f7_fixtures() {
        assert_native_control_parity_shard(0);
    }

    #[test]
    fn native_control_odd_shard_matches_three_planes_on_all_f7_fixtures() {
        assert_native_control_parity_shard(1);
    }

    #[test]
    fn active_mask_excludes_noncanonical_tail_bits_from_the_product() {
        let state = ThreePlane {
            b0: 0b01,
            b1: 0,
            b2: !0b11,
        };
        assert_eq!(state.horizontal_product(0b01), 1);
    }

    #[test]
    fn corpus_contains_the_required_f7_structural_and_high_order_cases() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        for n in [16, 20, 24] {
            assert!(corpus
                .fixtures()
                .iter()
                .any(|fixture| fixture.q() == FIELD_ORDER && fixture.n() == n));
        }
        for requirement in [
            FixtureRequirement::GrayAdditionTransition,
            FixtureRequirement::GraySubtractionTransition,
            FixtureRequirement::ZeroContainingRowProduct,
        ] {
            assert!(corpus.fixtures().iter().any(|fixture| {
                fixture.q() == FIELD_ORDER && fixture.has_requirement(&requirement)
            }));
        }
        for exponent in 0..6 {
            assert!(corpus.fixtures().iter().any(|fixture| {
                fixture.q() == FIELD_ORDER
                    && fixture
                        .has_requirement(&FixtureRequirement::NonzeroExponentClass { exponent })
            }));
        }
    }

    #[test]
    fn permanent_kernel_absence_is_explicitly_falsified() {
        assert_eq!(device_falsification().reason(), DEVICE_UNAVAILABLE);
        assert!(run().is_ok());
    }
}
