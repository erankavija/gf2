//! F_3 zero-mask sign-popcount fold candidate.

//! The candidate deliberately reuses [`crate::wave`]'s partitioned Gray walk,
//! packed column construction, and lane-index reduction. Only the lane-local
//! product changes: magnitude-clear active lanes select zero, otherwise the
//! parity of active sign bits selects one or minus one.

#[cfg(feature = "fixture-oracle")]
use gf2_algebra::packed::Bipedal3;
#[cfg(feature = "fixture-oracle")]
use gf2_core::gfp::Fp;

use crate::DispatchResult;
#[cfg(feature = "fixture-oracle")]
use crate::{fixtures::Fixture, wave, EvaluationResult, Unsupported};

/// The candidate shares the control's host/device boundary; the optional HIP
/// evidence path selects this fold through the existing `FoldGf3` registry
/// entry rather than introducing a second measurement registry.
pub(crate) fn run() -> DispatchResult {
    Ok(())
}

/// Evaluate the zero-mask fold through the control's exact lane mapping.
///
/// Full permanent equality is intentionally limited to the tractable fixture
/// bound. The order-63 structural fixture remains an explicit unavailable row
/// because enumerating its `2^63` Ryser terms is infeasible; its mask and
/// interval behavior are probed without claiming a full permanent result.
#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate(fixture: &Fixture) -> EvaluationResult {
    if fixture.q() != 3 {
        return Err(Unsupported::new(
            "F_3 zero-mask sign-popcount fold only accepts F_3 fixtures",
        ));
    }
    if fixture.n() > 63 {
        return Err(Unsupported::new(
            "F_3 zero-mask sign-popcount fold follows Ryser's n <= 63 Gray-index bound",
        ));
    }
    if fixture.n() > wave::MAX_HOST_FIXTURE_ORDER {
        return Err(Unsupported::new(
            "F_3 zero-mask sign-popcount full-permanent fixture evidence is bounded at n <= 16 because exhaustive Ryser equivalence is exponential",
        ));
    }
    if fixture.n() == 0 {
        return Ok(1);
    }

    let n = fixture.n();
    let columns = wave::packed_columns(fixture);
    Ok(wave::evaluate_partitioned(
        &columns,
        n,
        wave::partition_for_order(n),
        wave::f3_wave_ops(sign_popcount_product),
    )
    .value())
}

/// Return the F_3 product of the first `n` bipedal lanes by detecting an
/// active zero once, then reducing the nonzero signs by population-count
/// parity. `n` is at most 63 for this wave mapping, so the active-mask shift
/// never reaches the width of a `u64`.
#[cfg(feature = "fixture-oracle")]
fn sign_popcount_product(sums: Bipedal3, n: usize) -> Fp<3> {
    assert!(n <= 63, "F_3 wave fold supports at most 63 active lanes");
    let active = (1_u64 << n) - 1;
    if (!sums.mag() & active) != 0 {
        return Fp::<3>::new(0);
    }
    if (sums.sgn() & active).count_ones().is_multiple_of(2) {
        Fp::<3>::new(1)
    } else {
        Fp::<3>::new(2)
    }
}

#[cfg(all(test, feature = "fixture-oracle"))]
mod tests {
    use gf2_algebra::packed::Bipedal3;
    use gf2_algebra::permanent::permanent_ryser;
    use gf2_core::gfp::Fp;

    use super::*;
    use crate::fixtures::{FixtureCorpus, DEFAULT_FIXTURE_SEED};
    use crate::wave::MAX_HOST_FIXTURE_ORDER;

    fn canonical_state(mut encoded_lanes: usize, n: usize) -> Bipedal3 {
        let mut magnitude = 0_u64;
        let mut sign = 0_u64;
        for lane in 0..n {
            match encoded_lanes % 3 {
                0 => {}
                1 => magnitude |= 1_u64 << lane,
                2 => {
                    magnitude |= 1_u64 << lane;
                    sign |= 1_u64 << lane;
                }
                _ => unreachable!("remainders modulo three are canonical F_3 lanes"),
            }
            encoded_lanes /= 3;
        }
        Bipedal3::from_raw(magnitude, sign)
    }

    #[test]
    fn zero_mask_sign_popcount_matches_halving_fold_exhaustively_for_small_lanes() {
        for n in 1..=8 {
            for encoded_lanes in 0..3_usize.pow(n as u32) {
                let sums = canonical_state(encoded_lanes, n);
                assert_eq!(
                    sign_popcount_product(sums, n),
                    sums.fold_mul_first_n(n),
                    "n={n}, canonical packed state={encoded_lanes}"
                );
            }
        }
    }

    #[test]
    fn zero_mask_fold_masks_inactive_lane_and_preserves_active_zero_at_n63() {
        let active = (1_u64 << 63) - 1;
        assert_eq!(
            sign_popcount_product(Bipedal3::from_raw(active, 1_u64 << 63), 63),
            Fp::<3>::new(1),
            "the inactive sign lane must not affect the sign population count"
        );
        assert_eq!(
            sign_popcount_product(Bipedal3::from_raw(active & !(1_u64 << 62), 0), 63),
            Fp::<3>::new(0),
            "an active zero must select the zero-mask fast path"
        );
    }

    #[test]
    fn zero_mask_fold_matches_ryser_on_every_supported_committed_f3_fixture() {
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
                "{} must follow the shared wave mapping",
                fixture.id()
            );
        }
    }

    #[test]
    fn n63_full_permanent_stays_explicitly_unsupported() {
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
