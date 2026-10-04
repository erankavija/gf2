//! Soft-Output GRAND (SOGRAND, `@/citation/Yuan2025`).
//!
//! [`SoGrand`] wraps an [`OrbGrand`] list decoder and turns its list into
//! per-bit a-posteriori probability (APP) LLRs, extrinsic LLRs and the
//! predicted list BLER `P(C\L)`, computed in the log domain.

use super::orbgrand::{log_sum_exp, OrbGrand, OrbGrandResult};
use crate::llr::Llr;

/// Result of a SISO (Soft-Input Soft-Output) decoding operation.
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
    pub extrinsic_llrs: Vec<Llr>,

    /// Predicted probability that the correct codeword is NOT in the list:
    /// `P(C\L | r^n)`.
    pub list_bler_prediction: f64,

    /// Number of noise pattern queries performed by the underlying ORBGRAND.
    pub query_count: usize,
}

/// Soft-Output GRAND (SOGRAND) decoder: an [`OrbGrand`] list decoder plus the
/// per-bit APP and extrinsic LLR computation.
pub struct SoGrand {
    decoder: OrbGrand,
}

impl SoGrand {
    /// Creates a SOGRAND decoder around `decoder`, whose configuration sets
    /// the list size and query limit.
    pub fn new(decoder: OrbGrand) -> Self {
        Self { decoder }
    }

    /// Returns the codeword length.
    pub fn n(&self) -> usize {
        self.decoder.n()
    }

    /// Returns the message length.
    pub fn k(&self) -> usize {
        self.decoder.k()
    }

    /// Returns a reference to the underlying ORBGRAND decoder.
    pub fn orbgrand(&self) -> &OrbGrand {
        &self.decoder
    }

    /// Performs SISO decoding of `input_llrs` (channel plus a-priori LLRs,
    /// positive favouring bit 0).
    ///
    /// The extrinsic LLRs are `L_E = L_APP - input_llrs`; APP LLRs are
    /// clamped to `[-20, 20]`.
    ///
    /// # Panics
    ///
    /// Panics if `input_llrs.len() != n`.
    /// Panics if any LLR has a NaN magnitude.
    ///
    /// # Complexity
    ///
    /// The cost of [`OrbGrand::decode`] plus O(L × n) for a list of L
    /// codewords.
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

        let orb_result = self.decoder.decode(input_llrs);

        let (log_apps, log_p_not_in_list) = compute_block_apps(&orb_result, n, k);

        let app_llrs =
            compute_per_bit_app_llrs(&orb_result, &log_apps, log_p_not_in_list, input_llrs, n);

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
/// Per `@/citation/Yuan2025` Corollary 1, for a general (not even) code:
/// - `p_i = p(z^{n,q_i} | r^n)` for each list codeword
/// - `sum_list = sum of p_i` over list codewords
/// - `sum_cumulative = sum over all tested patterns` (from ORBGRAND)
/// - Denominator: `sum_list + (1 - sum_cumulative) * (2^k - 1) / (2^n - 1)`
/// - `APP_i = p_i / denominator`
/// - `P(C\L) = (1 - sum_cumulative) * (2^k - 1) / (2^n - 1) / denominator`
///
/// Returns `(log_apps, log_p_not_in_list)` where `log_apps[i]` is the log APP
/// for the i-th list codeword.
fn compute_block_apps(orb_result: &OrbGrandResult, n: usize, k: usize) -> (Vec<f64>, f64) {
    let codewords = &orb_result.codewords;

    if codewords.is_empty() {
        // No codewords found: P(C\L) = 1.0
        return (vec![], 0.0); // log(1.0) = 0.0
    }

    let log_sum_list = codewords
        .iter()
        .map(|cw| cw.noise_log_probability)
        .fold(f64::NEG_INFINITY, log_sum_exp);

    // Untested-mass ceiling is `log_parity_cap`, which is `log(1) = 0`
    // for non-even codes and `log P(parity(Z) = hard_parity)` for even
    // codes: the initial `P_notGuess` of `@/citation/Yuan2025` § III,
    // eq. (17).
    let log_not_tested_mass = log_cap_minus_exp(
        orb_result.cumulative_log_probability,
        orb_result.log_parity_cap,
    );

    // log((2^k - 1) / (2^n - 1)) for general codes, or
    // log((2^k - 1) / (2^(n-1) - 1)) ≈ 2^-(s-1) for even codes
    // (`@/citation/Yuan2025` eq. (17) correction: all 2^k codewords carry
    // the hard-decision parity while only 2^(n-1) binary words do).
    let log_codebook_ratio = log_codebook_ratio_for_code(n, k, orb_result.even_code);

    let log_not_found_unnorm = log_not_tested_mass + log_codebook_ratio;

    let log_denominator = log_sum_exp(log_sum_list, log_not_found_unnorm);

    let log_apps: Vec<f64> = codewords
        .iter()
        .map(|cw| cw.noise_log_probability - log_denominator)
        .collect();

    let log_p_not_in_list = log_not_found_unnorm - log_denominator;

    (log_apps, log_p_not_in_list)
}

/// Compute per-bit APP LLRs according to `@/citation/Yuan2025` eq. (17).
///
/// For each bit position i:
/// ```text
/// L'_{APP,i} = log(
///   (sum of P(c) where c_i=0 in list + P(C\L) * p(X_i=0|r_i)) /
///   (sum of P(c) where c_i=1 in list + P(C\L) * p(X_i=1|r_i))
/// )
/// ```
///
/// where `p(X_i=0|r_i) = 1/(1+exp(-LLR_i))` is the channel bit posterior.
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
            let llr_val = input_llrs[i].value() as f64;
            let log_p_bit0_channel = -ln_1_plus_exp(-llr_val);
            let log_p_bit1_channel = -ln_1_plus_exp(llr_val);

            let mut log_sum_0 = f64::NEG_INFINITY;
            let mut log_sum_1 = f64::NEG_INFINITY;

            for (j, cw) in codewords.iter().enumerate() {
                if cw.codeword.get(i) {
                    log_sum_1 = log_sum_exp(log_sum_1, log_apps[j]);
                } else {
                    log_sum_0 = log_sum_exp(log_sum_0, log_apps[j]);
                }
            }

            // `p(x_i = b | r_i)` is the channel bit posterior, not the
            // likelihood (`@/citation/Yuan2025` § III.C). `P_notL` is
            // jointly normalised with the list APPs in
            // `compute_block_apps` (sum_L APP + P_notL = 1), so summing
            // the fallback over bit values yields `P_notL · (p(0|r_i) +
            // p(1|r_i)) = P_notL` with no factor-of-2 adjustment.
            let log_fallback_0 = log_p_not_in_list + log_p_bit0_channel;
            let log_fallback_1 = log_p_not_in_list + log_p_bit1_channel;

            let log_numerator = log_sum_exp(log_sum_0, log_fallback_0);
            let log_denominator_bit = log_sum_exp(log_sum_1, log_fallback_1);

            let app_llr = log_numerator - log_denominator_bit;

            // Clamp to avoid infinity in output
            Llr::new(app_llr.clamp(-20.0, 20.0) as f32)
        })
        .collect()
}

/// Compute `log(1 - exp(x))` for `x <= 0` numerically stably.
///
/// Uses the identity of `@/citation/Machler2012`:
/// - If `x <= -ln(2)` (`exp(x) <= 0.5`): `log(1 - exp(x)) = log1p(-exp(x))`
/// - If `x > -ln(2)` (`exp(x) > 0.5`): `log(1 - exp(x)) = log(-expm1(x))`
pub(super) fn log1mexp(x: f64) -> f64 {
    if x >= 0.0 {
        // cumulative probability >= 1.0 (can happen due to floating point
        // when all patterns are tested). 1 - exp(x>=0) <= 0 → log = -inf.
        f64::NEG_INFINITY
    } else if x > -std::f64::consts::LN_2 {
        (-x.exp_m1()).ln()
    } else {
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
        (k as f64 - n as f64) * std::f64::consts::LN_2
    }
}

/// Codebook ratio with the even-code correction of `@/citation/Yuan2025`
/// eq. (17).
///
/// For a general code the "not found" weight in the SO-GRAND APP
/// denominator uses `(2^k − 1) / (2^n − 1) ≈ 2^−s` where `s = n − k`.
/// For an even code (every codeword has even Hamming weight) only the
/// parity-consistent half of the `2^n` binary words is reachable by the
/// noise, while all `2^k` codewords still are, so the ratio doubles:
/// `(2^k − 1) / (2^(n−1) − 1) ≈ 2^−(s−1)`.
///
/// In the log domain this is [`log_codebook_ratio`] + `ln(2)` (the
/// `−1`-in-the-exponent adjustment), with a small exact correction in
/// the `n ≤ 63` branch because `2^(n−1) − 1` is not exactly half of
/// `2^n − 1`.
pub(super) fn log_codebook_ratio_for_code(n: usize, k: usize, even_code: bool) -> f64 {
    if !even_code {
        return log_codebook_ratio(n, k);
    }
    if (1..=63).contains(&n) && k <= 63 {
        let numerator = (1u64 << k) as f64 - 1.0;
        // 2^(n-1) - 1 is the count of parity-consistent non-zero binary
        // words. For n = 1 this is 0, but even codes of length 1 are
        // degenerate (only the zero codeword); fall through to the
        // ln(2)-shift approximation in that corner.
        let denominator = (1u64 << (n - 1)) as f64 - 1.0;
        if denominator > 0.0 {
            return (numerator / denominator).ln();
        }
    }
    // Large-n approximation: 2^(k - (n - 1)) = 2 · 2^(k - n).
    log_codebook_ratio(n, k) + std::f64::consts::LN_2
}

/// Compute `log(exp(cap) - exp(x))` stably for `x <= cap`.
///
/// The untested mass: `P_notGuess` of `@/citation/Yuan2025` starts
/// at `exp(cap)` (= 1 in general, `prob_parity(hard_parity, |L|)` for
/// even codes) and decrements by the probability of each tested
/// pattern, so after `Q` queries the remaining mass is
/// `exp(cap) - exp(cumulative_log_prob)`.
///
/// Equal to `cap + log1mexp(x - cap)` (`@/citation/Machler2012`), which
/// specialises to `log1mexp(x)` when `cap = 0`.  Returns
/// `f64::NEG_INFINITY` when `x >= cap`, which floating-point noise produces
/// once the scan exhausts the reachable mass.
pub(super) fn log_cap_minus_exp(x: f64, cap: f64) -> f64 {
    if !cap.is_finite() && cap == f64::NEG_INFINITY {
        // No reachable mass at all: degenerate. Untested = 0.
        return f64::NEG_INFINITY;
    }
    if x == f64::NEG_INFINITY {
        return cap;
    }
    if x >= cap {
        return f64::NEG_INFINITY;
    }
    cap + log1mexp(x - cap)
}

use super::orbgrand::ln_1_plus_exp;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grand::{OneLineIntercept, OrbGrandConfig, ScoredCodeword};
    use gf2_core::BitVec;

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

    #[test]
    fn test_log_codebook_ratio_for_code_non_even_matches_baseline() {
        for (n, k) in [(7, 4), (16, 11), (32, 26), (64, 57)] {
            let baseline = log_codebook_ratio(n, k);
            let general = log_codebook_ratio_for_code(n, k, false);
            assert!(
                (baseline - general).abs() < 1e-12,
                "({n},{k}) baseline={baseline} general={general}"
            );
        }
    }

    #[test]
    fn test_log_codebook_ratio_for_code_even_is_ln2_shift() {
        // Even-code correction is (approximately) `+ ln(2)` relative to
        // the general ratio — exactly so in the large-n approximation,
        // and very close for moderate n (the exact form uses
        // `2^(n-1) - 1` rather than `(2^n - 1)/2`).
        let ln2 = std::f64::consts::LN_2;
        for (n, k) in [(16, 11), (32, 26), (64, 57)] {
            let general = log_codebook_ratio_for_code(n, k, false);
            let even = log_codebook_ratio_for_code(n, k, true);
            let shift = even - general;
            assert!(
                (shift - ln2).abs() < 5e-3,
                "({n},{k}) shift={shift}, expected ≈ ln2={ln2}"
            );
        }
    }

    #[test]
    fn test_log_cap_minus_exp_cap_one_matches_log1mexp() {
        for x in [-10.0, -1.0, -0.5, -0.1, -0.01] {
            let cap = log_cap_minus_exp(x, 0.0);
            let base = log1mexp(x);
            assert!((cap - base).abs() < 1e-12, "x={x}: cap={cap} base={base}");
        }
    }

    #[test]
    fn test_log_cap_minus_exp_decrements_under_parity_cap() {
        let cap = -std::f64::consts::LN_2;
        assert!((log_cap_minus_exp(f64::NEG_INFINITY, cap) - cap).abs() < 1e-12);
        assert_eq!(log_cap_minus_exp(cap, cap), f64::NEG_INFINITY);
        let x = cap - 0.1;
        let val = log_cap_minus_exp(x, cap);
        let expected = cap + (1.0 - (-0.1_f64).exp()).ln();
        assert!(
            (val - expected).abs() < 1e-10,
            "log_cap_minus_exp({x}, {cap}) = {val}, expected {expected}"
        );
    }

    #[test]
    fn test_log_cap_minus_exp_x_above_cap_returns_neg_inf() {
        // x > cap can only happen via numerical overshoot when the scan
        // exhausts the reachable mass; return -inf to avoid NaN.
        assert_eq!(log_cap_minus_exp(-0.1, -0.2), f64::NEG_INFINITY);
    }

    #[test]
    fn test_log_prob_parity_all_zero_llr_is_uniform() {
        // |L| = 0 ⇒ tanh(0) = 0 ⇒ product = 0 ⇒ P(parity) = 0.5.
        use crate::grand::orbgrand::log_prob_parity;
        let absl = vec![0.0, 0.0, 0.0];
        let even = log_prob_parity(&absl, false);
        let odd = log_prob_parity(&absl, true);
        let ln_half = -std::f64::consts::LN_2;
        assert!((even - ln_half).abs() < 1e-12);
        assert!((odd - ln_half).abs() < 1e-12);
    }

    #[test]
    fn test_log_prob_parity_large_llr_collapses_to_even_parity() {
        // |L| large ⇒ tanh → 1 ⇒ prod tanh → 1 ⇒ P(even) → 1, P(odd) → 0.
        use crate::grand::orbgrand::log_prob_parity;
        let absl = vec![20.0; 4];
        let even = log_prob_parity(&absl, false);
        let odd = log_prob_parity(&absl, true);
        assert!(even < 0.0 && even > -1e-3, "log P(even)={}", even);
        assert!(odd < -10.0, "log P(odd)={}", odd);
    }

    #[test]
    fn test_log_prob_parity_two_bit_closed_form() {
        // For n = 2 independent bit flips with p_i = 1/(1 + exp(|L|)),
        // P(even) = p1*p2 + (1-p1)*(1-p2); P(odd) = p1*(1-p2) + (1-p1)*p2.
        use crate::grand::orbgrand::log_prob_parity;
        let l1 = 1.5_f64;
        let l2 = 2.5_f64;
        let p1 = 1.0 / (1.0 + l1.exp());
        let p2 = 1.0 / (1.0 + l2.exp());
        let p_even = p1 * p2 + (1.0 - p1) * (1.0 - p2);
        let p_odd = p1 * (1.0 - p2) + (1.0 - p1) * p2;
        assert!((p_even + p_odd - 1.0).abs() < 1e-12);

        let absl = vec![l1, l2];
        let log_even = log_prob_parity(&absl, false);
        let log_odd = log_prob_parity(&absl, true);
        assert!(
            (log_even - p_even.ln()).abs() < 1e-10,
            "even: got {log_even}, expected {}",
            p_even.ln()
        );
        assert!(
            (log_odd - p_odd.ln()).abs() < 1e-10,
            "odd: got {log_odd}, expected {}",
            p_odd.ln()
        );
    }

    #[test]
    fn test_log_prob_parity_sums_to_one() {
        use crate::grand::orbgrand::log_prob_parity;
        let absl = vec![0.1, 0.7, 1.3, 2.0, 5.0];
        let log_even = log_prob_parity(&absl, false);
        let log_odd = log_prob_parity(&absl, true);
        let sum = log_sum_exp(log_even, log_odd);
        assert!(sum.abs() < 1e-10, "log(P_even + P_odd) = {sum}, expected 0");
    }

    #[test]
    fn test_compute_block_apps_empty_list() {
        let result = OrbGrandResult {
            hard_decision: BitVec::zeros(7),
            codewords: vec![],
            query_count: 100,
            cumulative_log_probability: f64::NEG_INFINITY,
            log_parity_cap: 0.0,
            even_code: false,
        };
        let (log_apps, log_p_not) = compute_block_apps(&result, 7, 4);
        assert!(log_apps.is_empty());
        assert!((log_p_not - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_block_apps_single_codeword_high_cumulative() {
        let result = OrbGrandResult {
            hard_decision: BitVec::zeros(7),
            codewords: vec![ScoredCodeword {
                codeword: BitVec::zeros(7),
                noise_log_probability: -0.1,
                noise_weight: 0,
            }],
            query_count: 100,
            cumulative_log_probability: -0.01,
            log_parity_cap: 0.0,
            even_code: false,
        };
        let (log_apps, log_p_not) = compute_block_apps(&result, 7, 4);
        assert_eq!(log_apps.len(), 1);
        assert!(log_apps[0] > log_p_not);
        assert!(log_p_not < -1.0);
    }

    #[test]
    fn test_compute_block_apps_probabilities_sum_to_one() {
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
            log_parity_cap: 0.0,
            even_code: false,
        };
        let (log_apps, log_p_not) = compute_block_apps(&result, 7, 4);

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

    #[test]
    fn test_per_bit_app_all_zero_codeword_positive_llrs() {
        let sogrand = make_sogrand(4);
        let input_llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

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

    #[test]
    fn test_siso_list_size_1_works() {
        let sogrand = make_sogrand(1);
        let input_llrs: Vec<Llr> = vec![Llr::new(3.0); 7];
        let result = sogrand.decode_siso(&input_llrs);

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

    #[test]
    fn test_app_llr_sign_consistency_with_channel() {
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

    #[test]
    fn test_no_nan_or_inf_in_output() {
        let sogrand = make_sogrand(4);
        let test_cases: Vec<Vec<Llr>> = vec![
            vec![Llr::new(0.01); 7],
            vec![Llr::new(10.0); 7],
            vec![Llr::new(-10.0); 7],
            vec![
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

        for msg_val in 0u8..16 {
            let mut msg = BitVec::with_capacity(4);
            for bit in 0..4 {
                msg.push_bit((msg_val >> bit) & 1 == 1);
            }
            let codeword = code.encode(&msg);

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

    mod prop_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
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
    use super::*;
    use crate::grand::orbgrand::{OneLineIntercept, OrbGrand, OrbGrandConfig};
    use crate::linear::LinearBlockCode;
    use crate::traits::BlockEncoder;
    use gf2_core::BitVec;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use rand_distr;

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
        let sigma = 0.7;
        let mut empirical_misses = 0;
        let mut total_predicted_bler = 0.0;

        for _ in 0..num_frames {
            let mut msg = BitVec::zeros(k);
            for i in 0..k {
                if rng.gen_bool(0.5) {
                    msg.set(i, true);
                }
            }
            let codeword = code.encode(&msg);

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

        // The predicted BLER can be very close to 0 when the list covers
        // most of the codebook probability.
        if avg_predicted_bler < 0.001 && empirical_bler < 0.05 {
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

    #[test]
    fn test_sogrand_near_64_bit_boundary() {
        use crate::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
        use crate::traits::block::ParityCheckMatrixAccess;
        use crate::transform::Extended;
        use gf2_core::field::extension::BinaryPrimeExt;
        use gf2_core::gf2m::Gf2mField;

        let extension = BinaryPrimeExt::new(Gf2mField::new(6, 0b1000011).with_tables())
            .expect("a valid binary BCH extension");
        let base = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(3)
                .expect("a positive BCH designed distance"),
        })
        .expect("a valid binary BCH construction");
        let ebch = Extended::new(base).expect("an extended BCH code fits in memory");
        let h = ebch
            .parity_check_matrix()
            .expect("extended BCH parity matrix");

        let config = OrbGrandConfig {
            max_queries: 50_000,
            list_size: 2,
            even_code: true,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let sogrand = SoGrand::new(OrbGrand::new(h, config));

        let llrs: Vec<Llr> = vec![Llr::new(5.0); 64];
        let result = sogrand.decode_siso(&llrs);

        assert_eq!(result.app_llrs.len(), 64);
        assert_eq!(result.extrinsic_llrs.len(), 64);
        for i in 0..64 {
            assert!(
                result.app_llrs[i].value() > 0.0,
                "APP LLR at bit {} should be positive for all-zero codeword",
                i
            );
        }
    }

    #[test]
    fn test_log_codebook_ratio_approximation_accuracy() {
        for (n, k) in &[(7, 4), (15, 11), (16, 11), (31, 26), (32, 26)] {
            let result = log_codebook_ratio(*n, *k);
            assert!(
                result < 0.0,
                "log_codebook_ratio({n},{k}) = {result} should be negative"
            );
            let approx = (*k as f64 - *n as f64) * 2.0_f64.ln();
            let error = (result - approx).abs();
            assert!(
                error < 1.0,
                "log_codebook_ratio({n},{k}) = {result:.4}, approx = {approx:.4}, error = {error:.4}"
            );
        }
    }
}
