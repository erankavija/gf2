//! CPU-oracle equivalence and zero-path reports for registered candidates.
//!
//! [`check_registered_candidates`] deliberately gets candidates only by
//! iterating [`MeasurementPath::ALL`].  A new path is therefore admitted the
//! moment it joins the study's canonical registry; there is no shadow list to
//! update in this correctness harness.

use std::collections::BTreeMap;

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::packed::packed7::Packed7Matrix;
use gf2_algebra::permanent::{
    permanent_bipedal3, permanent_bipedal5, permanent_bipedal7, permanent_ryser,
};
use gf2_core::gfp::Fp;

use crate::fixtures::{Fixture, FixtureCorpus};
use crate::{EvaluationResult, MeasurementPath};

/// State of one candidate at a single `(q, n)` corpus cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateCellStatus {
    /// Every candidate result equalled the Ryser oracle.
    Identical,
    /// At least one candidate result differed from the Ryser oracle.
    Mismatch,
    /// The candidate could not process any fixture in the cell.
    Unavailable {
        /// Explicit candidate-reported reason.
        reason: String,
    },
    /// The candidate ran only part of the cell and stated why it stopped.
    PartiallyUnavailable {
        /// Explicit candidate-reported reason.
        reason: String,
    },
}

/// Per-cell comparison of one registered candidate and the CPU oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateCell {
    candidate: &'static str,
    q: u64,
    n: usize,
    reference: &'static str,
    secondary_reference: Option<&'static str>,
    matrix_count: usize,
    compared_count: usize,
    mismatch_count: usize,
    secondary_reference_mismatch_count: usize,
    first_mismatch_fixture_id: Option<String>,
    status: CandidateCellStatus,
}

impl CandidateCell {
    /// Stable name of the registered candidate.
    #[must_use]
    pub const fn candidate(&self) -> &'static str {
        self.candidate
    }

    /// Field order of this corpus cell.
    #[must_use]
    pub const fn q(&self) -> u64 {
        self.q
    }

    /// Matrix order of this corpus cell.
    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Primary CPU oracle used for every returned candidate value.
    #[must_use]
    pub const fn reference(&self) -> &'static str {
        self.reference
    }

    /// Packed scalar reference checked in addition to Ryser, when supported.
    ///
    /// Returns `None` when this `(q, n)` cell has no packed scalar reference.
    /// The primary Ryser candidate row remains present for those cells.
    #[must_use]
    pub const fn secondary_reference(&self) -> Option<&'static str> {
        self.secondary_reference
    }

    /// Number of source matrices retained in the cell.
    #[must_use]
    pub const fn matrix_count(&self) -> usize {
        self.matrix_count
    }

    /// Number of matrices the candidate actually returned a value for.
    #[must_use]
    pub const fn compared_count(&self) -> usize {
        self.compared_count
    }

    /// Number of candidate values unequal to the Ryser oracle.
    #[must_use]
    pub const fn mismatch_count(&self) -> usize {
        self.mismatch_count
    }

    /// Number of packed-scalar values that disagreed with Ryser.
    ///
    /// This detects a bad second reference independently of the candidate
    /// comparison.  A nonzero count invalidates the cell's CPU reference
    /// evidence even if a candidate happens to equal Ryser.
    #[must_use]
    pub const fn secondary_reference_mismatch_count(&self) -> usize {
        self.secondary_reference_mismatch_count
    }

    /// Identifier of the first mismatching fixture, if any.
    #[must_use]
    pub fn first_mismatch_fixture_id(&self) -> Option<&str> {
        self.first_mismatch_fixture_id.as_deref()
    }

    /// Observable execution state for the cell.
    #[must_use]
    pub const fn status(&self) -> &CandidateCellStatus {
        &self.status
    }

    /// The explicit reason for an unavailable candidate, if it supplied one.
    #[must_use]
    pub fn unavailable_reason(&self) -> Option<&str> {
        match &self.status {
            CandidateCellStatus::Unavailable { reason }
            | CandidateCellStatus::PartiallyUnavailable { reason } => Some(reason),
            CandidateCellStatus::Identical | CandidateCellStatus::Mismatch => None,
        }
    }
}

/// Exact symbolic expectations for the two horizontal-product paths.
///
/// The expressions are retained instead of rounded floating-point values so a
/// report keeps the exact probability valid for large partial-word fixtures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZeroPathFrequency {
    q: u64,
    n: usize,
    observations: usize,
    observed_zero_fast_count: usize,
    observed_slow_count: usize,
}

impl ZeroPathFrequency {
    /// Field order of this corpus cell.
    #[must_use]
    pub const fn q(&self) -> u64 {
        self.q
    }

    /// Matrix order of this corpus cell.
    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Number of nonempty-subset probes observed for the cell.
    #[must_use]
    pub const fn observations(&self) -> usize {
        self.observations
    }

    /// Count whose row-product uses the zero fast path.
    #[must_use]
    pub const fn observed_zero_fast_count(&self) -> usize {
        self.observed_zero_fast_count
    }

    /// Count whose row-product takes the all-nonzero slow path.
    #[must_use]
    pub const fn observed_slow_count(&self) -> usize {
        self.observed_slow_count
    }

    /// Observed zero-fast-path frequency, if this cell has a nonempty subset.
    #[must_use]
    pub fn observed_zero_fast_frequency(&self) -> Option<f64> {
        ratio(self.observed_zero_fast_count, self.observations)
    }

    /// Observed all-nonzero slow-path frequency, if this cell has a subset.
    #[must_use]
    pub fn observed_slow_frequency(&self) -> Option<f64> {
        ratio(self.observed_slow_count, self.observations)
    }

    /// Exact marginal zero-fast expectation: `1 - ((q - 1) / q)^n`.
    #[must_use]
    pub fn zero_fast_expectation(&self) -> String {
        format!("1 - ({}/{})^{}", self.q - 1, self.q, self.n)
    }

    /// Exact marginal all-nonzero slow expectation: `((q - 1) / q)^n`.
    #[must_use]
    pub fn slow_expectation(&self) -> String {
        format!("({}/{})^{}", self.q - 1, self.q, self.n)
    }

    /// The two exact expectation expressions are complementary by definition.
    #[must_use]
    pub const fn expectations_are_complements(&self) -> bool {
        true
    }
}

/// Complete candidate and path-frequency result for one fixture corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EquivalenceReport {
    candidate_cells: Vec<CandidateCell>,
    path_frequencies: Vec<ZeroPathFrequency>,
}

impl EquivalenceReport {
    /// One retained row for every registry path and corpus cell.
    #[must_use]
    pub fn candidate_cells(&self) -> &[CandidateCell] {
        &self.candidate_cells
    }

    /// One zero-fast/slow observation row for every corpus cell.
    #[must_use]
    pub fn path_frequencies(&self) -> &[ZeroPathFrequency] {
        &self.path_frequencies
    }
}

/// Compare every registered candidate with [`permanent_ryser`].
///
/// A candidate that returns [`Unsupported`] is retained as an unavailable row
/// with the exact reason.  A candidate value is compared element-wise: every
/// fixture value is paired with the oracle value for that same fixture rather
/// than with an aggregate zero count.
#[must_use]
pub fn check_registered_candidates(corpus: &FixtureCorpus) -> EquivalenceReport {
    let fixtures: Vec<_> = corpus.fixtures().iter().collect();
    check_with_evaluator(&fixtures, |path, fixture| path.evaluate(fixture))
}

/// One packed CPU reference row checked against the generic Ryser oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuReferenceRow {
    reference: &'static str,
    q: u64,
    n: usize,
    matrix_count: usize,
    mismatch_count: usize,
    first_mismatch_fixture_id: Option<String>,
}

impl CpuReferenceRow {
    /// Packed CPU implementation used for this field.
    #[must_use]
    pub const fn reference(&self) -> &'static str {
        self.reference
    }

    /// Field order.
    #[must_use]
    pub const fn q(&self) -> u64 {
        self.q
    }

    /// Matrix order.
    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Number of matrices compared in this row.
    #[must_use]
    pub const fn matrix_count(&self) -> usize {
        self.matrix_count
    }

    /// Number of matrix values unequal to the generic oracle.
    #[must_use]
    pub const fn mismatch_count(&self) -> usize {
        self.mismatch_count
    }

    /// First mismatching fixture identifier, if one exists.
    #[must_use]
    pub fn first_mismatch_fixture_id(&self) -> Option<&str> {
        self.first_mismatch_fixture_id.as_deref()
    }
}

/// Cross-check packed CPU reference paths against Ryser for bounded fixtures.
///
/// `max_n` makes the ordinary test tier deliberately finite. Cells without a
/// packed scalar reference according to [`packed_reference_for`] are omitted;
/// they remain represented by [`check_registered_candidates`] with
/// [`CandidateCell::secondary_reference`] set to `None`. The full candidate
/// checker remains valid for every Ryser-supported order up to 63; callers
/// selecting a candidate at a larger order are responsible for the
/// corresponding exponential oracle work.
#[must_use]
pub fn check_cpu_reference_paths(corpus: &FixtureCorpus, max_n: usize) -> Vec<CpuReferenceRow> {
    let fixtures: Vec<_> = corpus.fixtures().iter().collect();
    let mut cells = corpus_cells(&fixtures);
    let mut rows = Vec::new();
    for ((q, n), fixtures) in &mut cells {
        if *n > max_n {
            continue;
        }
        let Some(reference) = packed_reference_for(*q, *n) else {
            continue;
        };
        let mut mismatch_count = 0;
        let mut first_mismatch_fixture_id = None;
        for fixture in fixtures.iter().copied() {
            let expected = oracle_value(fixture);
            let actual = packed_cpu_value(fixture);
            if actual != expected {
                mismatch_count += 1;
                first_mismatch_fixture_id.get_or_insert_with(|| fixture.id().to_owned());
            }
        }
        rows.push(CpuReferenceRow {
            reference,
            q: *q,
            n: *n,
            matrix_count: fixtures.len(),
            mismatch_count,
            first_mismatch_fixture_id,
        });
    }
    rows
}

fn check_with_evaluator<F>(fixtures: &[&Fixture], mut evaluate: F) -> EquivalenceReport
where
    F: FnMut(MeasurementPath, &Fixture) -> EvaluationResult,
{
    let cells = corpus_cells(fixtures);
    let mut candidate_cells = Vec::new();
    for path in MeasurementPath::ALL {
        for ((q, n), fixtures) in &cells {
            let mut compared_count = 0;
            let mut mismatch_count = 0;
            let mut secondary_reference_mismatch_count = 0;
            let mut first_mismatch_fixture_id = None;
            let mut unavailable_reason = None;
            for fixture in fixtures {
                match evaluate(path, fixture) {
                    Ok(actual) => {
                        compared_count += 1;
                        let expected = oracle_value(fixture);
                        if let Some(_secondary_reference) = packed_reference_for(*q, *n) {
                            if packed_cpu_value(fixture) != expected {
                                secondary_reference_mismatch_count += 1;
                            }
                        }
                        if actual != expected {
                            mismatch_count += 1;
                            first_mismatch_fixture_id
                                .get_or_insert_with(|| fixture.id().to_owned());
                        }
                    }
                    Err(reason) => {
                        unavailable_reason.get_or_insert_with(|| reason.reason().to_owned());
                    }
                }
            }
            let status = match (compared_count, unavailable_reason) {
                (0, Some(reason)) => CandidateCellStatus::Unavailable { reason },
                (_, Some(reason)) => CandidateCellStatus::PartiallyUnavailable { reason },
                (_, None) if mismatch_count == 0 => CandidateCellStatus::Identical,
                (_, None) => CandidateCellStatus::Mismatch,
            };
            candidate_cells.push(CandidateCell {
                candidate: path.name(),
                q: *q,
                n: *n,
                reference: "permanent_ryser",
                secondary_reference: packed_reference_for(*q, *n),
                matrix_count: fixtures.len(),
                compared_count,
                mismatch_count,
                secondary_reference_mismatch_count,
                first_mismatch_fixture_id,
                status,
            });
        }
    }
    let path_frequencies = cells
        .into_iter()
        .map(|((q, n), fixtures)| observe_zero_paths(q, n, &fixtures))
        .collect();
    EquivalenceReport {
        candidate_cells,
        path_frequencies,
    }
}

fn corpus_cells<'a>(fixtures: &[&'a Fixture]) -> BTreeMap<(u64, usize), Vec<&'a Fixture>> {
    let mut cells = BTreeMap::new();
    for fixture in fixtures {
        cells
            .entry((fixture.q(), fixture.n()))
            .or_insert_with(Vec::new)
            .push(*fixture);
    }
    cells
}

fn observe_zero_paths(q: u64, n: usize, fixtures: &[&Fixture]) -> ZeroPathFrequency {
    // Probe the fixed, nonempty singleton subset {column 0} only on the
    // domain-separated uniform fixtures.  It has the exact marginal stated
    // below.  Structural fixtures remain in the candidate check but cannot be
    // folded into this frequency without biasing the reported observation.
    let mut observed_zero_fast_count = 0;
    let mut observed_slow_count = 0;
    for fixture in fixtures.iter().copied().filter(|fixture| {
        fixture.has_requirement(&crate::fixtures::FixtureRequirement::UniformSample)
    }) {
        if n == 0 {
            continue;
        }
        let has_zero_sum = (0..n).any(|row| fixture.matrix_bytes()[row * n] == 0);
        if has_zero_sum {
            observed_zero_fast_count += 1;
        } else {
            observed_slow_count += 1;
        }
    }
    ZeroPathFrequency {
        q,
        n,
        observations: observed_zero_fast_count + observed_slow_count,
        observed_zero_fast_count,
        observed_slow_count,
    }
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}

fn oracle_value(fixture: &Fixture) -> u64 {
    match fixture.q() {
        3 => permanent_ryser::<Fp<3>>(&fp_entries::<3>(fixture), fixture.n()).value(),
        5 => permanent_ryser::<Fp<5>>(&fp_entries::<5>(fixture), fixture.n()).value(),
        7 => permanent_ryser::<Fp<7>>(&fp_entries::<7>(fixture), fixture.n()).value(),
        _ => unreachable!("Fixture validates its field order"),
    }
}

fn packed_cpu_value(fixture: &Fixture) -> u64 {
    match fixture.q() {
        3 => permanent_bipedal3(&Bipedal3Matrix::from_row_major(
            &fp_entries::<3>(fixture),
            fixture.n(),
            fixture.n(),
        ))
        .value(),
        5 => permanent_bipedal5(&Packed5Matrix::from_row_major(
            &fp_entries::<5>(fixture),
            fixture.n(),
            fixture.n(),
        ))
        .value(),
        7 => permanent_bipedal7(&Packed7Matrix::from_row_major(
            &fp_entries::<7>(fixture),
            fixture.n(),
            fixture.n(),
        ))
        .value(),
        _ => unreachable!("Fixture validates its field order"),
    }
}

fn fp_entries<const Q: u64>(fixture: &Fixture) -> Vec<Fp<Q>> {
    fixture
        .matrix_bytes()
        .iter()
        .map(|&value| Fp::<Q>::new(u64::from(value)))
        .collect()
}

fn packed_reference_for(q: u64, n: usize) -> Option<&'static str> {
    match q {
        3 if n <= 63 => Some("permanent_bipedal3"),
        5 if n <= 63 => Some("permanent_bipedal5"),
        7 if n <= 16 => Some("permanent_bipedal7"),
        3 | 5 | 7 => None,
        _ => unreachable!("Fixture validates its field order"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::fixtures::DEFAULT_FIXTURE_SEED;
    use crate::Unsupported;

    fn parity_shard(corpus: &FixtureCorpus, shard: usize) -> Vec<&Fixture> {
        corpus
            .fixtures()
            .iter()
            .enumerate()
            .filter_map(|(index, fixture)| (index % 2 == shard).then_some(fixture))
            .collect()
    }

    fn assert_parity_shards_partition_the_corpus(corpus: &FixtureCorpus) {
        let first: BTreeSet<_> = parity_shard(corpus, 0)
            .into_iter()
            .map(Fixture::id)
            .collect();
        let second: BTreeSet<_> = parity_shard(corpus, 1)
            .into_iter()
            .map(Fixture::id)
            .collect();
        let complete: BTreeSet<_> = corpus.fixtures().iter().map(Fixture::id).collect();

        assert!(first.is_disjoint(&second));
        assert_eq!(first.len() + second.len(), complete.len());
        assert_eq!(
            first.union(&second).copied().collect::<BTreeSet<_>>(),
            complete
        );
    }

    fn assert_candidate_report_semantics(report: &EquivalenceReport) {
        assert_eq!(
            report.candidate_cells().len(),
            MeasurementPath::ALL.len() * report.path_frequencies().len(),
            "every registered candidate must receive one result per retained corpus cell"
        );
        for row in report.candidate_cells() {
            assert_eq!(row.reference(), "permanent_ryser");
            assert_eq!(
                row.mismatch_count(),
                0,
                "{} q={} n={} first={:?}",
                row.candidate(),
                row.q(),
                row.n(),
                row.first_mismatch_fixture_id()
            );
            assert_eq!(
                row.secondary_reference_mismatch_count(),
                0,
                "{} q={} n={} packed reference must agree with Ryser",
                row.candidate(),
                row.q(),
                row.n()
            );
            match row.status() {
                CandidateCellStatus::Identical => {
                    assert!(row.compared_count() > 0);
                    assert!(row.unavailable_reason().is_none());
                }
                CandidateCellStatus::Unavailable { .. } => {
                    assert_eq!(row.compared_count(), 0);
                    assert!(row
                        .unavailable_reason()
                        .is_some_and(|reason| !reason.is_empty()));
                }
                CandidateCellStatus::PartiallyUnavailable { .. } => {
                    assert!(row.compared_count() > 0);
                    assert!(row
                        .unavailable_reason()
                        .is_some_and(|reason| !reason.is_empty()));
                }
                CandidateCellStatus::Mismatch => panic!(
                    "{} q={} n={} disagreed with the CPU oracle at {:?}",
                    row.candidate(),
                    row.q(),
                    row.n(),
                    row.first_mismatch_fixture_id()
                ),
            }
        }
        for n in [20, 24] {
            let rows: Vec<_> = report
                .candidate_cells()
                .iter()
                .filter(|row| row.q() == 7 && row.n() == n)
                .collect();
            assert_eq!(rows.len(), MeasurementPath::ALL.len());
            assert!(rows.iter().all(|row| {
                row.reference() == "permanent_ryser" && row.secondary_reference().is_none()
            }));
        }
        for n in [16, 20, 24] {
            let accumulator = report
                .candidate_cells()
                .iter()
                .find(|row| {
                    row.candidate() == "f7-three-plane-accumulator" && row.q() == 7 && row.n() == n
                })
                .expect("each shard must retain every required F_7 order");
            assert_eq!(accumulator.status(), &CandidateCellStatus::Identical);
            assert_eq!(accumulator.compared_count(), accumulator.matrix_count());
        }
        for frequency in report.path_frequencies() {
            assert!(frequency.expectations_are_complements());
            assert!(frequency.zero_fast_expectation().starts_with("1 - "));
            assert!(frequency.slow_expectation().starts_with('('));
            assert_eq!(
                frequency.observed_zero_fast_count() + frequency.observed_slow_count(),
                frequency.observations()
            );
        }
    }

    fn assert_registered_candidate_parity_shard(shard: usize) {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        assert_parity_shards_partition_the_corpus(&corpus);
        let fixtures = parity_shard(&corpus, shard);
        let report = check_with_evaluator(&fixtures, |path, fixture| path.evaluate(fixture));
        assert_candidate_report_semantics(&report);
    }

    #[test]
    fn registered_candidates_shard_even_is_oracle_identical() {
        assert_registered_candidate_parity_shard(0);
    }

    #[test]
    fn registered_candidates_shard_odd_is_oracle_identical() {
        assert_registered_candidate_parity_shard(1);
    }

    #[test]
    fn checker_counts_the_first_per_matrix_mismatch() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let fixtures: Vec<_> = corpus.fixtures().iter().collect();
        let report = check_with_evaluator(&fixtures, |_path, fixture| {
            if fixture.n() <= 1 {
                Ok(0)
            } else {
                Err(Unsupported::new(
                    "test candidate only accepts tiny fixtures",
                ))
            }
        });
        let row = report
            .candidate_cells()
            .iter()
            .find(|row| row.candidate() == "wave-gf3" && row.q() == 3 && row.n() == 0)
            .expect("the canonical registry must reach q=3 n=0");
        assert_eq!(row.mismatch_count(), 1);
        assert_eq!(row.first_mismatch_fixture_id(), Some("q3-n0-empty"));
    }

    #[test]
    fn checker_accepts_an_executable_candidate_that_matches_ryser() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let fixtures: Vec<_> = corpus.fixtures().iter().collect();
        let report = check_with_evaluator(&fixtures, |_path, fixture| {
            if fixture.n() == 1 {
                Ok(oracle_value(fixture))
            } else {
                Err(Unsupported::new(
                    "test candidate only accepts singleton fixtures",
                ))
            }
        });
        let row = report
            .candidate_cells()
            .iter()
            .find(|row| row.candidate() == "wave-gf3" && row.q() == 3 && row.n() == 1)
            .expect("the canonical registry must reach q=3 n=1");
        assert_eq!(row.status(), &CandidateCellStatus::Identical);
        assert!(row.compared_count() > 0);
        assert_eq!(row.mismatch_count(), 0);
        assert!(row.unavailable_reason().is_none());
    }
}
