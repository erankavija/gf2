//! Mutable belief-propagation decoding with syndrome-domain OSD fallback.
//!
//! [`BpOsdDecoder`] runs LDPC belief propagation first.  A BP hard word whose
//! syndrome is zero is the final decision; otherwise the decoder passes that
//! word and BP's posterior LLRs to [`SyndromeOsdCorrector`].  The composition
//! is intentionally an inherent mutable API rather than an implementation of
//! [`crate::traits::SoftDecoder`], whose decode operation is immutable.

use std::fmt;

use gf2_core::BitVec;

use crate::ldpc::{DecoderConfig, LdpcCode, LdpcDecoder};
use crate::llr::Llr;
use crate::traits::IterativeSoftDecoder;

use super::{
    OsdConfig, OsdTermination, OsdWork, SyndromeOsdCorrector, SyndromeOsdError, SyndromeOsdResult,
};

/// The stage whose decision terminated a BP-to-OSD decode.
///
/// OSD is the deciding stage whenever fallback ran, including a bounded run
/// that found no candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BpOsdStage {
    /// BP produced a hard word with zero syndrome, so OSD did not run.
    BeliefPropagation,
    /// BP failed its syndrome check and syndrome-domain OSD ran.
    OrderedStatistics,
}

/// Why a BP-to-OSD decode stopped.
///
/// The BP variant records the early path for which no [`OsdWork`] exists.  An
/// OSD variant preserves the shared engine reason verbatim.  The composition's
/// ordinary fallback forms `H yᵀ`, which is necessarily consistent for the
/// same `H`; [`OsdTermination::InconsistentTransform`] remains representable
/// when normalizing metadata from [`SyndromeOsdCorrector::solve`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BpOsdTermination {
    /// BP's hard word satisfied the parity checks.
    BpSyndromeSatisfied,
    /// Fallback stopped for the enclosed OSD engine reason.
    Osd(OsdTermination),
}

impl From<OsdTermination> for BpOsdTermination {
    fn from(termination: OsdTermination) -> Self {
        Self::Osd(termination)
    }
}

/// The inherent result of one mutable [`BpOsdDecoder`] call.
///
/// The BP hard word and iteration count are always retained.  [`Self::osd_work`]
/// is `None` exactly when BP succeeded and fallback did not run; otherwise it
/// exposes every shared [`OsdWork`] counter.  [`Self::decoded_word`] is absent
/// when OSD ran without testing a candidate, such as with a zero candidate
/// cap.
#[derive(Clone, Debug, PartialEq)]
pub struct BpOsdResult {
    bp_hard_word: BitVec,
    bp_iterations: usize,
    stage: BpOsdStage,
    osd_result: Option<SyndromeOsdResult>,
    syndrome_check_passed: bool,
    termination: BpOsdTermination,
}

impl BpOsdResult {
    fn bp_success(bp_hard_word: BitVec, bp_iterations: usize) -> Self {
        Self {
            bp_hard_word,
            bp_iterations,
            stage: BpOsdStage::BeliefPropagation,
            osd_result: None,
            syndrome_check_passed: true,
            termination: BpOsdTermination::BpSyndromeSatisfied,
        }
    }

    fn osd_decision(
        bp_hard_word: BitVec,
        bp_iterations: usize,
        osd_result: SyndromeOsdResult,
        syndrome_check_passed: bool,
    ) -> Self {
        let termination = osd_result.work().termination().into();
        Self {
            bp_hard_word,
            bp_iterations,
            stage: BpOsdStage::OrderedStatistics,
            osd_result: Some(osd_result),
            syndrome_check_passed,
            termination,
        }
    }

    /// Returns the stage that terminated the composition.
    pub const fn stage(&self) -> BpOsdStage {
        self.stage
    }

    /// Returns BP's full hard-decided codeword before any OSD correction.
    pub const fn bp_hard_word(&self) -> &BitVec {
        &self.bp_hard_word
    }

    /// Returns the final decoded codeword, if the deciding stage supplied one.
    ///
    /// This is BP's hard word when BP passed its syndrome check, the corrected
    /// word when OSD found a candidate, and `None` when fallback found none.
    pub fn decoded_word(&self) -> Option<&BitVec> {
        match &self.osd_result {
            Some(result) => result.corrected_word(),
            None => Some(&self.bp_hard_word),
        }
    }

    /// Returns the number of BP iterations performed before the decision.
    pub const fn bp_iterations(&self) -> usize {
        self.bp_iterations
    }

    /// Returns OSD work counters, or `None` when BP succeeded without fallback.
    pub fn osd_work(&self) -> Option<&OsdWork> {
        self.osd_result.as_ref().map(SyndromeOsdResult::work)
    }

    /// Returns the syndrome-domain fallback result, if fallback ran.
    ///
    /// This exposes the failed BP syndrome, selected error pattern, candidate,
    /// and complete engine outcome without duplicating their representations.
    pub const fn osd_result(&self) -> Option<&SyndromeOsdResult> {
        self.osd_result.as_ref()
    }

    /// Returns whether the final decoded word has zero syndrome.
    ///
    /// The status is false when [`Self::decoded_word`] is absent.
    pub const fn syndrome_check_passed(&self) -> bool {
        self.syndrome_check_passed
    }

    /// Returns the reason the composition stopped.
    pub const fn termination(&self) -> BpOsdTermination {
        self.termination
    }

    /// Consumes the result and returns the final decoded word, if one exists.
    pub fn into_decoded_word(self) -> Option<BitVec> {
        match self.osd_result {
            Some(result) => result.into_corrected_word(),
            None => Some(self.bp_hard_word),
        }
    }
}

/// Errors returned after BP hands a failed hard word to OSD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BpOsdDecodeError {
    /// Syndrome-domain OSD rejected the BP hard word, posterior, or search.
    Osd(SyndromeOsdError),
}

impl fmt::Display for BpOsdDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Osd(error) => write!(formatter, "BP fallback failed: {error}"),
        }
    }
}

impl std::error::Error for BpOsdDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Osd(error) => Some(error),
        }
    }
}

impl From<SyndromeOsdError> for BpOsdDecodeError {
    fn from(error: SyndromeOsdError) -> Self {
        Self::Osd(error)
    }
}

/// Mutable LDPC BP decoding with syndrome-domain OSD fallback.
///
/// The decoder owns one [`LdpcDecoder`] and a [`SyndromeOsdCorrector`] built
/// from the same parity-check matrix.  [`Self::decode`] always starts BP from
/// reset messages and beliefs, then stores a clone of the returned result as
/// [`Self::last_result`].  Repeated calls are independent and replace both BP
/// state and the retained result.
///
/// Before the first decode, and after [`Self::reset`], posterior beliefs are
/// zero and `last_result()` is `None`.  A bounded OSD run that finds no
/// candidate is a normal `Ok` result with no decoded word, false final
/// syndrome status, and retained OSD work.  If OSD returns an error,
/// `last_result()` stays `None` while the failed BP posterior remains
/// available for diagnosis; a subsequent decode starts from reset state.
/// `reset()` clears both BP state and the retained composition result.
///
/// This type deliberately has no [`crate::traits::SoftDecoder`]
/// implementation: belief propagation and its lifecycle require `&mut self`.
///
/// # Complexity
///
/// A successful BP path costs O(iterations × Tanner-graph edges).  Fallback
/// adds one ordered elimination plus O(candidates × (order + n) / 64)
/// reconstruction and ranking work; see [`SyndromeOsdCorrector::correct`].
/// The decoder retains O(Tanner-graph edges) BP messages, the parity-check
/// matrix, and one result-sized clone of the latest decision.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::LdpcCode;
/// use gf2_coding::llr::Llr;
/// use gf2_coding::osd::{BpOsdDecoder, BpOsdStage, OsdConfig};
///
/// // H = [1 1 0; 0 1 1] defines a three-bit repetition code.  One bounded
/// // BP iteration leaves a failed hard word, which order-zero syndrome OSD
/// // corrects using BP's posterior reliabilities.
/// let code = LdpcCode::from_edges(
///     2,
///     3,
///     &[(0, 0), (0, 1), (1, 1), (1, 2)],
/// );
/// let mut decoder = BpOsdDecoder::new(code, OsdConfig::new(0));
/// let llrs = [Llr::new(-2.0), Llr::new(1.0), Llr::new(10.0)];
///
/// let result = decoder.decode(&llrs, 1).unwrap();
/// assert_eq!(result.stage(), BpOsdStage::OrderedStatistics);
/// assert!(result.syndrome_check_passed());
/// assert_eq!(result.decoded_word().unwrap().count_ones(), 0);
/// assert_eq!(result.bp_iterations(), 1);
/// assert_eq!(result.osd_work().unwrap().tested_candidates(), 1);
/// ```
#[derive(Debug)]
pub struct BpOsdDecoder {
    bp: LdpcDecoder,
    corrector: SyndromeOsdCorrector,
    last_result: Option<BpOsdResult>,
}

impl BpOsdDecoder {
    /// Creates a decoder with the default BP configuration and `osd_config`.
    pub fn new(code: LdpcCode, osd_config: OsdConfig) -> Self {
        Self::with_bp_config(code, DecoderConfig::default(), osd_config)
    }

    /// Creates a decoder with explicit BP and OSD configurations.
    ///
    /// The syndrome corrector receives a dense copy of `code`'s parity-check
    /// matrix, which keeps BP convergence and OSD correction in the same code
    /// domain.
    pub fn with_bp_config(code: LdpcCode, bp_config: DecoderConfig, osd_config: OsdConfig) -> Self {
        let parity_check = code.parity_check_matrix().to_dense();
        Self {
            bp: LdpcDecoder::with_config(code, bp_config),
            corrector: SyndromeOsdCorrector::new(parity_check, osd_config),
            last_result: None,
        }
    }

    /// Runs mutable BP and invokes syndrome-domain OSD only after BP failure.
    ///
    /// BP success returns its full hard word directly with no OSD work.  On BP
    /// syndrome failure, this passes the exact hard word and the matching
    /// [`LdpcDecoder::posterior_llrs`] slice to
    /// [`SyndromeOsdCorrector::correct`].  The returned result is also retained
    /// by [`Self::last_result`].
    ///
    /// # Errors
    ///
    /// Returns [`BpOsdDecodeError::Osd`] if fallback rejects its dimensions or
    /// if the configured uncapped candidate bound cannot be represented in
    /// `usize`.  The construction invariant keeps the BP word and posterior
    /// dimensions equal to the corrector width.
    ///
    /// # Panics
    ///
    /// Panics if `llrs` does not contain one value per codeword coordinate.
    /// If BP fails and OSD runs, also panics if a posterior LLR has NaN
    /// magnitude, matching [`SyndromeOsdCorrector::correct`].
    ///
    /// # Complexity
    ///
    /// O(`max_iterations` × Tanner-graph edges) for BP.  On fallback, add the
    /// ordered-elimination and candidate costs documented on
    /// [`SyndromeOsdCorrector::correct`].
    pub fn decode(
        &mut self,
        llrs: &[Llr],
        max_iterations: usize,
    ) -> Result<BpOsdResult, BpOsdDecodeError> {
        self.last_result = None;
        self.bp.reset();
        let bp_result = self.bp.decode_to_codeword(llrs, max_iterations);

        let result = if bp_result.syndrome_check_passed {
            BpOsdResult::bp_success(bp_result.decoded_bits, bp_result.iterations)
        } else {
            let osd_result = self
                .corrector
                .correct(&bp_result.decoded_bits, self.bp.posterior_llrs())?;
            let syndrome_check_passed = osd_result
                .corrected_word()
                .is_some_and(|word| self.corrector.parity_check().matvec(word).count_ones() == 0);
            BpOsdResult::osd_decision(
                bp_result.decoded_bits,
                bp_result.iterations,
                osd_result,
                syndrome_check_passed,
            )
        };

        self.last_result = Some(result.clone());
        Ok(result)
    }

    /// Returns BP's current posterior beliefs.
    ///
    /// They are zero before decoding and after [`Self::reset`], and otherwise
    /// belong to the BP run behind [`Self::last_result`] or the latest OSD
    /// error.
    pub fn posterior_llrs(&self) -> &[Llr] {
        self.bp.posterior_llrs()
    }

    /// Returns the most recently completed composition result.
    ///
    /// OSD errors and [`Self::reset`] leave this as `None`.
    pub const fn last_result(&self) -> Option<&BpOsdResult> {
        self.last_result.as_ref()
    }

    /// Returns the retained BP decoder for read-only state inspection.
    pub const fn bp_decoder(&self) -> &LdpcDecoder {
        &self.bp
    }

    /// Returns the retained syndrome-domain corrector.
    pub const fn corrector(&self) -> &SyndromeOsdCorrector {
        &self.corrector
    }

    /// Resets BP messages, beliefs, iteration count, and composition state.
    pub fn reset(&mut self) {
        self.bp.reset();
        self.last_result = None;
    }
}

#[cfg(test)]
mod tests {
    use gf2_core::{BitMatrix, BitVec};

    use crate::ldpc::LdpcCode;
    use crate::llr::Llr;
    use crate::osd::{OsdConfig, OsdTermination};

    use super::{BpOsdDecoder, BpOsdStage, BpOsdTermination};

    fn fixture() -> (LdpcCode, BitMatrix) {
        let edges = [(0, 0), (0, 1), (1, 1), (1, 2)];
        let code = LdpcCode::from_edges(2, 3, &edges);
        let parity_check = code.parity_check_matrix().to_dense();
        (code, parity_check)
    }

    fn failing_llrs() -> [Llr; 3] {
        [Llr::new(-2.0), Llr::new(1.0), Llr::new(10.0)]
    }

    fn rank_deficient_fixture() -> LdpcCode {
        LdpcCode::from_edges(
            3,
            4,
            &[
                (0, 0),
                (0, 2),
                (0, 3),
                (1, 1),
                (1, 2),
                (1, 3),
                (2, 0),
                (2, 1),
            ],
        )
    }

    fn bitvec(mask: usize, len: usize) -> BitVec {
        let mut word = BitVec::zeros(len);
        for bit in 0..len {
            word.set(bit, (mask >> bit) & 1 == 1);
        }
        word
    }

    #[test]
    fn bp_success_returns_bp_word_without_invoking_osd() {
        let (code, _) = fixture();
        let mut decoder = BpOsdDecoder::new(code, OsdConfig::new(0).with_candidate_cap(Some(0)));

        let result = decoder.decode(&[Llr::new(10.0); 3], 1).unwrap();

        assert_eq!(result.stage(), BpOsdStage::BeliefPropagation);
        assert_eq!(result.decoded_word(), Some(&BitVec::zeros(3)));
        assert_eq!(result.bp_hard_word(), &BitVec::zeros(3));
        assert_eq!(result.bp_iterations(), 1);
        assert!(result.osd_work().is_none());
        assert!(result.syndrome_check_passed());
        assert_eq!(result.termination(), BpOsdTermination::BpSyndromeSatisfied);
    }

    #[test]
    fn bp_failure_with_osd_success_returns_zero_syndrome_word() {
        let (code, parity_check) = fixture();
        let mut decoder = BpOsdDecoder::new(code, OsdConfig::new(0));

        let result = decoder.decode(&failing_llrs(), 1).unwrap();

        assert_eq!(result.stage(), BpOsdStage::OrderedStatistics);
        assert_eq!(result.bp_hard_word(), &bitvec(0b001, 3));
        assert_eq!(decoder.posterior_llrs()[0].magnitude(), 1.0);
        assert_ne!(
            decoder.posterior_llrs()[0].magnitude(),
            failing_llrs()[0].magnitude()
        );
        let corrected = result.decoded_word().unwrap();
        assert_eq!(parity_check.matvec(corrected).count_ones(), 0);
        assert!(result.syndrome_check_passed());
        let work = result.osd_work().unwrap();
        assert_eq!(work.generated_patterns(), 1);
        assert_eq!(work.tested_candidates(), 1);
        assert_eq!(work.eliminations(), 1);
        assert_eq!(
            result.osd_result().unwrap().candidate().unwrap().metric(),
            1.0
        );
        assert_eq!(
            result.termination(),
            BpOsdTermination::Osd(OsdTermination::Exhaustive)
        );
    }

    #[test]
    fn exhausted_osd_reports_termination_and_counters() {
        let (code, _) = fixture();
        let mut decoder = BpOsdDecoder::new(code, OsdConfig::new(1).with_candidate_cap(Some(0)));

        let result = decoder.decode(&failing_llrs(), 1).unwrap();

        assert_eq!(result.stage(), BpOsdStage::OrderedStatistics);
        assert!(result.decoded_word().is_none());
        assert!(!result.syndrome_check_passed());
        let work = result.osd_work().unwrap();
        assert_eq!(work.rank(), 2);
        assert_eq!(work.generated_patterns(), 0);
        assert_eq!(work.tested_candidates(), 0);
        assert_eq!(work.eliminations(), 1);
        assert!(work.theoretical_candidates() > 0);
        assert_eq!(work.termination(), OsdTermination::CandidateCap);
        assert_eq!(
            result.termination(),
            BpOsdTermination::Osd(OsdTermination::CandidateCap)
        );
    }

    #[test]
    fn inconsistent_syndrome_is_reported_not_panicked() {
        let decoder = BpOsdDecoder::new(rank_deficient_fixture(), OsdConfig::new(4));
        let outcome = decoder
            .corrector()
            .solve(&bitvec(0b111, 3), &[Llr::new(1.0); 4])
            .unwrap();

        assert!(outcome.best().is_none());
        assert_eq!(outcome.work().tested_candidates(), 0);
        assert_eq!(
            BpOsdTermination::from(outcome.work().termination()),
            BpOsdTermination::Osd(OsdTermination::InconsistentTransform)
        );
    }

    #[test]
    fn reset_restores_pre_decode_beliefs_and_clears_composition_state() {
        let (code, _) = fixture();
        let mut decoder = BpOsdDecoder::new(code, OsdConfig::new(1).with_candidate_cap(Some(0)));

        assert!(decoder.last_result().is_none());
        assert!(decoder
            .posterior_llrs()
            .iter()
            .all(|belief| belief.value() == 0.0));

        let failed = decoder.decode(&failing_llrs(), 1).unwrap();
        assert!(!failed.syndrome_check_passed());
        assert!(decoder.last_result().is_some());
        assert!(decoder
            .posterior_llrs()
            .iter()
            .any(|belief| belief.value() != 0.0));

        let repeated = decoder.decode(&[Llr::new(10.0); 3], 1).unwrap();
        assert!(repeated.syndrome_check_passed());
        assert_eq!(decoder.last_result(), Some(&repeated));

        decoder.reset();

        assert!(decoder.last_result().is_none());
        assert!(decoder
            .posterior_llrs()
            .iter()
            .all(|belief| belief.value() == 0.0));
    }
}
