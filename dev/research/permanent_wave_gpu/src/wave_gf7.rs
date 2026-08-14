//! Host mirrors for the paired F_7 permanent-shaped kernel candidates.
//!
//! The HIP implementation keeps the canonical lane-owns-interval mapping in
//! `hip/wave_gf7_equivalence.hip`. These bounded host mirrors reuse
//! [`crate::wave`]'s interval reconstruction and lane-order reduction so the
//! fixture oracle can exercise the same mapping without selecting a device.
//! They intentionally stop at order 16; the direct HIP evidence driver owns
//! the exact order-20 and order-24 comparisons.

#[cfg(feature = "fixture-oracle")]
use gf2_algebra::packed::Packed7;
#[cfg(feature = "fixture-oracle")]
use gf2_core::gfp::Fp;

use crate::device_batch::{DeviceBatchKernel, DeviceExecutable};
use crate::paths::DeviceBatchResult;
#[cfg(feature = "fixture-oracle")]
use crate::{f7_three_plane, fixtures::Fixture, wave, EvaluationResult, Unsupported};

#[cfg(feature = "fixture-oracle")]
const FIELD_ORDER: u64 = 7;
#[cfg(feature = "fixture-oracle")]
const MAX_HOST_FIXTURE_ORDER: usize = 16;

/// The F_7 lookup-table control's device batch kernel.
///
/// `wave_gf7_lookup_table_kernel<Words>` packs `ceil(n / 16)` nibble words per
/// column, and `hip/wave_gf7_equivalence.hip` instantiates it up to
/// `kMaxControlWords = 2`, so the control accepts `n <= 32`. Its arithmetic
/// reads the canonical Packed7 tables, which the batch boundary uploads.
pub(crate) fn lookup_table_control_device_batch_kernel() -> DeviceBatchResult {
    Ok(DeviceBatchKernel::new(
        7,
        32,
        DeviceExecutable::WaveGf7,
        &["--path", "f7-lookup-table-control"],
        true,
    ))
}

/// The F_7 permanent-shaped three-plane kernel's device batch evaluator.
///
/// `wave_gf7_three_plane_kernel` consumes prepared bit planes, so a batch
/// launch stages them through `prepare_three_plane_columns` first. It carries
/// no multiplication tables and keeps Ryser's `n <= 63` Gray bound.
pub(crate) fn three_plane_device_batch_kernel() -> DeviceBatchResult {
    Ok(DeviceBatchKernel::new(
        7,
        63,
        DeviceExecutable::WaveGf7,
        &["--path", "f7-three-plane-permanent"],
        false,
    ))
}

#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate_lookup_table_control(fixture: &Fixture) -> EvaluationResult {
    check_host_fixture_bound(fixture)?;
    if fixture.n() == 0 {
        return Ok(1);
    }
    let columns = packed_columns(fixture);
    Ok(wave::evaluate_partitioned(
        &columns,
        fixture.n(),
        wave::partition_for_order(fixture.n()),
        packed7_wave_ops(),
    )
    .value())
}

#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate_three_plane(fixture: &Fixture) -> EvaluationResult {
    check_host_fixture_bound(fixture)?;
    if fixture.n() == 0 {
        return Ok(1);
    }
    let columns = f7_three_plane::bit_sliced_columns(fixture);
    Ok(wave::evaluate_partitioned(
        &columns,
        fixture.n(),
        wave::partition_for_order(fixture.n()),
        three_plane_wave_ops(),
    ))
}

#[cfg(feature = "fixture-oracle")]
fn check_host_fixture_bound(fixture: &Fixture) -> Result<(), Unsupported> {
    if fixture.q() != FIELD_ORDER {
        return Err(Unsupported::new(
            "F_7 permanent-shaped candidates only accept F_7 fixtures",
        ));
    }
    if fixture.n() > MAX_HOST_FIXTURE_ORDER {
        return Err(Unsupported::new(
            "F_7 permanent-shaped full-permanent fixture evidence is bounded at n <= 16; orders 20 and 24 are checked by the opt-in device evidence driver because exhaustive Ryser equality is exponential",
        ));
    }
    Ok(())
}

#[cfg(feature = "fixture-oracle")]
fn packed_columns(fixture: &Fixture) -> Vec<Packed7> {
    (0..fixture.n())
        .map(|column| {
            let mut packed = Packed7::zero();
            for row in 0..fixture.n() {
                packed = packed.with_lane(
                    row,
                    Fp::<7>::new(u64::from(
                        fixture.matrix_bytes()[row * fixture.n() + column],
                    )),
                );
            }
            packed
        })
        .collect()
}

#[cfg(feature = "fixture-oracle")]
fn packed7_wave_ops() -> wave::WaveOps<Packed7, Fp<7>> {
    wave::WaveOps {
        packed_zero: Packed7::zero(),
        scalar_zero: Fp::<7>::new(0),
        packed_add: Packed7::add_inherent,
        packed_sub: Packed7::sub_inherent,
        product: Packed7::fold_mul_first_n,
        scalar_add: f7_add,
        scalar_sub: f7_sub,
        scalar_neg: f7_neg,
    }
}

#[cfg(feature = "fixture-oracle")]
fn three_plane_wave_ops() -> wave::WaveOps<f7_three_plane::ThreePlane, u64> {
    wave::WaveOps {
        packed_zero: f7_three_plane::ThreePlane::ZERO,
        scalar_zero: 0,
        packed_add: f7_three_plane::ThreePlane::add,
        packed_sub: f7_three_plane::ThreePlane::sub,
        product: three_plane_product,
        scalar_add: add_scalar,
        scalar_sub: sub_scalar,
        scalar_neg: neg_scalar,
    }
}

#[cfg(feature = "fixture-oracle")]
fn three_plane_product(sums: f7_three_plane::ThreePlane, n: usize) -> u64 {
    let active = (1_u64 << n) - 1;
    u64::from(sums.horizontal_product(active))
}

#[cfg(feature = "fixture-oracle")]
fn f7_add(left: Fp<7>, right: Fp<7>) -> Fp<7> {
    left + right
}

#[cfg(feature = "fixture-oracle")]
fn f7_sub(left: Fp<7>, right: Fp<7>) -> Fp<7> {
    left - right
}

#[cfg(feature = "fixture-oracle")]
fn f7_neg(value: Fp<7>) -> Fp<7> {
    -value
}

#[cfg(feature = "fixture-oracle")]
fn add_scalar(left: u64, right: u64) -> u64 {
    (left + right) % FIELD_ORDER
}

#[cfg(feature = "fixture-oracle")]
fn sub_scalar(left: u64, right: u64) -> u64 {
    (left + FIELD_ORDER - right) % FIELD_ORDER
}

#[cfg(feature = "fixture-oracle")]
fn neg_scalar(value: u64) -> u64 {
    (FIELD_ORDER - value) % FIELD_ORDER
}

#[cfg(all(test, feature = "fixture-oracle"))]
mod tests {
    use gf2_algebra::permanent::permanent_ryser;

    use super::*;
    use crate::fixtures::{FixtureCorpus, DEFAULT_FIXTURE_SEED};

    #[test]
    fn paired_host_mirrors_match_ryser_on_small_f7_fixtures() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        for fixture in corpus
            .fixtures()
            .iter()
            .filter(|fixture| fixture.q() == FIELD_ORDER && fixture.n() <= 4)
        {
            let expected = permanent_ryser::<Fp<7>>(
                &fixture
                    .matrix_bytes()
                    .iter()
                    .map(|&value| Fp::<7>::new(u64::from(value)))
                    .collect::<Vec<_>>(),
                fixture.n(),
            )
            .value();
            assert_eq!(evaluate_lookup_table_control(fixture), Ok(expected));
            assert_eq!(evaluate_three_plane(fixture), Ok(expected));
        }
    }

    #[test]
    fn paired_host_mirrors_leave_expensive_orders_to_device_evidence() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        for n in [20, 24] {
            let fixture = corpus
                .fixtures()
                .iter()
                .find(|fixture| fixture.q() == FIELD_ORDER && fixture.n() == n)
                .expect("the committed corpus must retain the device-evidence order");
            for evaluate in [evaluate_lookup_table_control, evaluate_three_plane] {
                let unavailable = evaluate(fixture)
                    .expect_err("the normal host tier must not enumerate n=20 or n=24");
                assert!(unavailable.reason().contains("n <= 16"));
            }
        }
    }
}
