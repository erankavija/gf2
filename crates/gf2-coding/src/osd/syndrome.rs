//! Syndrome-domain ordered-statistics decoding for parity-check matrices.
//!
//! The adapter computes `s = H yᵀ`, solves `H eᵀ = s`, and searches error
//! patterns with ascending-magnitude pivots.  Free coordinates are enumerated
//! in deterministic order, equal-cost ties keep enumeration order, and
//! candidates have cost `sum_i e_i |L_i|`.  The global order-`m` minimum is
//! attained only in an uncapped exhaustive configuration.
//!
//! Only the parity-check semantics live here.  The ordered elimination, the
//! bounded pattern enumeration, the soft ranking, and the work metadata all
//! come from the shared [`MostReliableBasis`] and [`reprocess`] engine.

use std::fmt;

use gf2_core::{BitMatrix, BitVec};

use crate::llr::Llr;

use super::{
    reprocess, ColumnPreference, MostReliableBasis, OsdCandidate, OsdConfig, OsdEngineError,
    OsdOutcome, OsdSemantics, OsdWork, ReprocessedColumns,
};

/// Parity-check-domain OSD correction of a failed hard word.
///
/// The corrector holds one parity-check matrix `H` and an [`OsdConfig`].  For a
/// failed hard word `y` and posterior LLRs `L` it forms the syndrome
/// `s = H yᵀ` and searches the affine solution set of `H eᵀ = s`.  The shared
/// engine pivots its ordered elimination on ascending posterior magnitude, so
/// the pivots are the least reliable coordinates and the reprocessed free
/// coordinates the most reliable ones.  Each order-`m` pattern assigns the free
/// coordinates and pivot back-substitution completes it to one error pattern
/// `e`; order zero is the all-zero free assignment.  Candidates are scored
/// against the all-zero reference word, so the soft metric is exactly
/// `sum_i e_i |L_i|`, and an equal metric keeps the earlier candidate in
/// enumeration order.
///
/// A candidate reconstructed over a consistent basis solves `H eᵀ = s` by
/// construction, so `y ⊕ e` has zero syndrome; the corrector returns a
/// corrected word exactly when the search tested a candidate.  The two runs
/// that test none are a syndrome outside the reachable row space, reported as
/// [`super::OsdTermination::InconsistentTransform`], and a zero candidate cap.
/// A rank-deficient `H` with a consistent syndrome stays searchable: rank `r`
/// leaves `n - r` free coordinates, which carry the whole coset search.
///
/// Use [`Self::correct`] for a failed hard word and [`Self::solve`] for a
/// caller-supplied syndrome.
#[derive(Clone, Debug)]
pub struct SyndromeOsdCorrector {
    parity_check: BitMatrix,
    config: OsdConfig,
}

impl SyndromeOsdCorrector {
    /// Creates a syndrome-domain corrector for `parity_check` and `config`.
    pub fn new(parity_check: BitMatrix, config: OsdConfig) -> Self {
        Self {
            parity_check,
            config,
        }
    }

    /// Returns the parity-check matrix retained by the corrector.
    pub const fn parity_check(&self) -> &BitMatrix {
        &self.parity_check
    }

    /// Returns the OSD search configuration.
    pub const fn config(&self) -> OsdConfig {
        self.config
    }

    /// Returns the number of codeword columns in the parity-check matrix.
    pub fn n(&self) -> usize {
        self.parity_check.cols()
    }

    /// Computes `s = H yᵀ`, then returns the lowest-cost correction of
    /// `hard_word` under the supplied posterior LLRs.
    ///
    /// [`SyndromeOsdResult::syndrome`] is that computed syndrome, a tested
    /// engine candidate is an error pattern `e`, and the corrected word is
    /// `y ⊕ e`.  [`Self::solve`] is the same search against a caller-supplied
    /// syndrome.
    ///
    /// # Errors
    ///
    /// Returns [`SyndromeOsdError::HardWordLength`] when `hard_word` does not
    /// have one entry per parity-check column, and
    /// [`SyndromeOsdError::Engine`] wrapping
    /// [`OsdEngineError::MagnitudeLength`] when `llrs` does not, or
    /// [`OsdEngineError::Patterns`] when the checked uncapped candidate bound
    /// for the free dimension and configured order does not fit in `usize`.
    ///
    /// # Panics
    ///
    /// Panics if any LLR has a NaN magnitude.
    ///
    /// # Complexity
    ///
    /// One ordered elimination plus the shared engine's O(candidates ×
    /// (order + n) / 64) reconstruction and ranking work.  The corrector
    /// stores O(rows × n) matrix bits and each call uses O(rows × n)
    /// elimination storage plus O((n − rank) × n) candidate-delta bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::osd::{OsdConfig, OsdTermination, SyndromeOsdCorrector};
    /// use gf2_coding::LinearBlockCode;
    /// use gf2_core::BitVec;
    ///
    /// // Hamming(7,4): the all-zero codeword with coordinate 4 flipped, and a
    /// // posterior that is least confident exactly there.
    /// let code = LinearBlockCode::hamming(3);
    /// let corrector =
    ///     SyndromeOsdCorrector::new(code.parity_check().unwrap().clone(), OsdConfig::new(1));
    ///
    /// let mut llrs = [Llr::new(2.0); 7];
    /// llrs[4] = Llr::new(-0.5);
    /// let mut hard_word = BitVec::zeros(7);
    /// hard_word.set(4, true);
    ///
    /// let result = corrector.correct(&hard_word, &llrs).unwrap();
    /// assert_eq!(result.corrected_word().unwrap(), &BitVec::zeros(7));
    /// assert_eq!(result.error_pattern().unwrap().count_ones(), 1);
    /// assert_eq!(result.work().rank(), 3);
    /// assert_eq!(result.work().theoretical_candidates(), 5); // 1 + (7 - 3)
    /// assert_eq!(result.work().termination(), OsdTermination::Exhaustive);
    /// ```
    pub fn correct(
        &self,
        hard_word: &BitVec,
        llrs: &[Llr],
    ) -> Result<SyndromeOsdResult, SyndromeOsdError> {
        if hard_word.len() != self.n() {
            return Err(SyndromeOsdError::HardWordLength {
                expected: self.n(),
                actual: hard_word.len(),
            });
        }

        let syndrome = self.parity_check.matvec(hard_word);
        let outcome = self.solve(&syndrome, llrs)?;
        let corrected_word = outcome.best().map(|candidate| {
            let mut corrected = hard_word.clone();
            corrected.bit_xor_into(candidate.word());
            corrected
        });

        Ok(SyndromeOsdResult {
            corrected_word,
            syndrome,
            outcome,
        })
    }

    /// Returns the shared-engine outcome of solving `H eᵀ = syndrome`.
    ///
    /// This is the general syndrome-domain surface, for a syndrome that does
    /// not come from a hard word of its own — a measured one, for instance.
    /// The best candidate's word is the lowest-cost error pattern `e` and
    /// [`OsdOutcome::work`] carries the search metadata.  A syndrome the row
    /// transform places outside the reachable row space has no solution, so
    /// the run generates nothing and reports
    /// [`super::OsdTermination::InconsistentTransform`].
    ///
    /// # Errors
    ///
    /// Returns [`SyndromeOsdError::Engine`] wrapping
    /// [`OsdEngineError::MagnitudeLength`] when `llrs` does not carry one
    /// entry per parity-check column, [`OsdEngineError::Elimination`] when
    /// `syndrome` does not carry one entry per parity-check row, and
    /// [`OsdEngineError::Patterns`] when the checked uncapped candidate bound
    /// for the free dimension and configured order does not fit in `usize`.
    ///
    /// # Panics
    ///
    /// Panics if any LLR has a NaN magnitude.
    ///
    /// # Complexity
    ///
    /// One ordered elimination plus the shared engine's O(candidates ×
    /// (order + n) / 64) reconstruction and ranking work, with O(rows × n)
    /// elimination storage and O((n − rank) × n) candidate-delta storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::osd::{OsdConfig, OsdTermination, SyndromeOsdCorrector};
    /// use gf2_core::BitVec;
    ///
    /// let matrix = gf2_core::bitmatrix![1, 0, 1; 0, 1, 1; 1, 1, 0];
    /// let mut syndrome = BitVec::zeros(3);
    /// syndrome.set(2, true);
    /// let result = SyndromeOsdCorrector::new(matrix, OsdConfig::new(1))
    ///     .solve(&syndrome, &[Llr::new(1.0); 3])
    ///     .unwrap();
    /// assert!(result.best().is_none());
    /// assert_eq!(result.work().rank(), 2);
    /// assert_eq!(result.work().termination(), OsdTermination::InconsistentTransform);
    /// ```
    pub fn solve(&self, syndrome: &BitVec, llrs: &[Llr]) -> Result<OsdOutcome, SyndromeOsdError> {
        let magnitudes: Vec<f32> = llrs.iter().map(|llr| llr.magnitude()).collect();
        let zero_word = BitVec::zeros(self.n());
        let basis = MostReliableBasis::build(
            &self.parity_check,
            &magnitudes,
            &zero_word,
            Some(syndrome),
            ColumnPreference::LeastReliableFirst,
        )?;
        Ok(reprocess(&basis, &SyndromeSemantics, self.config)?)
    }
}

/// A syndrome-domain OSD decision together with the shared engine outcome.
///
/// [`Self::work`] directly exposes the engine's rank, uncapped candidate
/// bound, generated-pattern count, tested-candidate count, elimination count,
/// and termination reason.  No adapter-local search counters are maintained.
///
/// [`Self::syndrome`] is the syndrome of the failed hard word,
/// [`Self::error_pattern`] the selected solution `e` of `H eᵀ = s`, and
/// [`Self::corrected_word`] their combination `y ⊕ e`.  The decision is absent
/// exactly in the two runs [`SyndromeOsdCorrector`] describes: an inconsistent
/// transform and a zero candidate cap.
#[derive(Clone, Debug, PartialEq)]
pub struct SyndromeOsdResult {
    corrected_word: Option<BitVec>,
    syndrome: BitVec,
    outcome: OsdOutcome,
}

impl SyndromeOsdResult {
    /// Returns the corrected word `y ⊕ e`, if a candidate was tested.
    pub fn corrected_word(&self) -> Option<&BitVec> {
        self.corrected_word.as_ref()
    }

    /// Returns the selected error pattern, if a candidate was tested.
    pub fn error_pattern(&self) -> Option<&BitVec> {
        self.outcome.best().map(OsdCandidate::word)
    }

    /// Returns the syndrome `s = H yᵀ` associated with this result.
    pub fn syndrome(&self) -> &BitVec {
        &self.syndrome
    }

    /// Returns the selected engine candidate, if one was tested.
    pub fn candidate(&self) -> Option<&OsdCandidate> {
        self.outcome.best()
    }

    /// Returns the shared engine work counters and termination metadata.
    pub const fn work(&self) -> &OsdWork {
        self.outcome.work()
    }

    /// Returns the complete shared-engine outcome.
    pub const fn outcome(&self) -> &OsdOutcome {
        &self.outcome
    }

    /// Consumes the result and returns its corrected word, if any.
    pub fn into_corrected_word(self) -> Option<BitVec> {
        self.corrected_word
    }

    /// Consumes the result and returns the complete shared-engine outcome.
    pub fn into_outcome(self) -> OsdOutcome {
        self.outcome
    }
}

/// Errors returned by the syndrome-domain OSD adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyndromeOsdError {
    /// The failed hard word does not carry one entry per parity-check column.
    HardWordLength {
        /// Number of parity-check columns.
        expected: usize,
        /// Number of entries in the failed hard word.
        actual: usize,
    },
    /// The shared OSD engine rejected the search inputs.
    Engine(OsdEngineError),
}

impl fmt::Display for SyndromeOsdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HardWordLength { expected, actual } => write!(
                formatter,
                "failed hard word has {actual} entries, expected one per column ({expected})"
            ),
            Self::Engine(error) => write!(formatter, "OSD reprocessing failed: {error}"),
        }
    }
}

impl std::error::Error for SyndromeOsdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::HardWordLength { .. } => None,
        }
    }
}

impl From<OsdEngineError> for SyndromeOsdError {
    fn from(error: OsdEngineError) -> Self {
        Self::Engine(error)
    }
}

/// Syndrome-coset candidate reconstruction for the shared engine.
struct SyndromeSemantics;

impl OsdSemantics for SyndromeSemantics {
    fn reprocessed_columns(&self) -> ReprocessedColumns {
        ReprocessedColumns::Free
    }

    fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
        let elimination = basis.elimination();
        let mut candidate = BitVec::zeros(basis.reference().len());
        // The pivot columns carry the identity in the reduced matrix, so the
        // all-zero free assignment back-substitutes to the transformed
        // right-hand side.  A homogeneous basis, which this adapter never
        // builds, has the all-zero particular solution.
        if let Some(transformed_rhs) = basis.transformed_rhs() {
            for (row, &column) in elimination.selected_cols.iter().enumerate() {
                candidate.set(column, transformed_rhs.get(row));
            }
        }
        candidate
    }

    fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
        let elimination = basis.elimination();
        basis
            .free_cols()
            .iter()
            .map(|&free_column| {
                // Setting one free coordinate changes each pivot coordinate by
                // that free column's entry in the pivot's reduced row, which is
                // the back-substitution of this single-coordinate assignment.
                let mut delta = BitVec::zeros(basis.reference().len());
                delta.set(free_column, true);
                for (row, &column) in elimination.selected_cols.iter().enumerate() {
                    if elimination.reduced.get(row, free_column) {
                        delta.set(column, true);
                    }
                }
                delta
            })
            .collect()
    }

    fn accepts(&self, _candidate: &BitVec) -> bool {
        // Over a consistent basis every reconstructed candidate satisfies
        // R e = t, hence U H e = U s; U is invertible, so H e = s and the
        // corrected word y ⊕ e has zero syndrome.
        true
    }
}

#[cfg(test)]
mod tests {
    use gf2_core::alg::rref::OrderedEliminationError;
    use gf2_core::{BitMatrix, BitVec};

    use crate::llr::Llr;

    use super::{SyndromeOsdCorrector, SyndromeOsdError};
    use crate::osd::{
        ColumnPreference, MostReliableBasis, OsdConfig, OsdEngineError, OsdTermination,
    };

    fn bitvec(mask: usize, len: usize) -> BitVec {
        let mut word = BitVec::zeros(len);
        for bit in 0..len {
            word.set(bit, (mask >> bit) & 1 == 1);
        }
        word
    }

    fn fixture_a() -> (BitMatrix, [Llr; 4], BitVec) {
        (
            gf2_core::bitmatrix![1, 0, 1, 1; 0, 1, 1, 1],
            [
                Llr::new(-1.0),
                Llr::new(-1.5),
                Llr::new(2.0),
                Llr::new(10.0),
            ],
            bitvec(0b0011, 4),
        )
    }

    fn rank_deficient_matrix() -> BitMatrix {
        gf2_core::bitmatrix![1, 0, 1, 1; 0, 1, 1, 1; 1, 1, 0, 0]
    }

    fn magnitudes(llrs: &[Llr]) -> Vec<f32> {
        llrs.iter().map(|llr| llr.magnitude()).collect()
    }

    /// Returns the minimum magnitude cost over every solution of `H eᵀ = s`,
    /// together with the number of solutions attaining it, by enumerating all
    /// `2^n` words.
    fn brute_force_minimum(
        matrix: &BitMatrix,
        syndrome: &BitVec,
        magnitudes: &[f32],
    ) -> Option<(f64, usize)> {
        let mut best: Option<(f64, usize)> = None;
        for mask in 0..(1usize << matrix.cols()) {
            let candidate = bitvec(mask, matrix.cols());
            if matrix.matvec(&candidate) != *syndrome {
                continue;
            }
            let metric: f64 = magnitudes
                .iter()
                .enumerate()
                .filter(|&(column, _)| candidate.get(column))
                .map(|(_, &magnitude)| f64::from(magnitude))
                .sum();
            best = Some(match best {
                None => (metric, 1),
                Some((standing, _)) if metric < standing => (metric, 1),
                Some((standing, attaining)) if metric == standing => (standing, attaining + 1),
                Some(standing) => standing,
            });
        }
        best
    }

    fn assert_back_substitution_identity(
        matrix: &BitMatrix,
        syndrome: &BitVec,
        magnitudes: &[f32],
        error: &BitVec,
    ) {
        let basis = MostReliableBasis::build(
            matrix,
            magnitudes,
            &BitVec::zeros(matrix.cols()),
            Some(syndrome),
            ColumnPreference::LeastReliableFirst,
        )
        .unwrap();
        let elimination = basis.elimination();
        let transformed_rhs = basis.transformed_rhs().unwrap();
        for row in 0..elimination.rank {
            let free_sum = basis
                .free_cols()
                .iter()
                .filter(|&&column| elimination.reduced.get(row, column) && error.get(column))
                .fold(false, |sum, _| !sum);
            assert_eq!(
                error.get(elimination.selected_cols[row]),
                transformed_rhs.get(row) ^ free_sum
            );
        }
    }

    #[test]
    fn order_zero_completes_the_all_zero_free_pattern_by_back_substitution() {
        let (matrix, llrs, hard_word) = fixture_a();
        let corrector = SyndromeOsdCorrector::new(matrix.clone(), OsdConfig::new(0));
        let result = corrector.correct(&hard_word, &llrs).unwrap();

        assert_eq!(result.error_pattern().unwrap(), &bitvec(0b0011, 4));
        assert_eq!(result.candidate().unwrap().pattern(), &[] as &[usize]);
        assert_eq!(result.candidate().unwrap().metric(), 2.5);
        assert_eq!(result.corrected_word().unwrap(), &BitVec::zeros(4));
        assert_eq!(
            matrix.matvec(result.corrected_word().unwrap()).count_ones(),
            0
        );
    }

    #[test]
    fn order_one_selects_the_single_free_coordinate_flip() {
        let (matrix, llrs, hard_word) = fixture_a();
        let corrector = SyndromeOsdCorrector::new(matrix.clone(), OsdConfig::new(1));
        let result = corrector.correct(&hard_word, &llrs).unwrap();

        assert_eq!(result.candidate().unwrap().pattern(), &[0]);
        assert_eq!(result.error_pattern().unwrap(), &bitvec(0b0100, 4));
        assert_eq!(result.candidate().unwrap().metric(), 2.0);
        assert_eq!(result.corrected_word().unwrap(), &bitvec(0b0111, 4));
        assert_eq!(
            matrix.matvec(result.corrected_word().unwrap()).count_ones(),
            0
        );
        let work = result.work();
        assert_eq!(work.rank(), 2);
        assert_eq!(work.theoretical_candidates(), 3);
        assert_eq!(work.generated_patterns(), 3);
        assert_eq!(work.tested_candidates(), 3);
        assert_eq!(work.eliminations(), 1);
        assert_eq!(work.termination(), OsdTermination::Exhaustive);
    }

    #[test]
    fn pivots_follow_ascending_posterior_magnitude() {
        let (matrix, llrs, hard_word) = fixture_a();
        let magnitudes = magnitudes(&llrs);
        let syndrome = matrix.matvec(&hard_word);
        let basis = MostReliableBasis::build(
            &matrix,
            &magnitudes,
            &BitVec::zeros(4),
            Some(&syndrome),
            ColumnPreference::LeastReliableFirst,
        )
        .unwrap();

        assert_eq!(basis.elimination().selected_cols, [0, 1]);
        assert_eq!(basis.free_cols(), &[2, 3]);
        assert!(basis
            .elimination()
            .selected_cols
            .windows(2)
            .all(|pair| magnitudes[pair[0]] <= magnitudes[pair[1]]));

        let tied = [1.0, 1.0, 2.0, 10.0];
        let tied_basis = MostReliableBasis::build(
            &matrix,
            &tied,
            &BitVec::zeros(4),
            Some(&syndrome),
            ColumnPreference::LeastReliableFirst,
        )
        .unwrap();
        assert_eq!(tied_basis.preference_order(), &[0, 1, 2, 3]);
        assert_eq!(tied_basis.elimination().selected_cols, [0, 1]);
    }

    #[test]
    fn each_returned_completion_satisfies_the_pivot_back_substitution_identity() {
        let (full_rank, llrs, _) = fixture_a();
        let rank_deficient = rank_deficient_matrix();
        let magnitudes = magnitudes(&llrs);

        for matrix in [&full_rank, &rank_deficient] {
            let corrector = SyndromeOsdCorrector::new(matrix.clone(), OsdConfig::new(1));
            for mask in 0..16 {
                let hard_word = bitvec(mask, 4);
                let result = corrector.correct(&hard_word, &llrs).unwrap();
                assert_back_substitution_identity(
                    matrix,
                    result.syndrome(),
                    &magnitudes,
                    result.error_pattern().unwrap(),
                );
                assert_eq!(*result.syndrome(), matrix.matvec(&hard_word));
            }
        }
    }

    #[test]
    fn rank_deficient_consistent_system_matches_the_full_rank_result() {
        let (_, llrs, hard_word) = fixture_a();
        let matrix = rank_deficient_matrix();
        let corrector = SyndromeOsdCorrector::new(matrix, OsdConfig::new(1));
        let result = corrector.correct(&hard_word, &llrs).unwrap();

        assert_eq!(result.work().rank(), 2);
        assert_eq!(result.corrected_word().unwrap(), &bitvec(0b0111, 4));
        assert_eq!(result.candidate().unwrap().metric(), 2.0);
    }

    #[test]
    fn inconsistent_syndrome_is_reported_without_a_correction() {
        let matrix = rank_deficient_matrix();
        let corrector = SyndromeOsdCorrector::new(matrix, OsdConfig::new(4));
        let result = corrector
            .solve(&bitvec(0b0111, 3), &[Llr::new(1.0); 4])
            .unwrap();

        assert!(result.best().is_none());
        assert_eq!(result.work().rank(), 2);
        assert_eq!(result.work().theoretical_candidates(), 4);
        assert_eq!(result.work().generated_patterns(), 0);
        assert_eq!(result.work().tested_candidates(), 0);
        assert_eq!(
            result.work().termination(),
            OsdTermination::InconsistentTransform
        );
    }

    #[test]
    fn uncapped_search_matches_exhaustive_magnitude_cost_error_search() {
        let (full_rank, _, _) = fixture_a();
        let rank_deficient = rank_deficient_matrix();
        for matrix in [&full_rank, &rank_deficient] {
            for magnitudes in [[1.0, 1.5, 2.0, 10.0], [1.0, 1.0, 2.0, 2.0]] {
                let llrs: Vec<_> = magnitudes.iter().copied().map(Llr::new).collect();
                let corrector = SyndromeOsdCorrector::new((*matrix).clone(), OsdConfig::new(4));
                for mask in 0..16 {
                    let hard_word = bitvec(mask, 4);
                    let syndrome = matrix.matvec(&hard_word);
                    let result = corrector.correct(&hard_word, &llrs).unwrap();
                    let (minimum, _) = brute_force_minimum(matrix, &syndrome, &magnitudes).unwrap();

                    assert_eq!(result.candidate().unwrap().metric(), minimum);
                    assert_eq!(matrix.matvec(result.error_pattern().unwrap()), syndrome);
                    assert_eq!(
                        matrix.matvec(result.corrected_word().unwrap()).count_ones(),
                        0
                    );
                }
            }
        }
    }

    #[test]
    fn exhaustive_solve_matches_brute_force_for_every_syndrome_class() {
        let matrix = rank_deficient_matrix();
        let magnitudes = [1.0, 1.5, 2.0, 10.0];
        let llrs: Vec<_> = magnitudes.iter().copied().map(Llr::new).collect();
        let corrector = SyndromeOsdCorrector::new(matrix.clone(), OsdConfig::new(4));
        for mask in 0..8 {
            let syndrome = bitvec(mask, 3);
            let result = corrector.solve(&syndrome, &llrs).unwrap();
            match brute_force_minimum(&matrix, &syndrome, &magnitudes) {
                Some((minimum, _)) => {
                    assert_eq!(result.best().unwrap().metric(), minimum);
                    assert_eq!(matrix.matvec(result.best().unwrap().word()), syndrome);
                }
                None => {
                    assert!(result.best().is_none());
                    assert_eq!(
                        result.work().termination(),
                        OsdTermination::InconsistentTransform
                    );
                }
            }
        }
    }

    #[test]
    fn tied_costs_keep_the_earlier_candidate_in_enumeration_order() {
        let (matrix, _, hard_word) = fixture_a();
        let llrs = [Llr::new(1.0), Llr::new(1.0), Llr::new(2.0), Llr::new(10.0)];
        let syndrome = matrix.matvec(&hard_word);
        let (minimum, attaining) =
            brute_force_minimum(&matrix, &syndrome, &magnitudes(&llrs)).unwrap();
        assert!(
            attaining > 1,
            "the fixture must offer competing minimum-cost solutions"
        );

        let corrector = SyndromeOsdCorrector::new(matrix, OsdConfig::new(1));
        let first = corrector.correct(&hard_word, &llrs).unwrap();
        let second = corrector.correct(&hard_word, &llrs).unwrap();

        assert_eq!(first.candidate().unwrap().metric(), minimum);
        assert_eq!(first.candidate().unwrap().generation(), 0);
        assert_eq!(first.candidate(), second.candidate());
        assert_eq!(first.corrected_word(), second.corrected_word());
    }

    #[test]
    fn candidate_cap_is_reported_without_losing_the_uncapped_bound() {
        let (matrix, llrs, hard_word) = fixture_a();
        let config = OsdConfig::new(1).with_candidate_cap(Some(2));
        let result = SyndromeOsdCorrector::new(matrix, config)
            .correct(&hard_word, &llrs)
            .unwrap();

        assert_eq!(result.work().theoretical_candidates(), 3);
        assert_eq!(result.work().generated_patterns(), 2);
        assert_eq!(result.work().tested_candidates(), 2);
        assert_eq!(result.work().termination(), OsdTermination::CandidateCap);
    }

    #[test]
    fn zero_candidate_cap_returns_metadata_without_a_correction() {
        let (matrix, llrs, hard_word) = fixture_a();
        let config = OsdConfig::new(1).with_candidate_cap(Some(0));
        let result = SyndromeOsdCorrector::new(matrix, config)
            .correct(&hard_word, &llrs)
            .unwrap();

        assert!(result.corrected_word().is_none());
        assert!(result.error_pattern().is_none());
        assert_eq!(result.work().tested_candidates(), 0);
        assert_eq!(result.work().termination(), OsdTermination::CandidateCap);
    }

    #[test]
    fn the_surface_exposes_the_retained_inputs_and_the_engine_outcome() {
        let (matrix, llrs, hard_word) = fixture_a();
        let corrector = SyndromeOsdCorrector::new(matrix.clone(), OsdConfig::new(1));
        assert_eq!(corrector.parity_check(), &matrix);
        assert_eq!(corrector.config(), OsdConfig::new(1));
        assert_eq!(corrector.n(), matrix.cols());

        let result = corrector.correct(&hard_word, &llrs).unwrap();
        assert_eq!(result.syndrome(), &matrix.matvec(&hard_word));
        assert_eq!(result.work(), result.outcome().work());
        assert_eq!(result.error_pattern(), result.candidate().map(|c| c.word()));

        let work = *result.work();
        let corrected = result.corrected_word().unwrap().clone();
        assert_eq!(result.clone().into_corrected_word(), Some(corrected));
        assert_eq!(*result.into_outcome().work(), work);
    }

    #[test]
    fn hard_word_length_mismatch_is_reported() {
        let matrix = gf2_core::bitmatrix![1, 0, 1, 1; 0, 1, 1, 1];
        let error = SyndromeOsdCorrector::new(matrix, OsdConfig::new(0))
            .correct(&bitvec(0, 3), &[Llr::new(1.0); 4])
            .unwrap_err();
        assert_eq!(
            error,
            SyndromeOsdError::HardWordLength {
                expected: 4,
                actual: 3
            }
        );
    }

    #[test]
    fn llr_length_mismatch_is_reported() {
        let (matrix, _, hard_word) = fixture_a();
        let error = SyndromeOsdCorrector::new(matrix, OsdConfig::new(0))
            .correct(&hard_word, &[Llr::new(1.0); 3])
            .unwrap_err();
        assert_eq!(
            error,
            SyndromeOsdError::Engine(OsdEngineError::MagnitudeLength {
                expected: 4,
                actual: 3
            })
        );
    }

    #[test]
    fn syndrome_length_mismatch_is_reported() {
        let (matrix, llrs, _) = fixture_a();
        let error = SyndromeOsdCorrector::new(matrix, OsdConfig::new(0))
            .solve(&bitvec(0, 1), &llrs)
            .unwrap_err();
        assert!(matches!(
            error,
            SyndromeOsdError::Engine(OsdEngineError::Elimination(
                OrderedEliminationError::RightHandSideLength {
                    expected: 2,
                    actual: 1
                }
            ))
        ));
    }

    #[test]
    #[should_panic(expected = "cannot be NaN")]
    fn nan_posterior_magnitude_panics() {
        let (matrix, _, hard_word) = fixture_a();
        let llrs = [Llr::new(f32::NAN); 4];
        let _ = SyndromeOsdCorrector::new(matrix, OsdConfig::new(0)).correct(&hard_word, &llrs);
    }
}
