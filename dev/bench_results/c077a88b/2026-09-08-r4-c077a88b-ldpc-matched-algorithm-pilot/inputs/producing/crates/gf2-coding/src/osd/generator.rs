//! Generator-matrix ordered-statistics decoding.

use gf2_core::{BitMatrix, BitVec};

use crate::llr::Llr;
use crate::traits::{DecoderResult, GeneratorMatrixAccess, SoftDecoder};

use super::{
    reprocess, ColumnPreference, MostReliableBasis, OsdCandidate, OsdConfig, OsdEngineError,
    OsdOutcome, OsdSemantics, OsdWork, ReprocessedColumns,
};

/// Order-`m` OSD for a code that exposes its generator matrix.
///
/// The decoder snapshots `code.generator_matrix()` at construction and uses a
/// most-reliable independent set of its columns for every immutable decode
/// call.  Reprocessing happens in the reduced generator coordinates, while
/// the returned bits are transformed back to the original generator domain.
/// Consequently, the decoder does not require a systematic generator.
///
/// A rank-deficient generator is valid.  Its search dimension and candidate
/// bound use the matrix rank, and the decoded bits are the deterministic
/// preimage obtained by setting the unused reduced-row coefficients to zero
/// before applying the elimination row transform.  Re-encoding those bits
/// always yields the selected codeword, although another generator-domain
/// preimage can represent the same word.
///
/// Construct with [`Self::new`] and use [`Self::decode`] when engine metadata
/// is needed.  The [`SoftDecoder`] implementation returns the same decoded
/// bits for configurations that test at least one candidate.
#[derive(Clone, Debug)]
pub struct GeneratorMatrixOsdDecoder<C> {
    code: C,
    generator: BitMatrix,
    config: OsdConfig,
}

/// A generator-matrix OSD decision together with the shared engine outcome.
///
/// [`Self::work`] directly exposes the shared engine's rank, theoretical
/// candidate bound, generated-pattern count, tested-candidate count,
/// elimination count, and termination reason.  No adapter-local search
/// counters are maintained.
///
/// A zero candidate cap produces a result without a decision and with
/// [`super::OsdTermination::CandidateCap`] metadata.  Every other successful
/// generator-matrix run tests at least the empty pattern and therefore carries
/// decoded bits and a selected codeword.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratorMatrixOsdResult {
    decoded_bits: Option<BitVec>,
    outcome: OsdOutcome,
}

impl<C> GeneratorMatrixOsdDecoder<C>
where
    C: GeneratorMatrixAccess,
{
    /// Creates a decoder for `code` using `config`.
    ///
    /// The generator matrix is requested once and retained by the decoder.
    /// Both the order and optional cap are read directly from `config` by the
    /// shared OSD engine on each decode.
    ///
    /// # Panics
    ///
    /// Panics if `code.generator_matrix()` does not have exactly `code.k()`
    /// rows and `code.n()` columns, in violation of the
    /// [`GeneratorMatrixAccess`] contract.
    pub fn new(code: C, config: OsdConfig) -> Self {
        let generator = code.generator_matrix();
        assert_eq!(
            generator.rows(),
            code.k(),
            "generator matrix must have code.k() rows"
        );
        assert_eq!(
            generator.cols(),
            code.n(),
            "generator matrix must have code.n() columns"
        );
        Self {
            code,
            generator,
            config,
        }
    }

    /// Returns the code being decoded.
    pub const fn code(&self) -> &C {
        &self.code
    }

    /// Returns the OSD search configuration.
    pub const fn config(&self) -> OsdConfig {
        self.config
    }

    /// Decodes one LLR word and returns the generator-domain result.
    ///
    /// The LLR hard decisions form the reference word and their magnitudes
    /// select the most-reliable independent generator columns.  The shared
    /// engine enumerates the configured order-`m` patterns, reconstructs
    /// codewords from the reduced generator rows, and retains the first
    /// candidate attaining the minimum soft metric.  Canonical reliability
    /// ordering and engine generation order make ties deterministic.
    ///
    /// Rank-deficient generators are accepted as described on
    /// [`GeneratorMatrixOsdDecoder`].  A zero candidate cap returns successful
    /// metadata with no decision; this inherent surface therefore preserves
    /// the engine outcome even when the [`SoftDecoder`] surface cannot return
    /// a message.
    ///
    /// # Errors
    ///
    /// Returns [`OsdEngineError::MagnitudeLength`] if `llrs.len()` is not
    /// `self.code().n()`.  Returns [`OsdEngineError::Patterns`] if the checked
    /// uncapped candidate bound for the generator rank and configured order
    /// does not fit in `usize`.
    ///
    /// # Panics
    ///
    /// Panics if any LLR has a NaN magnitude.
    ///
    /// # Complexity
    ///
    /// One ordered elimination plus the shared engine's
    /// O(candidates × (order + n) / 64) reconstruction and ranking work.  The
    /// decoder stores O(k × n) generator bits and each call uses O(k × n)
    /// elimination storage plus O(k × n) candidate-delta bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::LinearBlockCode;
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::osd::{GeneratorMatrixOsdDecoder, OsdConfig, OsdTermination};
    ///
    /// let decoder = GeneratorMatrixOsdDecoder::new(
    ///     LinearBlockCode::hamming(3),
    ///     OsdConfig::new(1),
    /// );
    /// let llrs = [
    ///     Llr::new(3.0), Llr::new(-2.0), Llr::new(1.0),
    ///     Llr::new(4.0), Llr::new(-1.0), Llr::new(2.0), Llr::new(3.0),
    /// ];
    ///
    /// let result = decoder.decode(&llrs).unwrap();
    /// assert_eq!(result.decoded_bits().unwrap().len(), 4);
    /// assert_eq!(result.codeword().unwrap().len(), 7);
    /// assert_eq!(result.work().rank(), 4);
    /// assert_eq!(result.work().termination(), OsdTermination::Exhaustive);
    /// ```
    pub fn decode(&self, llrs: &[Llr]) -> Result<GeneratorMatrixOsdResult, OsdEngineError> {
        let magnitudes: Vec<f32> = llrs.iter().map(|llr| llr.magnitude()).collect();
        let mut reference = BitVec::zeros(llrs.len());
        for (column, llr) in llrs.iter().enumerate() {
            reference.set(column, llr.hard_decision());
        }

        let basis = MostReliableBasis::build(
            &self.generator,
            &magnitudes,
            &reference,
            None,
            ColumnPreference::MostReliableFirst,
        )?;
        let outcome = reprocess(&basis, &GeneratorMatrixSemantics, self.config)?;
        let decoded_bits = outcome
            .best()
            .map(|candidate| generator_preimage(&basis, candidate.word()));

        Ok(GeneratorMatrixOsdResult {
            decoded_bits,
            outcome,
        })
    }
}

impl GeneratorMatrixOsdResult {
    /// Returns the decoded generator-domain bits, if a candidate was tested.
    ///
    /// Re-encoding these bits with the decoder's generator matrix yields
    /// [`Self::codeword`].  For a rank-deficient generator they are the
    /// canonical preimage described on [`GeneratorMatrixOsdDecoder`].
    pub fn decoded_bits(&self) -> Option<&BitVec> {
        self.decoded_bits.as_ref()
    }

    /// Returns the selected codeword, if a candidate was tested.
    pub fn codeword(&self) -> Option<&BitVec> {
        self.outcome.best().map(OsdCandidate::word)
    }

    /// Returns the selected engine candidate, if one was tested.
    pub fn candidate(&self) -> Option<&OsdCandidate> {
        self.outcome.best()
    }

    /// Returns the engine work counters and termination metadata.
    pub const fn work(&self) -> &OsdWork {
        self.outcome.work()
    }

    /// Consumes the result and returns the decoded generator-domain bits.
    pub fn into_decoded_bits(self) -> Option<BitVec> {
        self.decoded_bits
    }

    /// Returns the complete shared-engine outcome.
    pub const fn outcome(&self) -> &OsdOutcome {
        &self.outcome
    }

    /// Consumes the result and returns the complete shared-engine outcome.
    pub fn into_outcome(self) -> OsdOutcome {
        self.outcome
    }
}

/// Generator-row-space candidate reconstruction for the shared engine.
struct GeneratorMatrixSemantics;

impl OsdSemantics for GeneratorMatrixSemantics {
    fn reprocessed_columns(&self) -> ReprocessedColumns {
        ReprocessedColumns::Basis
    }

    fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
        let elimination = basis.elimination();
        let mut reduced_coefficients = BitVec::zeros(elimination.reduced.rows());
        for (row, &column) in elimination.selected_cols.iter().enumerate() {
            reduced_coefficients.set(row, basis.reference().get(column));
        }
        elimination.reduced.matvec_transpose(&reduced_coefficients)
    }

    fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
        (0..basis.elimination().rank)
            .map(|row| basis.elimination().reduced.row_as_bitvec(row))
            .collect()
    }

    fn accepts(&self, _candidate: &BitVec) -> bool {
        // The base and every delta are rows of the reduced generator, so every
        // reconstructed candidate is a codeword in the generator row space.
        true
    }
}

/// Recovers a deterministic original-row coefficient vector from a codeword.
fn generator_preimage(basis: &MostReliableBasis, codeword: &BitVec) -> BitVec {
    let elimination = basis.elimination();
    let mut reduced_coefficients = BitVec::zeros(elimination.reduced.rows());
    // The selected columns form the identity in `reduced`, so these codeword
    // coordinates are exactly the reduced-row coefficients q.  Since
    // reduced = U G, q reduced = (q U) G; multiplying q by U therefore returns
    // coefficients for the original generator rows.  Coefficients beyond the
    // rank stay zero, selecting one deterministic preimage when G is deficient.
    for (row, &column) in elimination.selected_cols.iter().enumerate() {
        reduced_coefficients.set(row, codeword.get(column));
    }
    elimination
        .transform
        .matvec_transpose(&reduced_coefficients)
}

impl<C> SoftDecoder for GeneratorMatrixOsdDecoder<C>
where
    C: GeneratorMatrixAccess,
{
    fn k(&self) -> usize {
        self.code.k()
    }

    fn n(&self) -> usize {
        self.code.n()
    }

    /// Returns the generator-domain bits of the best OSD candidate.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != self.n()`, if the configured uncapped
    /// candidate bound does not fit in `usize`, or if the candidate cap is
    /// zero and therefore no hard decision was tested.
    ///
    /// Panics if any LLR has a NaN magnitude.
    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        self.decode(llrs)
            .unwrap_or_else(|error| panic!("generator-matrix OSD decoding failed: {error}"))
            .into_decoded_bits()
            .expect("generator-matrix OSD cannot return a decision with a zero candidate cap")
    }

    /// Returns the generic soft-decoder result for the same OSD decision.
    ///
    /// `iterations` records the engine elimination count and `queries` records
    /// the tested-candidate count.  Use [`GeneratorMatrixOsdDecoder::decode`]
    /// for the rank, uncapped bound, and termination reason.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != self.n()`, if the configured uncapped
    /// candidate bound does not fit in `usize`, or if the candidate cap is
    /// zero and therefore no hard decision was tested.
    ///
    /// Panics if any LLR has a NaN magnitude.
    fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
        let result = self
            .decode(llrs)
            .unwrap_or_else(|error| panic!("generator-matrix OSD decoding failed: {error}"));
        let iterations = result.work().eliminations();
        let queries = result.work().tested_candidates();
        let decoded = result
            .into_decoded_bits()
            .expect("generator-matrix OSD cannot return a decision with a zero candidate cap");
        let mut decoder_result = DecoderResult::new(decoded, iterations, true, true);
        decoder_result.queries = Some(queries);
        decoder_result
    }
}

#[cfg(test)]
mod tests {
    use gf2_core::{bitmatrix, BitVec};

    use super::*;
    use crate::osd::OsdTermination;

    #[derive(Clone, Debug)]
    struct FixtureCode {
        generator: BitMatrix,
    }

    impl GeneratorMatrixAccess for FixtureCode {
        fn k(&self) -> usize {
            self.generator.rows()
        }

        fn n(&self) -> usize {
            self.generator.cols()
        }

        fn generator_matrix(&self) -> BitMatrix {
            self.generator.clone()
        }
    }

    fn bits(mask: usize, len: usize) -> BitVec {
        let mut value = BitVec::zeros(len);
        for index in 0..len {
            value.set(index, mask & (1 << index) != 0);
        }
        value
    }

    fn metric(candidate: &BitVec, llrs: &[Llr]) -> f64 {
        llrs.iter()
            .enumerate()
            .filter(|(index, llr)| candidate.get(*index) != llr.hard_decision())
            .map(|(_, llr)| f64::from(llr.magnitude()))
            .sum()
    }

    fn exhaustive_ml_metric(generator: &BitMatrix, llrs: &[Llr]) -> f64 {
        (0..(1 << generator.rows()))
            .map(|mask| generator.matvec_transpose(&bits(mask, generator.rows())))
            .map(|candidate| metric(&candidate, llrs))
            .reduce(f64::min)
            .expect("the all-zero message is always present")
    }

    fn nonsystematic_code() -> FixtureCode {
        FixtureCode {
            generator: bitmatrix![
                1, 1, 0, 1, 0;
                0, 1, 1, 0, 1;
                1, 0, 1, 1, 0;
            ],
        }
    }

    #[test]
    fn exhaustive_order_matches_ml_for_tied_nonsystematic_fixture() {
        let code = nonsystematic_code();
        let generator = code.generator_matrix();
        let decoder = GeneratorMatrixOsdDecoder::new(code, OsdConfig::new(3));
        let llrs = [
            Llr::new(-2.0),
            Llr::new(2.0),
            Llr::new(-2.0),
            Llr::new(1.0),
            Llr::new(1.0),
        ];

        let result = decoder.decode(&llrs).unwrap();
        assert_eq!(result.work().rank(), 3);
        assert_eq!(
            result.candidate().unwrap().metric(),
            exhaustive_ml_metric(&generator, &llrs)
        );
        assert_eq!(
            generator.matvec_transpose(result.decoded_bits().unwrap()),
            *result.codeword().unwrap()
        );
    }

    #[test]
    fn exhaustive_order_matches_ml_for_second_small_code_fixture() {
        let generator = bitmatrix![
            1, 0, 1, 1;
            0, 1, 1, 0;
        ];
        let code = FixtureCode {
            generator: generator.clone(),
        };
        let decoder = GeneratorMatrixOsdDecoder::new(code, OsdConfig::new(2));

        for hard_word in 0..(1 << generator.cols()) {
            let llrs: Vec<Llr> = (0..generator.cols())
                .map(|column| {
                    let magnitude = if column < 2 { 2.0 } else { 1.0 };
                    let sign = if hard_word & (1 << column) == 0 {
                        magnitude
                    } else {
                        -magnitude
                    };
                    Llr::new(sign)
                })
                .collect();
            let result = decoder.decode(&llrs).unwrap();
            assert_eq!(
                result.candidate().unwrap().metric(),
                exhaustive_ml_metric(&generator, &llrs),
                "hard word {hard_word:04b}"
            );
        }
    }

    #[test]
    fn tied_reliabilities_are_deterministic_under_repetition() {
        let decoder = GeneratorMatrixOsdDecoder::new(nonsystematic_code(), OsdConfig::new(3));
        let llrs = [Llr::new(-1.0); 5];
        let first = decoder.decode(&llrs).unwrap();

        for _ in 0..16 {
            assert_eq!(decoder.decode(&llrs).unwrap(), first);
        }
    }

    #[test]
    fn rank_deficient_generator_returns_canonical_reencoding() {
        let generator = bitmatrix![
            1, 0, 1, 1;
            0, 1, 1, 0;
            1, 1, 0, 1;
        ];
        let code = FixtureCode {
            generator: generator.clone(),
        };
        let decoder = GeneratorMatrixOsdDecoder::new(code, OsdConfig::new(2));
        let llrs = [Llr::new(-3.0), Llr::new(2.0), Llr::new(-1.0), Llr::new(4.0)];

        let result = decoder.decode(&llrs).unwrap();
        assert_eq!(result.work().rank(), 2);
        assert_eq!(result.work().theoretical_candidates(), 4);
        assert_eq!(result.decoded_bits().unwrap().len(), 3);
        assert_eq!(
            generator.matvec_transpose(result.decoded_bits().unwrap()),
            *result.codeword().unwrap()
        );
        assert_eq!(
            result.candidate().unwrap().metric(),
            exhaustive_ml_metric(&generator, &llrs)
        );
    }

    #[test]
    fn order_zero_tests_only_the_basis_hard_decision() {
        let decoder = GeneratorMatrixOsdDecoder::new(nonsystematic_code(), OsdConfig::new(0));
        let llrs = [
            Llr::new(4.0),
            Llr::new(-3.0),
            Llr::new(2.0),
            Llr::new(-1.0),
            Llr::new(0.5),
        ];

        let result = decoder.decode(&llrs).unwrap();
        assert_eq!(result.candidate().unwrap().pattern(), &[] as &[usize]);
        assert_eq!(result.work().theoretical_candidates(), 1);
        assert_eq!(result.work().generated_patterns(), 1);
        assert_eq!(result.work().tested_candidates(), 1);
        assert_eq!(result.work().eliminations(), 1);
        assert_eq!(result.work().termination(), OsdTermination::Exhaustive);
    }

    #[test]
    fn candidate_cap_is_reported_without_losing_the_uncapped_bound() {
        let decoder = GeneratorMatrixOsdDecoder::new(
            nonsystematic_code(),
            OsdConfig::new(2).with_candidate_cap(Some(2)),
        );
        let llrs = [Llr::new(1.0); 5];

        let result = decoder.decode(&llrs).unwrap();
        assert_eq!(result.work().theoretical_candidates(), 7);
        assert_eq!(result.work().generated_patterns(), 2);
        assert_eq!(result.work().tested_candidates(), 2);
        assert_eq!(result.work().termination(), OsdTermination::CandidateCap);
    }

    #[test]
    fn zero_candidate_cap_returns_metadata_without_a_decision() {
        let decoder = GeneratorMatrixOsdDecoder::new(
            nonsystematic_code(),
            OsdConfig::new(2).with_candidate_cap(Some(0)),
        );
        let llrs = [Llr::new(1.0); 5];

        let result = decoder.decode(&llrs).unwrap();
        assert!(result.decoded_bits().is_none());
        assert!(result.codeword().is_none());
        assert_eq!(result.work().theoretical_candidates(), 7);
        assert_eq!(result.work().tested_candidates(), 0);
        assert_eq!(result.work().termination(), OsdTermination::CandidateCap);
    }

    #[test]
    #[should_panic(expected = "expected one per column")]
    fn soft_decoder_panics_on_llr_length_mismatch() {
        let decoder = GeneratorMatrixOsdDecoder::new(nonsystematic_code(), OsdConfig::new(1));
        let _ = decoder.decode_soft(&[Llr::new(1.0); 4]);
    }

    #[test]
    fn inherent_decode_reports_llr_length_mismatch() {
        let decoder = GeneratorMatrixOsdDecoder::new(nonsystematic_code(), OsdConfig::new(1));
        assert_eq!(
            decoder.decode(&[Llr::new(1.0); 4]).unwrap_err(),
            OsdEngineError::MagnitudeLength {
                expected: 5,
                actual: 4,
            }
        );
    }

    #[test]
    #[should_panic(expected = "cannot be NaN")]
    fn inherent_decode_panics_on_nan() {
        let decoder = GeneratorMatrixOsdDecoder::new(nonsystematic_code(), OsdConfig::new(1));
        let _ = decoder.decode(&[
            Llr::new(1.0),
            Llr::new(f32::NAN),
            Llr::new(1.0),
            Llr::new(1.0),
            Llr::new(1.0),
        ]);
    }

    #[test]
    #[should_panic(expected = "zero candidate cap")]
    fn soft_decoder_panics_when_cap_tests_no_candidate() {
        let decoder = GeneratorMatrixOsdDecoder::new(
            nonsystematic_code(),
            OsdConfig::new(1).with_candidate_cap(Some(0)),
        );
        let _ = decoder.decode_soft(&[Llr::new(1.0); 5]);
    }

    #[test]
    fn implements_the_immutable_soft_decoder_surface() {
        fn assert_soft_decoder<D: SoftDecoder>(_decoder: &D) {}

        let decoder = GeneratorMatrixOsdDecoder::new(nonsystematic_code(), OsdConfig::new(3));
        assert_soft_decoder(&decoder);
        assert_eq!(SoftDecoder::k(&decoder), 3);
        assert_eq!(SoftDecoder::n(&decoder), 5);

        let llrs = [
            Llr::new(-2.0),
            Llr::new(2.0),
            Llr::new(-2.0),
            Llr::new(1.0),
            Llr::new(1.0),
        ];
        assert_eq!(
            decoder.decode_soft(&llrs),
            decoder.decode(&llrs).unwrap().into_decoded_bits().unwrap()
        );

        let osd_result = decoder.decode(&llrs).unwrap();
        let detailed = decoder.decode_soft_with_result(&llrs);
        assert_eq!(detailed.iterations, osd_result.work().eliminations());
        assert_eq!(
            detailed.queries,
            Some(osd_result.work().tested_candidates())
        );
        assert!(detailed.converged);
        assert!(detailed.syndrome_check_passed);
    }

    /// A canonical BCH code satisfies the decoder's legacy generator-matrix
    /// bound through the `binary-code-v1` blanket adapter, with no
    /// BCH-specific path in this module.
    #[test]
    fn accepts_a_canonical_bch_generator_matrix() {
        use crate::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
        use gf2_core::field::extension::BinaryPrimeExt;
        use gf2_core::gf2m::Gf2mField;

        let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011).with_tables())
            .expect("a valid binary BCH extension");
        let code = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(3)
                .expect("a positive BCH designed distance"),
        })
        .expect("a valid binary BCH construction");

        let decoder = GeneratorMatrixOsdDecoder::new(code, OsdConfig::new(2));
        let llrs: Vec<Llr> = (0..15).map(|_| Llr::new(3.0)).collect();

        let result = decoder.decode(&llrs).unwrap();
        assert_eq!(result.work().rank(), 11);
        assert_eq!(
            result.decoded_bits().unwrap(),
            &BitVec::zeros(11),
            "uniform zero-bit-favoring LLRs decode to the all-zero message"
        );
    }
}
