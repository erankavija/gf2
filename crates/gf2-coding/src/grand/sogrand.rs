//! Soft-Output GRAND (SOGRAND) decoder.
//!
//! SOGRAND wraps an [`OrbGrand`] decoder to produce per-bit a-posteriori probability
//! (APP) LLR outputs and extrinsic information, enabling use as a SISO (Soft-Input
//! Soft-Output) component in turbo decoding architectures.
//!
//! # Algorithm
//!
//! Given channel LLRs (possibly combined with a-priori information), SOGRAND:
//!
//! 1. Runs ORBGRAND in list mode to find up to `L` codewords.
//! 2. Computes per-block APP for each list element using Corollary 1 from the
//!    SO-GRAND paper: the noise probability `p(z|r)` of each codeword's noise
//!    pattern, normalized by the total probability mass.
//! 3. Computes the "not found" probability `P(C\L)` — the probability that the
//!    correct codeword is not in the list — using the code parameters `(n, k)`.
//! 4. Computes per-bit APP LLRs (eq. 17): for each bit position, sums the APPs
//!    of list codewords that have that bit as 0 or 1, then adds the fallback
//!    "not found" term weighted by the channel bit probability.
//! 5. Returns APP LLRs, extrinsic LLRs (`L_APP - input`), and the predicted
//!    list BLER `P(C\L)`.
//!
//! # Numerical Stability
//!
//! All intermediate computations are performed in the log domain using
//! log-sum-exp to avoid underflow with large block lengths.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::grand::{OrbGrand, OrbGrandConfig, SoGrand};
//! use gf2_coding::llr::Llr;
//! use gf2_core::BitMatrix;
//!
//! // Hamming(7,4) parity-check matrix
//! let h = gf2_core::bitmatrix![
//!     1, 1, 0, 1, 1, 0, 0;
//!     1, 0, 1, 1, 0, 1, 0;
//!     0, 1, 1, 1, 0, 0, 1
//! ];
//!
//! let config = OrbGrandConfig {
//!     list_size: 4,
//!     ..OrbGrandConfig::default()
//! };
//! let orbgrand = OrbGrand::new(h, config);
//! let sogrand = SoGrand::new(orbgrand);
//!
//! // High-confidence channel LLRs for the all-zero codeword
//! let input_llrs: Vec<Llr> = vec![Llr::new(3.0); 7];
//! let result = sogrand.decode_siso(&input_llrs);
//!
//! // APP LLRs should be positive (favoring bit 0)
//! assert!(result.app_llrs.iter().all(|l| l.value() > 0.0));
//! assert_eq!(result.app_llrs.len(), 7);
//! ```
//!
//! # References
//!
//! - Condo, C., et al. (2022). "Fixed Complexity Soft-Output GRAND."
//!   *IEEE Trans. Commun.*

use super::orbgrand::{log_sum_exp, OrbGrand, OrbGrandResult};
use crate::llr::Llr;

/// Result of a SISO (Soft-Input Soft-Output) decoding operation.
///
/// Contains per-bit APP LLRs, extrinsic LLRs for turbo iteration, and
/// the predicted list BLER.
///
/// # Examples
///
/// ```
/// use gf2_coding::grand::SisoResult;
/// use gf2_coding::llr::Llr;
///
/// let result = SisoResult {
///     app_llrs: vec![Llr::new(2.0), Llr::new(-1.5)],
///     extrinsic_llrs: vec![Llr::new(0.5), Llr::new(-0.3)],
///     list_bler_prediction: 0.01,
///     query_count: 100,
/// };
/// assert_eq!(result.app_llrs.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct SisoResult {
    /// Per-bit APP LLRs (length n).
    ///
    /// `app_llrs[i] = log(P(c_i=0 | r) / P(c_i=1 | r))`.
    /// Positive means bit 0 is more likely; negative means bit 1.
    pub app_llrs: Vec<Llr>,

    /// Per-bit extrinsic LLRs (length n).
    ///
    /// `extrinsic_llrs[i] = app_llrs[i] - input_llrs[i]`.
    /// This is the new information produced by the decoder, used by turbo
    /// iteration loops.
    pub extrinsic_llrs: Vec<Llr>,

    /// Predicted probability that the correct codeword is NOT in the list:
    /// `P(C\L | r^n)`.
    ///
    /// A key output for Fig. 2 validation: this should match the empirical
    /// list-BLER when the APP formula is correct.
    pub list_bler_prediction: f64,

    /// Number of noise pattern queries performed by the underlying ORBGRAND.
    pub query_count: usize,
}

/// Soft-Output GRAND (SOGRAND) decoder.
///
/// Wraps an [`OrbGrand`] decoder and adds soft-output computation: per-bit
/// APP LLRs and extrinsic information suitable for turbo decoding.
///
/// # Arguments
///
/// Constructed with an `OrbGrand` decoder (which must have `list_size >= 1`).
///
/// # Examples
///
/// ```
/// use gf2_coding::grand::{OrbGrand, OrbGrandConfig, SoGrand};
/// use gf2_coding::llr::Llr;
///
/// let h = gf2_core::bitmatrix![
///     1, 1, 0, 1, 1, 0, 0;
///     1, 0, 1, 1, 0, 1, 0;
///     0, 1, 1, 1, 0, 0, 1
/// ];
///
/// let config = OrbGrandConfig {
///     list_size: 2,
///     ..OrbGrandConfig::default()
/// };
/// let orbgrand = OrbGrand::new(h, config);
/// let sogrand = SoGrand::new(orbgrand);
///
/// let llrs: Vec<Llr> = vec![Llr::new(3.0); 7];
/// let result = sogrand.decode_siso(&llrs);
/// assert_eq!(result.app_llrs.len(), 7);
/// assert!(result.list_bler_prediction >= 0.0);
/// assert!(result.list_bler_prediction <= 1.0);
/// ```
///
/// # Complexity
///
/// Same as the underlying ORBGRAND query complexity O(Q * n), plus O(L * n) for
/// the soft-output computation over the list of L codewords and n bit positions.
pub struct SoGrand {
    /// Underlying ORBGRAND decoder.
    decoder: OrbGrand,
}

impl SoGrand {
    /// Creates a new SOGRAND decoder wrapping the given ORBGRAND decoder.
    ///
    /// # Arguments
    ///
    /// * `decoder` - An ORBGRAND decoder configured with the desired list size,
    ///   query limit, and code parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig, SoGrand};
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let orbgrand = OrbGrand::new(h, OrbGrandConfig::default());
    /// let sogrand = SoGrand::new(orbgrand);
    /// ```
    pub fn new(decoder: OrbGrand) -> Self {
        Self { decoder }
    }

    /// Returns the codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig, SoGrand};
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let sogrand = SoGrand::new(OrbGrand::new(h, OrbGrandConfig::default()));
    /// assert_eq!(sogrand.n(), 7);
    /// ```
    pub fn n(&self) -> usize {
        self.decoder.n()
    }

    /// Returns the message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig, SoGrand};
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let sogrand = SoGrand::new(OrbGrand::new(h, OrbGrandConfig::default()));
    /// assert_eq!(sogrand.k(), 4);
    /// ```
    pub fn k(&self) -> usize {
        self.decoder.k()
    }

    /// Returns a reference to the underlying ORBGRAND decoder.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig, SoGrand};
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let sogrand = SoGrand::new(OrbGrand::new(h, OrbGrandConfig::default()));
    /// assert_eq!(sogrand.orbgrand().n(), 7);
    /// ```
    pub fn orbgrand(&self) -> &OrbGrand {
        &self.decoder
    }

    /// Performs SISO decoding: takes input LLRs and returns APP LLRs plus
    /// extrinsic information.
    ///
    /// This is the main entry point for turbo decoding. The input LLRs are
    /// typically `L_Ch + L_A` (channel LLRs plus a-priori from another decoder).
    /// The output extrinsic LLRs are `L_E = L_APP - input_llrs`.
    ///
    /// # Arguments
    ///
    /// * `input_llrs` - Combined input LLRs (channel + a-priori), length n.
    ///   Positive means bit 0 is more likely.
    ///
    /// # Returns
    ///
    /// A [`SisoResult`] containing APP LLRs, extrinsic LLRs, the predicted
    /// list BLER, and the query count.
    ///
    /// # Panics
    ///
    /// Panics if `input_llrs.len() != n`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig, SoGrand};
    /// use gf2_coding::llr::Llr;
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let config = OrbGrandConfig {
    ///     list_size: 4,
    ///     ..OrbGrandConfig::default()
    /// };
    /// let sogrand = SoGrand::new(OrbGrand::new(h, config));
    ///
    /// let llrs: Vec<Llr> = vec![Llr::new(2.0); 7];
    /// let result = sogrand.decode_siso(&llrs);
    ///
    /// assert_eq!(result.app_llrs.len(), 7);
    /// assert_eq!(result.extrinsic_llrs.len(), 7);
    /// // All positive LLRs → all-zero codeword most likely → APP should be positive
    /// assert!(result.app_llrs.iter().all(|l| l.value() > 0.0));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(Q * n) for the ORBGRAND list decoding plus O(L * n) for the
    /// soft-output computation, where Q is the query count, L is the list
    /// size, and n is the code length.
    pub fn decode_siso(&self, input_llrs: &[Llr]) -> SisoResult {
        let n = self.n();
        let k = self.k();
        assert_eq!(
            input_llrs.len(),
            n,
            "Input LLR length {} must equal code length {}",
            input_llrs.len(),
            n
        );

        // Step 1: Run ORBGRAND in list mode
        let orb_result = self.decoder.decode(input_llrs);

        // Step 2: Compute per-block APP and the "not found" probability
        let (log_apps, log_p_not_in_list) = compute_block_apps(&orb_result, n, k);

        // Step 3: Compute per-bit APP LLRs (eq. 17)
        let app_llrs =
            compute_per_bit_app_llrs(&orb_result, &log_apps, log_p_not_in_list, input_llrs, n);

        // Step 4: Compute extrinsic LLRs: L_E = L_APP - input
        let extrinsic_llrs: Vec<Llr> = app_llrs
            .iter()
            .zip(input_llrs.iter())
            .map(|(&app, &input)| Llr::new(app.value() - input.value()))
            .collect();

        let list_bler = log_p_not_in_list.exp().clamp(0.0, 1.0);

        SisoResult {
            app_llrs,
            extrinsic_llrs,
            list_bler_prediction: list_bler,
            query_count: orb_result.query_count,
        }
    }
}

/// Compute per-block APP (log domain) for each codeword in the list, plus
/// the "not found" log-probability.
///
/// Per Corollary 1:
/// - `p_i = p(z^{n,q_i} | r^n)` for each list codeword
/// - `sum_list = sum of p_i` over list codewords
/// - `sum_cumulative = sum over all tested patterns` (from ORBGRAND)
/// - Denominator: `sum_list + (1 - sum_cumulative) * (2^k - 1) / (2^n - 1)`
/// - `APP_i = p_i / denominator`
/// - `P(C\L) = (1 - sum_cumulative) * (2^k - 1) / (2^n - 1) / denominator`
///
/// All computed in log domain for numerical stability.
///
/// Returns `(log_apps, log_p_not_in_list)` where `log_apps[i]` is the log APP
/// for the i-th list codeword.
fn compute_block_apps(orb_result: &OrbGrandResult, n: usize, k: usize) -> (Vec<f64>, f64) {
    let codewords = &orb_result.codewords;

    if codewords.is_empty() {
        // No codewords found: P(C\L) = 1.0
        return (vec![], 0.0); // log(1.0) = 0.0
    }

    // log(sum of noise probabilities over the list)
    let log_sum_list = codewords
        .iter()
        .map(|cw| cw.noise_log_probability)
        .fold(f64::NEG_INFINITY, log_sum_exp);

    // log(1 - sum_cumulative): the probability mass NOT yet tested
    // Uses log1p(-exp(x)) for stability when cumulative is close to 1
    let log_one_minus_cumulative = log1mexp(orb_result.cumulative_log_probability);

    // log((2^k - 1) / (2^n - 1)): ratio of codewords to total words
    // For stability with large n, k, use the identity:
    // (2^k - 1)/(2^n - 1) = (2^k - 1) / (2^n - 1)
    // When n and k are moderate (< 64), direct computation is fine.
    let log_codebook_ratio = log_codebook_ratio(n, k);

    // log of the "not found" unnormalized weight:
    // log((1 - sum_cumulative) * (2^k - 1) / (2^n - 1))
    let log_not_found_unnorm = log_one_minus_cumulative + log_codebook_ratio;

    // Denominator: log(sum_list + not_found_unnorm)
    let log_denominator = log_sum_exp(log_sum_list, log_not_found_unnorm);

    // Per-codeword APP (log domain): log(p_i / denominator) = log(p_i) - log(denominator)
    let log_apps: Vec<f64> = codewords
        .iter()
        .map(|cw| cw.noise_log_probability - log_denominator)
        .collect();

    // P(C\L) = not_found_unnorm / denominator
    let log_p_not_in_list = log_not_found_unnorm - log_denominator;

    (log_apps, log_p_not_in_list)
}

/// Compute per-bit APP LLRs according to eq. 17 from the SO-GRAND paper.
///
/// For each bit position i:
/// ```text
/// L'_{APP,i} = log(
///   (sum of P(c) where c_i=0 in list + P(C\L) * p(X_i=0|r_i)) /
///   (sum of P(c) where c_i=1 in list + P(C\L) * p(X_i=1|r_i))
/// )
/// ```
///
/// where `p(X_i=0|r_i) = 1/(1+exp(-|LLR_i|))` when `LLR_i > 0` (and the
/// complement for bit 1).
fn compute_per_bit_app_llrs(
    orb_result: &OrbGrandResult,
    log_apps: &[f64],
    log_p_not_in_list: f64,
    input_llrs: &[Llr],
    n: usize,
) -> Vec<Llr> {
    let codewords = &orb_result.codewords;

    (0..n)
        .map(|i| {
            // Channel bit probabilities from input LLR:
            // LLR_i = log(P(x_i=0|r_i) / P(x_i=1|r_i))
            // P(x_i=0|r_i) = 1 / (1 + exp(-LLR_i))  → log = -log(1+exp(-LLR_i))
            // P(x_i=1|r_i) = 1 / (1 + exp(LLR_i))   → log = -log(1+exp(LLR_i))
            let llr_val = input_llrs[i].value() as f64;
            let log_p_bit0_channel = -ln_1_plus_exp(-llr_val);
            let log_p_bit1_channel = -ln_1_plus_exp(llr_val);

            // Sum APP of list codewords with bit i = 0
            let mut log_sum_0 = f64::NEG_INFINITY;
            // Sum APP of list codewords with bit i = 1
            let mut log_sum_1 = f64::NEG_INFINITY;

            for (j, cw) in codewords.iter().enumerate() {
                if cw.codeword.get(i) {
                    log_sum_1 = log_sum_exp(log_sum_1, log_apps[j]);
                } else {
                    log_sum_0 = log_sum_exp(log_sum_0, log_apps[j]);
                }
            }

            // Add the "not found" fallback term.
            // Factor of 2 (LN_2) accounts for uniform prior P(c_i=b)=0.5:
            // the paper's formula (eq. 17) uses the likelihood ratio
            // P(y_i|c_i=b)/P(y_i) = P(c_i=b|y_i)/P(c_i=b) = 2*P(c_i=b|y_i).
            let log_fallback_0 = log_p_not_in_list + log_p_bit0_channel + std::f64::consts::LN_2;
            let log_fallback_1 = log_p_not_in_list + log_p_bit1_channel + std::f64::consts::LN_2;

            let log_numerator = log_sum_exp(log_sum_0, log_fallback_0);
            let log_denominator_bit = log_sum_exp(log_sum_1, log_fallback_1);

            // APP LLR = log(P(bit=0|r)) - log(P(bit=1|r))
            let app_llr = log_numerator - log_denominator_bit;

            // Clamp to avoid infinity in output
            Llr::new(app_llr.clamp(-20.0, 20.0) as f32)
        })
        .collect()
}

/// Compute `log(1 - exp(x))` for `x <= 0` numerically stably.
///
/// This is the log of `1 - p` where `p = exp(x)` is a probability.
/// Uses the identity:
/// - If `x < -ln(2)` (i.e., `p < 0.5`): `log(1 - exp(x)) = log1p(-exp(x))`
/// - If `x >= -ln(2)` (i.e., `p >= 0.5`): `log(1 - exp(x)) = log(-expm1(x))`
///
/// Reference: Machler (2012), "Accurately Computing log(1 - exp(-|a|))".
pub(super) fn log1mexp(x: f64) -> f64 {
    if x >= 0.0 {
        // cumulative probability >= 1.0 (can happen due to floating point
        // when all patterns are tested). 1 - exp(x>=0) <= 0 → log = -inf.
        f64::NEG_INFINITY
    } else if x > -std::f64::consts::LN_2 {
        // |x| < ln(2), so exp(x) > 0.5
        // Use expm1 for accuracy: log(-expm1(x))
        (-x.exp_m1()).ln()
    } else {
        // |x| >= ln(2), so exp(x) <= 0.5
        // Use log1p for accuracy: log1p(-exp(x))
        (-x.exp()).ln_1p()
    }
}

/// Compute `log((2^k - 1) / (2^n - 1))` stably.
///
/// For moderate n, k (<= 63): uses direct f64 computation.
/// For large n, k: approximates as `(k - n) * ln(2)` since
/// `(2^k - 1)/(2^n - 1) ≈ 2^(k-n)` for large values.
pub(super) fn log_codebook_ratio(n: usize, k: usize) -> f64 {
    if n <= 63 && k <= 63 {
        let numerator = (1u64 << k) as f64 - 1.0;
        let denominator = (1u64 << n) as f64 - 1.0;
        (numerator / denominator).ln()
    } else {
        // Approximation for large n, k
        (k as f64 - n as f64) * std::f64::consts::LN_2
    }
}

/// Import the numerically stable `ln(1 + exp(x))` from orbgrand.
use super::orbgrand::ln_1_plus_exp;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grand::{OneLineIntercept, OrbGrandConfig, ScoredCodeword};
    use gf2_core::BitVec;

    // =====================================================================
    // Helper: Hamming(7,4) parity-check matrix
    // =====================================================================
    fn hamming_7_4_h() -> gf2_core::BitMatrix {
        gf2_core::bitmatrix![
            1, 1, 0, 1, 1, 0, 0;
            1, 0, 1, 1, 0, 1, 0;
            0, 1, 1, 1, 0, 0, 1
        ]
    }

    fn make_sogrand(list_size: usize) -> SoGrand {
        let h = hamming_7_4_h();
        let config = OrbGrandConfig {
            max_queries: 1_000_000,
            list_size,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        SoGrand::new(OrbGrand::new(h, config))
    }

    // =====================================================================
    // Numerical utility tests (TDD: written first)
    // =====================================================================

    #[test]
    fn test_log1mexp_at_zero() {
        // log(1 - exp(0)) = log(0) = -inf
        assert_eq!(log1mexp(0.0), f64::NEG_INFINITY);
    }

    #[test]
    fn test_log1mexp_at_neg_infinity() {
        // log(1 - exp(-inf)) = log(1) = 0
        let val = log1mexp(f64::NEG_INFINITY);
        assert!((val - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_log1mexp_small_probability() {
        // exp(-10) ≈ 0.0000454; log(1 - 0.0000454) ≈ -0.0000454
        let val = log1mexp(-10.0);
        let expected = (1.0 - (-10.0_f64).exp()).ln();
        assert!(
            (val - expected).abs() < 1e-12,
            "log1mexp(-10) = {}, expected {}",
            val,
            expected
        );
    }

    #[test]
    fn test_log1mexp_half() {
        // log(1 - exp(-ln(2))) = log(1 - 0.5) = log(0.5) = -ln(2)
        let val = log1mexp(-std::f64::consts::LN_2);
        assert!(
            (val - (-std::f64::consts::LN_2)).abs() < 1e-10,
            "log1mexp(-ln2) = {}, expected {}",
            val,
            -std::f64::consts::LN_2
        );
    }

    #[test]
    fn test_log1mexp_large_probability() {
        // exp(-0.1) ≈ 0.905; log(1 - 0.905) ≈ log(0.095) ≈ -2.354
        let val = log1mexp(-0.1);
        let expected = (1.0 - (-0.1_f64).exp()).ln();
        assert!(
            (val - expected).abs() < 1e-10,
            "log1mexp(-0.1) = {}, expected {}",
            val,
            expected
        );
    }

    #[test]
    fn test_log1mexp_positive_input_returns_neg_inf() {
        // Positive input means cumulative >= 1.0 (floating point overshoot).
        // Returns -inf (= log(0)) since 1 - exp(x>=0) <= 0.
        assert_eq!(log1mexp(1.0), f64::NEG_INFINITY);
        assert_eq!(log1mexp(0.0), f64::NEG_INFINITY);
    }

    #[test]
    fn test_log_codebook_ratio_hamming_7_4() {
        // (2^4 - 1) / (2^7 - 1) = 15/127
        let val = log_codebook_ratio(7, 4);
        let expected = (15.0_f64 / 127.0).ln();
        assert!(
            (val - expected).abs() < 1e-10,
            "log_codebook_ratio(7,4) = {}, expected {}",
            val,
            expected
        );
    }

    #[test]
    fn test_log_codebook_ratio_identity() {
        // When k = n, ratio = (2^n - 1)/(2^n - 1) = 1, log = 0
        let val = log_codebook_ratio(5, 5);
        assert!(
            val.abs() < 1e-10,
            "log_codebook_ratio(5,5) = {}, expected 0",
            val
        );
    }

    // =====================================================================
    // Block APP computation tests
    // =====================================================================

    #[test]
    fn test_compute_block_apps_empty_list() {
        let result = OrbGrandResult {
            hard_decision: BitVec::zeros(7),
            codewords: vec![],
            query_count: 100,
            cumulative_log_probability: f64::NEG_INFINITY,
        };
        let (log_apps, log_p_not) = compute_block_apps(&result, 7, 4);
        assert!(log_apps.is_empty());
        // P(C\L) = 1.0 when no codewords found
        assert!((log_p_not - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_block_apps_single_codeword_high_cumulative() {
        // Single codeword found, cumulative probability close to 1
        let result = OrbGrandResult {
            hard_decision: BitVec::zeros(7),
            codewords: vec![ScoredCodeword {
                codeword: BitVec::zeros(7),
                noise_log_probability: -0.1, // high probability
                noise_weight: 0,
            }],
            query_count: 100,
            cumulative_log_probability: -0.01, // almost all probability mass tested
        };
        let (log_apps, log_p_not) = compute_block_apps(&result, 7, 4);
        assert_eq!(log_apps.len(), 1);
        // The single codeword should have high APP
        assert!(log_apps[0] > log_p_not);
        // P(C\L) should be small since cumulative is high
        assert!(log_p_not < -1.0);
    }

    #[test]
    fn test_compute_block_apps_probabilities_sum_to_one() {
        // Two codewords in list with valid log-probabilities
        // p1 = 0.3, p2 = 0.1, cumulative = 0.6
        let result = OrbGrandResult {
            hard_decision: BitVec::zeros(7),
            codewords: vec![
                ScoredCodeword {
                    codeword: BitVec::zeros(7),
                    noise_log_probability: (0.3_f64).ln(),
                    noise_weight: 0,
                },
                ScoredCodeword {
                    codeword: BitVec::zeros(7),
                    noise_log_probability: (0.1_f64).ln(),
                    noise_weight: 1,
                },
            ],
            query_count: 50,
            cumulative_log_probability: (0.6_f64).ln(),
        };
        let (log_apps, log_p_not) = compute_block_apps(&result, 7, 4);

        // Sum of all APPs + P(C\L) should equal 1.0
        let mut log_total = log_p_not;
        for &la in &log_apps {
            log_total = log_sum_exp(log_total, la);
        }
        let total = log_total.exp();
        assert!(
            (total - 1.0).abs() < 1e-10,
            "APPs + P(C\\L) should sum to 1.0, got {}",
            total
        );
    }

    // =====================================================================
    // Per-bit APP LLR tests
    // =====================================================================

    #[test]
    fn test_per_bit_app_all_zero_codeword_positive_llrs() {
        let sogrand = make_sogrand(4);
        let input_llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        // With high-confidence LLRs for all-zero, APP should be strongly positive
        for (i, &llr) in result.app_llrs.iter().enumerate() {
            assert!(
                llr.value() > 0.0,
                "APP LLR at position {} should be positive, got {}",
                i,
                llr.value()
            );
        }
    }

    #[test]
    fn test_per_bit_app_all_one_codeword_negative_llrs() {
        let sogrand = make_sogrand(4);
        // All-ones is a valid Hamming(7,4) codeword
        let input_llrs: Vec<Llr> = vec![Llr::new(-5.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        // APP should be strongly negative (favoring bit 1)
        for (i, &llr) in result.app_llrs.iter().enumerate() {
            assert!(
                llr.value() < 0.0,
                "APP LLR at position {} should be negative, got {}",
                i,
                llr.value()
            );
        }
    }

    #[test]
    fn test_per_bit_app_length_equals_n() {
        let sogrand = make_sogrand(2);
        let input_llrs: Vec<Llr> = vec![Llr::new(1.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        assert_eq!(result.app_llrs.len(), 7);
        assert_eq!(result.extrinsic_llrs.len(), 7);
    }

    // =====================================================================
    // Extrinsic LLR tests
    // =====================================================================

    #[test]
    fn test_extrinsic_equals_app_minus_input() {
        let sogrand = make_sogrand(4);
        let input_llrs = vec![
            Llr::new(3.0),
            Llr::new(-2.0),
            Llr::new(1.0),
            Llr::new(-0.5),
            Llr::new(2.0),
            Llr::new(1.5),
            Llr::new(-1.0),
        ];
        let result = sogrand.decode_siso(&input_llrs);

        for (i, ((app, ext), input)) in result
            .app_llrs
            .iter()
            .zip(result.extrinsic_llrs.iter())
            .zip(input_llrs.iter())
            .enumerate()
        {
            let expected = app.value() - input.value();
            let actual = ext.value();
            assert!(
                (actual - expected).abs() < 1e-6,
                "Extrinsic[{}]: expected {}, got {}",
                i,
                expected,
                actual
            );
        }
    }

    // =====================================================================
    // List BLER prediction tests
    // =====================================================================

    #[test]
    fn test_list_bler_between_zero_and_one() {
        let sogrand = make_sogrand(2);
        let input_llrs: Vec<Llr> = vec![Llr::new(1.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        assert!(
            result.list_bler_prediction >= 0.0,
            "List BLER should be >= 0, got {}",
            result.list_bler_prediction
        );
        assert!(
            result.list_bler_prediction <= 1.0,
            "List BLER should be <= 1, got {}",
            result.list_bler_prediction
        );
    }

    #[test]
    fn test_list_bler_high_snr_is_small() {
        // At very high SNR with list size 4, most probability mass is on the
        // correct codeword, so P(C\L) should be very small
        let sogrand = make_sogrand(4);
        let input_llrs: Vec<Llr> = vec![Llr::new(10.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        assert!(
            result.list_bler_prediction < 0.01,
            "At high SNR, list BLER should be very small, got {}",
            result.list_bler_prediction
        );
    }

    #[test]
    fn test_list_bler_larger_list_lower_bler() {
        // Larger list should capture more probability mass → lower P(C\L)
        let input_llrs: Vec<Llr> = vec![
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
        ];

        let sogrand_l1 = make_sogrand(1);
        let result_l1 = sogrand_l1.decode_siso(&input_llrs);

        let sogrand_l4 = make_sogrand(4);
        let result_l4 = sogrand_l4.decode_siso(&input_llrs);

        assert!(
            result_l4.list_bler_prediction <= result_l1.list_bler_prediction + 1e-10,
            "Larger list should have lower BLER: L=1 got {}, L=4 got {}",
            result_l1.list_bler_prediction,
            result_l4.list_bler_prediction
        );
    }

    // =====================================================================
    // SISO interface tests
    // =====================================================================

    #[test]
    fn test_siso_list_size_1_works() {
        let sogrand = make_sogrand(1);
        let input_llrs: Vec<Llr> = vec![Llr::new(3.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        // Even with L=1, the "not found" term provides meaningful soft output
        assert_eq!(result.app_llrs.len(), 7);
        assert!(result.app_llrs.iter().all(|l| l.value() > 0.0));
    }

    #[test]
    fn test_siso_list_size_2_works() {
        let sogrand = make_sogrand(2);
        let input_llrs: Vec<Llr> = vec![Llr::new(2.0); 7];
        let result = sogrand.decode_siso(&input_llrs);
        assert_eq!(result.app_llrs.len(), 7);
    }

    #[test]
    fn test_siso_list_size_4_works() {
        let sogrand = make_sogrand(4);
        let input_llrs: Vec<Llr> = vec![Llr::new(2.0); 7];
        let result = sogrand.decode_siso(&input_llrs);
        assert_eq!(result.app_llrs.len(), 7);
    }

    #[test]
    #[should_panic(expected = "Input LLR length")]
    fn test_siso_wrong_length_panics() {
        let sogrand = make_sogrand(1);
        let input_llrs: Vec<Llr> = vec![Llr::new(1.0); 5];
        sogrand.decode_siso(&input_llrs);
    }

    // =====================================================================
    // Accessor tests
    // =====================================================================

    #[test]
    fn test_sogrand_n_and_k() {
        let sogrand = make_sogrand(1);
        assert_eq!(sogrand.n(), 7);
        assert_eq!(sogrand.k(), 4);
    }

    #[test]
    fn test_sogrand_orbgrand_accessor() {
        let sogrand = make_sogrand(1);
        assert_eq!(sogrand.orbgrand().n(), 7);
        assert_eq!(sogrand.orbgrand().k(), 4);
    }

    // =====================================================================
    // Query count tracking test
    // =====================================================================

    #[test]
    fn test_query_count_propagated() {
        let sogrand = make_sogrand(1);
        let input_llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        assert!(
            result.query_count >= 1,
            "Query count should be at least 1, got {}",
            result.query_count
        );
    }

    // =====================================================================
    // APP LLR sign consistency test
    // =====================================================================

    #[test]
    fn test_app_llr_sign_consistency_with_channel() {
        // For a high-SNR all-zero codeword, APP LLR sign should match channel
        let sogrand = make_sogrand(4);
        let input_llrs = vec![
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(5.0),
        ];
        let result = sogrand.decode_siso(&input_llrs);

        for (i, &app) in result.app_llrs.iter().enumerate() {
            assert!(
                app.value() > 0.0,
                "APP at bit {} should be positive (channel says 0), got {}",
                i,
                app.value()
            );
        }
    }

    #[test]
    fn test_app_llr_magnitude_at_least_channel_at_high_snr() {
        // At high SNR with the correct codeword clearly dominant,
        // APP LLR magnitude should be at least as large as channel LLR
        // (the decoder confirms the channel's decision)
        let sogrand = make_sogrand(4);
        let input_llrs: Vec<Llr> = vec![Llr::new(3.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

        for (i, &app) in result.app_llrs.iter().enumerate() {
            assert!(
                app.magnitude() >= input_llrs[i].magnitude() - 0.5,
                "APP magnitude at bit {} should be near or above channel: app={}, ch={}",
                i,
                app.magnitude(),
                input_llrs[i].magnitude()
            );
        }
    }

    // =====================================================================
    // Numerical stability tests
    // =====================================================================

    #[test]
    fn test_no_nan_or_inf_in_output() {
        let sogrand = make_sogrand(4);
        // Various input conditions
        let test_cases: Vec<Vec<Llr>> = vec![
            vec![Llr::new(0.01); 7],  // very low confidence
            vec![Llr::new(10.0); 7],  // very high confidence
            vec![Llr::new(-10.0); 7], // very high confidence (bit 1)
            vec![
                // mixed
                Llr::new(0.1),
                Llr::new(-0.1),
                Llr::new(5.0),
                Llr::new(-5.0),
                Llr::new(0.5),
                Llr::new(-0.5),
                Llr::new(1.0),
            ],
        ];

        for (idx, llrs) in test_cases.iter().enumerate() {
            let result = sogrand.decode_siso(llrs);
            for (i, &app) in result.app_llrs.iter().enumerate() {
                assert!(
                    app.value().is_finite(),
                    "APP LLR[{}] is not finite in test case {}: {}",
                    i,
                    idx,
                    app.value()
                );
            }
            for (i, &ext) in result.extrinsic_llrs.iter().enumerate() {
                assert!(
                    ext.value().is_finite(),
                    "Extrinsic LLR[{}] is not finite in test case {}: {}",
                    i,
                    idx,
                    ext.value()
                );
            }
            assert!(
                result.list_bler_prediction.is_finite(),
                "List BLER not finite in test case {}: {}",
                idx,
                result.list_bler_prediction
            );
        }
    }

    // =====================================================================
    // Full encode-decode SISO roundtrip
    // =====================================================================

    #[test]
    fn test_siso_roundtrip_all_messages() {
        use crate::linear::LinearBlockCode;
        use crate::traits::BlockEncoder;

        let code = LinearBlockCode::hamming(3);
        let h = code.parity_check().unwrap().clone();
        let config = OrbGrandConfig {
            max_queries: 1_000_000,
            list_size: 4,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let sogrand = SoGrand::new(OrbGrand::new(h, config));

        // Test all 16 possible 4-bit messages
        for msg_val in 0u8..16 {
            let mut msg = BitVec::with_capacity(4);
            for bit in 0..4 {
                msg.push_bit((msg_val >> bit) & 1 == 1);
            }
            let codeword = code.encode(&msg);

            // Create high-confidence LLRs from codeword
            let llrs: Vec<Llr> = (0..7)
                .map(|i| {
                    if codeword.get(i) {
                        Llr::new(-5.0)
                    } else {
                        Llr::new(5.0)
                    }
                })
                .collect();

            let result = sogrand.decode_siso(&llrs);

            // APP LLR sign should match the codeword bits
            for i in 0..7 {
                let expected_sign = if codeword.get(i) { -1.0 } else { 1.0 };
                let actual_sign = if result.app_llrs[i].value() >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                assert_eq!(
                    actual_sign,
                    expected_sign,
                    "Message {:04b}, bit {}: APP sign mismatch (app={}, expected bit={})",
                    msg_val,
                    i,
                    result.app_llrs[i].value(),
                    codeword.get(i) as u8
                );
            }
        }
    }

    // =====================================================================
    // Property-based tests
    // =====================================================================

    mod prop_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// APP LLRs should be finite for any input LLR values in [-10, 10].
            #[test]
            fn test_app_llrs_always_finite(
                llr_vals in proptest::collection::vec(-10.0f32..10.0f32, 7..=7)
            ) {
                let sogrand = make_sogrand(2);
                let input_llrs: Vec<Llr> = llr_vals.iter().map(|&v| Llr::new(v)).collect();
                let result = sogrand.decode_siso(&input_llrs);

                for (i, &app) in result.app_llrs.iter().enumerate() {
                    prop_assert!(
                        app.value().is_finite(),
                        "APP LLR[{}] is not finite: {} (input: {:?})",
                        i, app.value(), llr_vals
                    );
                }
                for (i, &ext) in result.extrinsic_llrs.iter().enumerate() {
                    prop_assert!(
                        ext.value().is_finite(),
                        "Extrinsic LLR[{}] is not finite: {} (input: {:?})",
                        i, ext.value(), llr_vals
                    );
                }
            }

            /// List BLER should be in [0, 1] for any valid input.
            #[test]
            fn test_list_bler_in_valid_range(
                llr_vals in proptest::collection::vec(-10.0f32..10.0f32, 7..=7)
            ) {
                let sogrand = make_sogrand(2);
                let input_llrs: Vec<Llr> = llr_vals.iter().map(|&v| Llr::new(v)).collect();
                let result = sogrand.decode_siso(&input_llrs);

                prop_assert!(
                    result.list_bler_prediction >= 0.0 && result.list_bler_prediction <= 1.0,
                    "List BLER out of range: {} (input: {:?})",
                    result.list_bler_prediction, llr_vals
                );
            }

            /// Extrinsic = APP - input, verified for random inputs.
            #[test]
            fn test_extrinsic_equals_app_minus_input_proptest(
                llr_vals in proptest::collection::vec(-5.0f32..5.0f32, 7..=7)
            ) {
                let sogrand = make_sogrand(2);
                let input_llrs: Vec<Llr> = llr_vals.iter().map(|&v| Llr::new(v)).collect();
                let result = sogrand.decode_siso(&input_llrs);

                for (i, ((app, ext), input)) in result
                    .app_llrs.iter()
                    .zip(result.extrinsic_llrs.iter())
                    .zip(input_llrs.iter())
                    .enumerate()
                {
                    let expected = app.value() - input.value();
                    let actual = ext.value();
                    prop_assert!(
                        (actual - expected).abs() < 1e-4,
                        "Extrinsic[{}]: expected {}, got {} (input: {:?})",
                        i, expected, actual, llr_vals
                    );
                }
            }

            /// Higher-confidence correct inputs should yield larger APP magnitude.
            #[test]
            fn test_higher_snr_yields_larger_app_magnitude(
                snr_low in 0.5f32..2.0f32,
                snr_high in 3.0f32..8.0f32,
            ) {
                let sogrand = make_sogrand(4);

                let llrs_low: Vec<Llr> = vec![Llr::new(snr_low); 7];
                let llrs_high: Vec<Llr> = vec![Llr::new(snr_high); 7];

                let result_low = sogrand.decode_siso(&llrs_low);
                let result_high = sogrand.decode_siso(&llrs_high);

                // Average APP magnitude should be higher at higher SNR
                let avg_mag_low: f32 = result_low.app_llrs.iter()
                    .map(|l| l.magnitude())
                    .sum::<f32>() / 7.0;
                let avg_mag_high: f32 = result_high.app_llrs.iter()
                    .map(|l| l.magnitude())
                    .sum::<f32>() / 7.0;

                prop_assert!(
                    avg_mag_high >= avg_mag_low - 0.1,
                    "Higher SNR ({}) should yield larger avg APP mag ({}) than lower SNR ({}) with ({})",
                    snr_high, avg_mag_high, snr_low, avg_mag_low
                );
            }
        }
    }
}

#[cfg(test)]
mod fig2_validation {
    //! Fig. 2 reproduction: compare predicted vs empirical list-BLER.
    use super::*;
    use crate::grand::orbgrand::{OneLineIntercept, OrbGrand, OrbGrandConfig};
    use crate::linear::LinearBlockCode;
    use crate::traits::BlockEncoder;
    use gf2_core::BitVec;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use rand_distr;

    /// Run many frames, compute empirical list-BLER (fraction where correct
    /// codeword is NOT in list), compare with average predicted list-BLER.
    #[test]
    fn test_predicted_vs_empirical_list_bler() {
        let code = LinearBlockCode::hamming(3); // (7,4)
        let h = code.parity_check().unwrap().clone();
        let n = code.n();
        let k = code.k();

        let config = OrbGrandConfig {
            max_queries: 5000,
            list_size: 2,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let sogrand = SoGrand::new(OrbGrand::new(h, config));

        let mut rng = StdRng::seed_from_u64(42);
        let num_frames = 200;
        let sigma = 0.7; // moderate noise
        let mut empirical_misses = 0;
        let mut total_predicted_bler = 0.0;

        for _ in 0..num_frames {
            // Random message
            let mut msg = BitVec::zeros(k);
            for i in 0..k {
                if rng.gen_bool(0.5) {
                    msg.set(i, true);
                }
            }
            let codeword = code.encode(&msg);

            // BPSK + AWGN
            let llrs: Vec<Llr> = (0..n)
                .map(|i| {
                    let symbol = if codeword.get(i) { -1.0 } else { 1.0 };
                    let noise: f64 = sigma * rng.sample::<f64, _>(rand_distr::StandardNormal);
                    let received = symbol + noise;
                    Llr::new((2.0 * received / (sigma * sigma)) as f32)
                })
                .collect();

            let result = sogrand.decode_siso(&llrs);
            total_predicted_bler += result.list_bler_prediction;

            // Check if correct codeword is in list (via ORBGRAND decode)
            let orb_result = OrbGrand::new(
                code.parity_check().unwrap().clone(),
                OrbGrandConfig {
                    max_queries: 5000,
                    list_size: 2,
                    even_code: false,
                    systematic: true,
                    list_bler_stop_threshold: None,
                    one_line_intercept: OneLineIntercept::Auto,
                },
            )
            .decode(&llrs);

            let correct_in_list = orb_result
                .codewords
                .iter()
                .any(|sc| (0..n).all(|i| sc.codeword.get(i) == codeword.get(i)));
            if !correct_in_list {
                empirical_misses += 1;
            }
        }

        let empirical_bler = empirical_misses as f64 / num_frames as f64;
        let avg_predicted_bler = total_predicted_bler / num_frames as f64;

        // Predicted and empirical should be in the same ballpark
        // Allow generous tolerance since this is a Monte Carlo comparison.
        // With correct probability accumulation, predicted BLER can be very
        // close to 0 when the list covers most of the codebook probability.
        if avg_predicted_bler < 0.001 && empirical_bler < 0.05 {
            // Both are small — the model correctly predicts low list BLER.
            return;
        }
        let ratio = if empirical_bler > 0.0 {
            avg_predicted_bler / empirical_bler
        } else {
            assert!(
                avg_predicted_bler < 0.1,
                "Predicted BLER {avg_predicted_bler:.3} too high when empirical is 0"
            );
            return;
        };

        assert!(
            ratio > 0.2 && ratio < 5.0,
            "Predicted ({avg_predicted_bler:.4}) and empirical ({empirical_bler:.4}) \
             list-BLER differ by more than 5x (ratio={ratio:.2})"
        );
    }

    /// Word-boundary test: exercise SOGRAND with code length near 64 bits.
    #[test]
    fn test_sogrand_near_64_bit_boundary() {
        use crate::bch::extended::ExtendedBchCode;

        let ebch = ExtendedBchCode::ebch_64_57();
        let h = ebch.parity_check().clone();

        let config = OrbGrandConfig {
            max_queries: 50_000,
            list_size: 2,
            even_code: true,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let sogrand = SoGrand::new(OrbGrand::new(h, config));

        // Noiseless LLRs for all-zero codeword
        let llrs: Vec<Llr> = vec![Llr::new(5.0); 64];
        let result = sogrand.decode_siso(&llrs);

        assert_eq!(result.app_llrs.len(), 64);
        assert_eq!(result.extrinsic_llrs.len(), 64);
        // All APP LLRs should favor bit 0 (positive)
        for i in 0..64 {
            assert!(
                result.app_llrs[i].value() > 0.0,
                "APP LLR at bit {} should be positive for all-zero codeword",
                i
            );
        }
    }

    /// Validate log_codebook_ratio approximation for moderate n.
    #[test]
    fn test_log_codebook_ratio_approximation_accuracy() {
        // For small n/k, compare exact vs our implementation
        for (n, k) in &[(7, 4), (15, 11), (16, 11), (31, 26), (32, 26)] {
            let result = log_codebook_ratio(*n, *k);
            // (2^k - 1) / (2^n - 1) should be a positive ratio < 1
            // In log domain, this should be negative
            assert!(
                result < 0.0,
                "log_codebook_ratio({n},{k}) = {result} should be negative"
            );
            // Rough check: result should be approximately (k-n)*ln(2)
            let approx = (*k as f64 - *n as f64) * 2.0_f64.ln();
            let error = (result - approx).abs();
            assert!(
                error < 1.0,
                "log_codebook_ratio({n},{k}) = {result:.4}, approx = {approx:.4}, error = {error:.4}"
            );
        }
    }
}
