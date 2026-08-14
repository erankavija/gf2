//! F_5 representation controls under the wave-owned Gray-interval mapping.
//!
//! Both candidates reuse [`crate::wave::evaluate_partitioned`]: every logical
//! wave lane reconstructs its own row-sum accumulator for the canonical Gray
//! subset at the start of its contiguous interval, advances it with one
//! column update per transition, and contributes only its scalar partial
//! Ryser sum in lane order. The byte path is the shipped HIP kernel's
//! elementwise modular control. The three-plane path uses the canonical
//! [`Packed5`] accumulator and its read-only planes for the $C_4$ row-product
//! reduction. The HIP circuit remains the necessary language-boundary
//! transliteration of that canonical Rust representation.

#[cfg(feature = "fixture-oracle")]
use gf2_algebra::packed::{Packed5, PackedField};
#[cfg(feature = "fixture-oracle")]
use gf2_core::gfp::Fp;

use crate::device_batch::{DeviceBatchKernel, DeviceExecutable};
use crate::paths::DeviceBatchResult;
#[cfg(feature = "fixture-oracle")]
use crate::wave::{self, WaveOps};
#[cfg(feature = "fixture-oracle")]
use crate::{fixtures::Fixture, EvaluationResult, Unsupported};

#[cfg(feature = "fixture-oracle")]
const FIELD_ORDER: u8 = 5;
/// The largest F_5 fixture order admitted by the full host walk.
///
/// The order-63 partial-word corpus row remains an explicit structural
/// boundary rather than an infeasible `2^63` permanent calculation.
#[cfg(feature = "fixture-oracle")]
const MAX_HOST_FIXTURE_ORDER: usize = 16;
#[cfg(feature = "fixture-oracle")]
const MAX_ROWS: usize = 63;

/// Byte-oriented F_5 row sums, matching the shipped kernel's representation.
///
/// On device the stable per-lane state is `n + 20` bytes: `n` accumulator
/// bytes, two `u64` interval bounds, and one `u32` partial sum. Loop
/// temporaries are intentionally excluded; the compiler resource report is
/// authoritative for allocation and spill behaviour.
#[cfg(feature = "fixture-oracle")]
#[derive(Clone, Copy)]
struct ByteAccumulator {
    values: [u8; MAX_ROWS],
}

#[cfg(feature = "fixture-oracle")]
impl ByteAccumulator {
    const ZERO: Self = Self {
        values: [0; MAX_ROWS],
    };

    fn from_column(fixture: &Fixture, column: usize) -> Self {
        let mut accumulator = Self::ZERO;
        for row in 0..fixture.n() {
            accumulator.values[row] = fixture.matrix_bytes()[row * fixture.n() + column];
        }
        accumulator
    }

    #[must_use]
    fn add(self, rhs: Self) -> Self {
        let mut result = self;
        for (left, &right) in result.values.iter_mut().zip(rhs.values.iter()) {
            *left = add_mod(*left, right);
        }
        result
    }

    #[must_use]
    fn sub(self, rhs: Self) -> Self {
        let mut result = self;
        for (left, &right) in result.values.iter_mut().zip(rhs.values.iter()) {
            *left = sub_mod(*left, right);
        }
        result
    }

    #[must_use]
    fn horizontal_product(self, n: usize) -> u8 {
        self.values[..n].iter().copied().fold(1, mul_mod)
    }
}

/// The byte representation control's device batch kernel.
///
/// `f5_byte_control_kernel` in `hip/f5_wave_equivalence.hip` gives one block to
/// each matrix of a batch and stages `n * n` bytes per block, within the
/// mapping's `n <= 63` Gray bound.
pub(crate) fn byte_control_device_batch_kernel() -> DeviceBatchResult {
    Ok(DeviceBatchKernel::new(
        5,
        63,
        DeviceExecutable::F5Wave,
        &["--path", "f5-byte-control"],
        false,
    ))
}

/// The canonical three-plane candidate's device batch kernel.
///
/// `f5_three_plane_kernel` shares the control's executable and batch geometry,
/// staging three packed planes per column instead of the byte table.
pub(crate) fn three_plane_device_batch_kernel() -> DeviceBatchResult {
    Ok(DeviceBatchKernel::new(
        5,
        63,
        DeviceExecutable::F5Wave,
        &["--path", "f5-three-plane"],
        false,
    ))
}

/// Evaluate the byte-oriented modular control through its registered path.
#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate_byte_control(fixture: &Fixture) -> EvaluationResult {
    validate_fixture(fixture)?;
    Ok(permanent_byte_control(fixture).value())
}

/// Evaluate the canonical three-plane candidate through its registered path.
#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate_three_plane(fixture: &Fixture) -> EvaluationResult {
    validate_fixture(fixture)?;
    Ok(permanent_three_plane(fixture).value())
}

#[cfg(feature = "fixture-oracle")]
fn validate_fixture(fixture: &Fixture) -> Result<(), Unsupported> {
    if fixture.q() != u64::from(FIELD_ORDER) {
        return Err(Unsupported::new(
            "F_5 wave candidates only accept F_5 fixtures",
        ));
    }
    if fixture.n() > MAX_ROWS {
        return Err(Unsupported::new(
            "F_5 wave candidates follow Ryser's n <= 63 Gray-index bound",
        ));
    }
    if fixture.n() > MAX_HOST_FIXTURE_ORDER {
        return Err(Unsupported::new(
            "F_5 wave full-permanent fixture evidence is bounded at n <= 16 because exhaustive Ryser equivalence is exponential",
        ));
    }
    Ok(())
}

#[cfg(feature = "fixture-oracle")]
fn permanent_byte_control(fixture: &Fixture) -> Fp<5> {
    let columns = (0..fixture.n())
        .map(|column| ByteAccumulator::from_column(fixture, column))
        .collect::<Vec<_>>();
    wave::evaluate_partitioned(
        &columns,
        fixture.n(),
        wave::partition_for_order(fixture.n()),
        byte_ops(),
    )
}

#[cfg(feature = "fixture-oracle")]
fn permanent_three_plane(fixture: &Fixture) -> Fp<5> {
    let columns = (0..fixture.n())
        .map(|column| packed5_column(fixture, column))
        .collect::<Vec<_>>();
    wave::evaluate_partitioned(
        &columns,
        fixture.n(),
        wave::partition_for_order(fixture.n()),
        packed5_ops(),
    )
}

#[cfg(feature = "fixture-oracle")]
fn packed5_column(fixture: &Fixture, column: usize) -> Packed5 {
    let mut packed = Packed5::zero();
    for row in 0..fixture.n() {
        packed = packed.with_lane(
            row,
            Fp::<5>::new(u64::from(
                fixture.matrix_bytes()[row * fixture.n() + column],
            )),
        );
    }
    packed
}

#[cfg(feature = "fixture-oracle")]
fn byte_ops() -> WaveOps<ByteAccumulator, Fp<5>> {
    WaveOps {
        packed_zero: ByteAccumulator::ZERO,
        scalar_zero: Fp::<5>::new(0),
        packed_add: ByteAccumulator::add,
        packed_sub: ByteAccumulator::sub,
        product: byte_product,
        scalar_add: fp5_add,
        scalar_sub: fp5_sub,
        scalar_neg: fp5_neg,
    }
}

#[cfg(feature = "fixture-oracle")]
fn packed5_ops() -> WaveOps<Packed5, Fp<5>> {
    WaveOps {
        packed_zero: Packed5::zero(),
        scalar_zero: Fp::<5>::new(0),
        packed_add: Packed5::add,
        packed_sub: Packed5::sub,
        product: packed5_c4_product,
        scalar_add: fp5_add,
        scalar_sub: fp5_sub,
        scalar_neg: fp5_neg,
    }
}

#[cfg(feature = "fixture-oracle")]
fn byte_product(sums: ByteAccumulator, n: usize) -> Fp<5> {
    Fp::<5>::new(u64::from(sums.horizontal_product(n)))
}

#[cfg(feature = "fixture-oracle")]
fn packed5_c4_product(sums: Packed5, n: usize) -> Fp<5> {
    Fp::<5>::new(u64::from(c4_product(sums, active_mask(n))))
}

/// Product of the active packed lanes through the `F_5*` C4 reduction.
#[cfg(feature = "fixture-oracle")]
fn c4_product(sums: Packed5, active: u64) -> u8 {
    let (b0, b1, b2) = sums.to_raw_planes();
    let b0 = b0 & active;
    let b1 = b1 & active;
    let b2 = b2 & active;
    let zero_mask = !(b0 | b1 | b2) & active;
    if zero_mask != 0 {
        return 0;
    }

    // Generator 2 orders F_5* as 1, 2, 4, 3. Each selector includes a
    // positive input plane, so complements cannot admit inactive tail bits.
    let exponent = (b1 & !b0 & !b2).count_ones()
        + 2 * (b2 & !b0 & !b1).count_ones()
        + 3 * (b0 & b1 & !b2).count_ones();
    const EXPONENT_TO_VALUE: [u8; 4] = [1, 2, 4, 3];
    EXPONENT_TO_VALUE[(exponent % 4) as usize]
}

#[cfg(feature = "fixture-oracle")]
const fn active_mask(n: usize) -> u64 {
    debug_assert!(n <= MAX_ROWS);
    (1_u64 << n) - 1
}

#[cfg(feature = "fixture-oracle")]
#[inline]
fn add_mod(left: u8, right: u8) -> u8 {
    let sum = left + right;
    if sum >= FIELD_ORDER {
        sum - FIELD_ORDER
    } else {
        sum
    }
}

#[cfg(feature = "fixture-oracle")]
#[inline]
fn sub_mod(left: u8, right: u8) -> u8 {
    if left >= right {
        left - right
    } else {
        left + FIELD_ORDER - right
    }
}

#[cfg(feature = "fixture-oracle")]
#[inline]
fn mul_mod(left: u8, right: u8) -> u8 {
    (left * right) % FIELD_ORDER
}

#[cfg(feature = "fixture-oracle")]
fn fp5_add(left: Fp<5>, right: Fp<5>) -> Fp<5> {
    left + right
}

#[cfg(feature = "fixture-oracle")]
fn fp5_sub(left: Fp<5>, right: Fp<5>) -> Fp<5> {
    left - right
}

#[cfg(feature = "fixture-oracle")]
fn fp5_neg(value: Fp<5>) -> Fp<5> {
    -value
}

#[cfg(all(test, feature = "fixture-oracle"))]
mod tests {
    use gf2_algebra::permanent::permanent_ryser;

    use super::*;
    use crate::{
        fixtures::{FixtureCorpus, FixtureRequirement, DEFAULT_FIXTURE_SEED},
        MeasurementPath,
    };

    #[test]
    fn c4_log_population_product_is_exhaustive_for_three_lanes() {
        for first in 0..FIELD_ORDER {
            for second in 0..FIELD_ORDER {
                for third in 0..FIELD_ORDER {
                    let state = Packed5::zero()
                        .with_lane(0, Fp::<5>::new(u64::from(first)))
                        .with_lane(1, Fp::<5>::new(u64::from(second)))
                        .with_lane(2, Fp::<5>::new(u64::from(third)));
                    let expected = mul_mod(mul_mod(first, second), third);
                    assert_eq!(
                        c4_product(state, 0b111),
                        expected,
                        "product ({first}, {second}, {third})"
                    );
                }
            }
        }
    }

    #[test]
    fn f5_fixture_values_match_ryser_through_both_registered_paths() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        for fixture in corpus.fixtures().iter().filter(|fixture| {
            fixture.q() == u64::from(FIELD_ORDER) && fixture.n() <= MAX_HOST_FIXTURE_ORDER
        }) {
            let expected = permanent_ryser::<Fp<5>>(
                &fixture
                    .matrix_bytes()
                    .iter()
                    .map(|&value| Fp::<5>::new(u64::from(value)))
                    .collect::<Vec<_>>(),
                fixture.n(),
            )
            .value();
            assert_eq!(
                MeasurementPath::F5ByteControl.evaluate(fixture),
                Ok(expected),
                "byte control mismatch for {}",
                fixture.id()
            );
            assert_eq!(
                MeasurementPath::F5ThreePlane.evaluate(fixture),
                Ok(expected),
                "three-plane mismatch for {}",
                fixture.id()
            );
        }
    }

    #[test]
    fn f5_fixture_corpus_retains_required_admitted_boundaries() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        for requirement in [
            FixtureRequirement::GrayAdditionTransition,
            FixtureRequirement::GraySubtractionTransition,
            FixtureRequirement::ZeroContainingRowProduct,
        ] {
            assert!(corpus.fixtures().iter().any(|fixture| {
                fixture.q() == u64::from(FIELD_ORDER)
                    && fixture.n() <= MAX_HOST_FIXTURE_ORDER
                    && fixture.has_requirement(&requirement)
            }));
        }
        for exponent in 0..4 {
            assert!(corpus.fixtures().iter().any(|fixture| {
                fixture.q() == u64::from(FIELD_ORDER)
                    && fixture.n() <= MAX_HOST_FIXTURE_ORDER
                    && fixture
                        .has_requirement(&FixtureRequirement::NonzeroExponentClass { exponent })
            }));
        }
    }

    #[test]
    fn n63_structural_evidence_checks_both_accumulators_without_a_permanent() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let structural = corpus
            .fixtures()
            .iter()
            .find(|fixture| fixture.q() == u64::from(FIELD_ORDER) && fixture.n() == MAX_ROWS)
            .expect("the committed F_5 partial-word fixture must remain present");
        for result in [
            MeasurementPath::F5ByteControl.evaluate(structural),
            MeasurementPath::F5ThreePlane.evaluate(structural),
        ] {
            let error = result.expect_err("n=63 full permanent is intentionally infeasible");
            assert!(error.reason().contains("n <= 16"));
            assert!(error.reason().contains("exponential"));
        }

        let active = active_mask(MAX_ROWS);
        let mut bytes = ByteAccumulator {
            values: [1; MAX_ROWS],
        };
        assert_eq!(bytes.horizontal_product(MAX_ROWS), 1);
        bytes.values[MAX_ROWS - 1] = 0;
        assert_eq!(bytes.horizontal_product(MAX_ROWS), 0);

        let one = Packed5::one();
        assert_eq!(c4_product(one, active), 1, "tail bits must not contribute");
        let active_zero = one.with_lane(MAX_ROWS - 1, Fp::<5>::new(0));
        assert_eq!(c4_product(active_zero, active), 0);

        let partition = wave::partition_for_order(MAX_ROWS);
        assert_eq!(partition.interval(0).start, 0);
        assert_eq!(partition.interval(31).end, 1_u64 << MAX_ROWS);
    }
}
