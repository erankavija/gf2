//! Ordered Reliability Bits GRAND (ORBGRAND, `@/citation/Duffy2022`): a
//! soft-input list decoder for any linear block code that tests noise patterns
//! in ascending combined weight `IC·w + lw`.

use crate::llr::Llr;
use crate::traits::{DecoderResult, SoftDecoder};
use gf2_core::sparse::SpBitMatrix;
use gf2_core::BitMatrix;
use gf2_core::BitVec;

/// Configuration for the ORBGRAND decoder.
#[derive(Debug, Clone)]
pub struct OrbGrandConfig {
    /// Maximum number of noise patterns to test before giving up.
    pub max_queries: usize,

    /// Number of codewords to collect in list decoding mode.
    /// Set to 1 for standard (non-list) decoding.
    pub list_size: usize,

    /// Whether the code is even (all codewords have even Hamming weight).
    /// When `true`, noise patterns whose weight parity does not match
    /// the received word's parity are skipped, halving the search space.
    pub even_code: bool,

    /// Whether the code is systematic (information bits are the first `k`
    /// bits of the codeword). Required for the [`SoftDecoder`] trait
    /// implementation, which extracts message bits as `codeword[0..k]`.
    ///
    /// When `false`, the [`SoftDecoder`] trait implementation will panic.
    /// Use [`OrbGrand::decode`] directly for non-systematic codes and
    /// extract message bits according to the code's structure.
    pub systematic: bool,

    /// Early-stop criterion on the running list-BLER estimate
    /// (`@/citation/Yuan2025`).
    ///
    /// When `Some(t)`, the search terminates as soon as the list has at
    /// least one codeword AND `P(C \ L) < t`, OR the list has `list_size`
    /// codewords (whichever fires first), in addition to the `max_queries`
    /// backstop.
    ///
    /// When `None` (default), the search stops at `max_queries`, or once
    /// `list_size` codewords are found and the cumulative probability
    /// reaches its cap.
    pub list_bler_stop_threshold: Option<f64>,

    /// 1-line ORBGRAND intercept (`IC` in `@/citation/Duffy2022`).
    ///
    /// Controls the combined-weight enumeration order
    /// `wt = IC·w + lw` where `w` is the Hamming weight of a test
    /// error pattern and `lw = sum of 1-based |LLR|-ranks of the
    /// flipped bits`:
    ///
    /// - [`OneLineIntercept::Basic`] (`IC = 0`) reduces to basic
    ///   ORBGRAND — pure logistic-weight ordering, Hamming weights
    ///   freely interleaved.
    /// - [`OneLineIntercept::Fixed(k)`](OneLineIntercept::Fixed)
    ///   pins a user-chosen intercept.
    /// - [`OneLineIntercept::Auto`] (default) recomputes `IC` from
    ///   the sorted `|LLR|` distribution on every decode, using the
    ///   slope heuristic of that work:
    ///   `β = (|L|_{(n/2)} − |L|_{(1)}) / (n/2 − 1)`,
    ///   `IC = max(round(|L|_{(1)} / β − 1), 0)`.
    pub one_line_intercept: OneLineIntercept,
}

/// Intercept selection for 1-line ORBGRAND (see
/// [`OrbGrandConfig::one_line_intercept`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneLineIntercept {
    /// Recompute `IC` from the sorted `|LLR|` distribution per decode.
    Auto,
    /// Basic ORBGRAND: `IC = 0`. Patterns are enumerated by pure
    /// ascending logistic weight.
    Basic,
    /// Fixed intercept, used verbatim for every decode.
    Fixed(u32),
}

impl Default for OrbGrandConfig {
    fn default() -> Self {
        Self {
            max_queries: 1_000_000,
            list_size: 1,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        }
    }
}

/// Compute the 1-line ORBGRAND intercept from a sorted-ascending slice of
/// `|LLR|` magnitudes, using the slope heuristic of `@/citation/Duffy2022`.
///
/// `β = (|L|_{(n/2)} − |L|_{(1)}) / (n/2 − 1)`,
/// `IC = max(round(|L|_{(1)} / β − 1), 0)`.
///
/// Returns 0 for edge cases (n < 4, degenerate slope).
pub(crate) fn auto_one_line_intercept(absl_sorted: &[f64]) -> u32 {
    let n = absl_sorted.len();
    if n < 4 {
        return 0;
    }
    let mid = n / 2;
    let denom = (mid as f64) - 1.0;
    if denom <= 0.0 {
        return 0;
    }
    let slope = (absl_sorted[mid - 1] - absl_sorted[0]) / denom;
    if slope <= 0.0 || !slope.is_finite() {
        return 0;
    }
    let ic_f = (absl_sorted[0] / slope - 1.0).round();
    if !ic_f.is_finite() || ic_f <= 0.0 {
        0
    } else {
        ic_f.min(u32::MAX as f64) as u32
    }
}

/// A codeword found during ORBGRAND decoding, annotated with its noise log-probability.
#[derive(Debug, Clone)]
pub struct ScoredCodeword {
    /// The decoded codeword (length n).
    pub codeword: BitVec,

    /// Log-probability of the noise pattern that produced this codeword:
    /// `ln p(z | r) = -sum_i ln(1 + exp(|LLR_i|))` for flipped bits
    /// plus `ln(1 - 1/(1+exp(|LLR_i|)))` for unflipped bits.
    /// Higher (less negative) values indicate more likely noise patterns.
    pub noise_log_probability: f64,

    /// Hamming weight of the noise pattern.
    pub noise_weight: usize,
}

impl ScoredCodeword {
    /// Returns `p(z | r) = exp(noise_log_probability)`.
    pub fn noise_probability(&self) -> f64 {
        self.noise_log_probability.exp()
    }
}

/// Result of an ORBGRAND decoding operation.
#[derive(Debug, Clone)]
pub struct OrbGrandResult {
    /// Hard-decision vector `y` derived from input LLRs (length n).
    /// Bit `i` is 1 when `LLR_i < 0`, and 0 otherwise.
    pub hard_decision: BitVec,

    /// List of codewords found, ordered by decreasing noise log-probability
    /// (most likely noise pattern first).
    pub codewords: Vec<ScoredCodeword>,

    /// Total number of noise patterns tested (queries).
    pub query_count: usize,

    /// Log of the cumulative probability of all tested noise patterns
    /// that are eligible under the parity constraint (all patterns for
    /// non-even codes, only patterns matching `hard_parity` for even codes).
    /// This is `ln(sum_z p(z|r))` summed over the eligible tested patterns.
    pub cumulative_log_probability: f64,

    /// Log of the total probability mass the noise can physically realise.
    ///
    /// For non-even codes this is `0.0` (= `log(1)`): any pattern is a
    /// possible noise realisation. For even codes, the noise parity is
    /// constrained to match `hard_parity`, so the reachable mass is
    /// `log P(parity(Z) = hard_parity | |L|)` (see
    /// `log_prob_parity`). The untested mass after `Q` queries is
    /// therefore `exp(log_parity_cap) − exp(cumulative_log_probability)`
    /// rather than `1 − exp(cumulative_log_probability)`.
    pub log_parity_cap: f64,

    /// Whether the decoder treated this as an even code (matches
    /// `OrbGrandConfig::even_code`). Downstream soft-output computations
    /// use this flag to apply the even-code codebook-ratio correction.
    pub even_code: bool,
}

impl OrbGrandResult {
    /// Returns `exp(cumulative_log_probability)`.
    pub fn cumulative_probability(&self) -> f64 {
        self.cumulative_log_probability.exp()
    }

    /// Returns `true` if at least one valid codeword was found.
    pub fn success(&self) -> bool {
        !self.codewords.is_empty()
    }

    /// Returns the most likely codeword, if any.
    pub fn best_codeword(&self) -> Option<&ScoredCodeword> {
        self.codewords.first()
    }
}

/// Ordered Reliability Bits GRAND (ORBGRAND) decoder for the linear block
/// code of a parity-check matrix.
pub struct OrbGrand {
    h_sparse: SpBitMatrix,

    n: usize,

    /// Rows of H.
    n_minus_k: usize,

    config: OrbGrandConfig,
}

impl OrbGrand {
    /// Creates a decoder from the `(n − k) × n` parity-check matrix `h`.
    ///
    /// # Panics
    ///
    /// Panics if `config.list_size` is zero.
    pub fn new(h: BitMatrix, config: OrbGrandConfig) -> Self {
        assert!(config.list_size > 0, "list_size must be at least 1");
        let n = h.cols();
        let n_minus_k = h.rows();
        let h_sparse = SpBitMatrix::from_dense(&h);
        Self {
            h_sparse,
            n,
            n_minus_k,
            config,
        }
    }

    /// Returns the codeword length.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message length (k = n - number of parity checks).
    pub fn k(&self) -> usize {
        self.n - self.n_minus_k
    }

    /// Decodes one word of `n` LLRs, where a positive LLR favours bit 0.
    ///
    /// The result lists every codeword found before a stop rule fired, sorted
    /// by decreasing noise log-probability; the list can exceed `list_size`.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n`.
    /// Panics if any LLR has a NaN magnitude.
    ///
    /// # Complexity
    ///
    /// Setup costs `n` sparse matrix–vector products.  A query of Hamming
    /// weight `w` costs O(w × (n − k) / 64) word operations, plus the pattern
    /// enumeration.
    pub fn decode(&self, llrs: &[Llr]) -> OrbGrandResult {
        assert_eq!(
            llrs.len(),
            self.n,
            "LLR vector length {} must equal code length {}",
            llrs.len(),
            self.n
        );

        let mut hard = BitVec::zeros(self.n);
        for (i, &llr) in llrs.iter().enumerate() {
            if llr.hard_decision() {
                hard.set(i, true);
            }
        }

        // pi[j] = the j-th least reliable bit position (0-indexed)
        let pi = Llr::reliability_permutation(llrs).ascending().to_vec();

        let base_syndrome = self.h_sparse.matvec(&hard);

        // syndrome_cols[j] = H * e_j (column j of H)
        let syndrome_cols: Vec<BitVec> = (0..self.n)
            .map(|j| {
                let mut ej = BitVec::zeros(self.n);
                ej.set(j, true);
                self.h_sparse.matvec(&ej)
            })
            .collect();

        // log p(flip bit i | LLR_i) = -ln(1 + exp(|LLR_i|))
        // log p(no flip bit i | LLR_i) = -ln(1 + exp(-|LLR_i|))
        let flip_log_probs: Vec<f64> = llrs
            .iter()
            .map(|llr| {
                let abs_llr = llr.magnitude() as f64;
                -ln_1_plus_exp(abs_llr)
            })
            .collect();

        let no_flip_log_probs: Vec<f64> = llrs
            .iter()
            .map(|llr| {
                let abs_llr = llr.magnitude() as f64;
                -ln_1_plus_exp(-abs_llr)
            })
            .collect();

        let base_log_prob: f64 = no_flip_log_probs.iter().sum();

        let hard_parity = hard.parity();

        // `P_notGuess` initialisation (`@/citation/Duffy2022`,
        // `@/citation/Yuan2025` § V). For even codes the noise parity is
        // constrained to match `hard_parity`, so the reachable pattern mass
        // is `P(parity(Z) = hard_parity | |L|)`, the ceiling of
        // `cumulative_log_prob`.
        let log_parity_cap = if self.config.even_code {
            let absl: Vec<f64> = llrs.iter().map(|l| l.magnitude() as f64).collect();
            log_prob_parity(&absl, hard_parity)
        } else {
            0.0 // log(1): all patterns are reachable
        };

        let mut codewords = Vec::new();
        let mut query_count: usize = 0;
        let mut cumulative_log_prob = f64::NEG_INFINITY;

        let ic = match self.config.one_line_intercept {
            OneLineIntercept::Basic => 0u32,
            OneLineIntercept::Fixed(k) => k,
            OneLineIntercept::Auto => {
                let sorted_abs: Vec<f64> =
                    pi.iter().map(|&idx| llrs[idx].magnitude() as f64).collect();
                auto_one_line_intercept(&sorted_abs)
            }
        };

        let pattern_iter = LogisticWeightPatternIter::with_ic(self.n, ic);

        // The list L and the cumulative S_Q reflect the same set of Q tested
        // patterns (`@/citation/Yuan2025`), keeping P(C\L) consistent, so the
        // list grows beyond list_size until a stop rule fires.
        let mut has_min_list = false;

        // The codebook-ratio absorbs the even-code correction
        // (`@/citation/Yuan2025` eq. (17) uses `2^-(s-1)` for even codes
        // rather than `2^-s`): only half the `2^n` binary words carry the
        // matching parity, and all `2^k` codewords of an even code do.
        let log_codebook_ratio =
            super::sogrand::log_codebook_ratio_for_code(self.n, self.k(), self.config.even_code);
        let mut log_sum_list = f64::NEG_INFINITY;

        for pattern in pattern_iter {
            if query_count >= self.config.max_queries {
                break;
            }
            if has_min_list && cumulative_log_prob > log_parity_cap - 1e-6 {
                break;
            }
            if let Some(threshold) = self.config.list_bler_stop_threshold {
                if codewords.len() >= self.config.list_size {
                    break;
                }
                if !codewords.is_empty() {
                    let log_not_tested_mass =
                        super::sogrand::log_cap_minus_exp(cumulative_log_prob, log_parity_cap);
                    let log_not_found = log_not_tested_mass + log_codebook_ratio;
                    let log_denom = log_sum_exp(log_sum_list, log_not_found);
                    let log_p_not_in_list = log_not_found - log_denom;
                    if log_p_not_in_list.exp() < threshold {
                        break;
                    }
                }
            }

            let bit_positions: Vec<usize> = pattern.iter().map(|&idx| pi[idx]).collect();
            let noise_weight = bit_positions.len();

            // Parity-mismatched patterns lie outside the mass capped by
            // `log_parity_cap`: they are neither accumulated nor counted as
            // queries.
            if self.config.even_code {
                let noise_parity = noise_weight % 2 == 1;
                if hard_parity ^ noise_parity {
                    continue;
                }
            }

            let noise_log_prob = compute_noise_log_prob(
                &bit_positions,
                &flip_log_probs,
                &no_flip_log_probs,
                base_log_prob,
            );

            cumulative_log_prob = log_sum_exp(cumulative_log_prob, noise_log_prob);

            query_count += 1;

            // H*(y XOR z) = H*y XOR H*z = base_syndrome XOR (XOR of H columns for flipped bits)
            let mut syndrome = base_syndrome.clone();
            for &pos in &bit_positions {
                syndrome.bit_xor_into(&syndrome_cols[pos]);
            }

            let is_zero = syndrome.count_ones() == 0;

            if is_zero {
                let mut codeword = hard.clone();
                for &pos in &bit_positions {
                    let current = codeword.get(pos);
                    codeword.set(pos, !current);
                }

                codewords.push(ScoredCodeword {
                    codeword,
                    noise_log_probability: noise_log_prob,
                    noise_weight,
                });
                log_sum_list = log_sum_exp(log_sum_list, noise_log_prob);

                if codewords.len() >= self.config.list_size {
                    has_min_list = true;
                }
            }
        }

        codewords.sort_by(|a, b| {
            b.noise_log_probability
                .partial_cmp(&a.noise_log_probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        OrbGrandResult {
            hard_decision: hard,
            codewords,
            query_count,
            cumulative_log_probability: cumulative_log_prob,
            log_parity_cap,
            even_code: self.config.even_code,
        }
    }
}

impl SoftDecoder for OrbGrand {
    fn k(&self) -> usize {
        self.k()
    }

    fn n(&self) -> usize {
        self.n()
    }

    /// Returns the first `k` bits of the best codeword, or the hard decisions
    /// of the first `k` LLRs when the search found no codeword.
    ///
    /// # Panics
    ///
    /// Panics if the decoder was configured with `systematic: false`, if
    /// `llrs.len() != n`, or if any LLR has a NaN magnitude.
    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        assert!(
            self.config.systematic,
            "SoftDecoder::decode_soft requires systematic=true in OrbGrandConfig. \
             Use OrbGrand::decode() directly for non-systematic codes."
        );
        let result = self.decode(llrs);
        if let Some(best) = result.best_codeword() {
            let k = self.k();
            let mut msg = BitVec::with_capacity(k);
            for i in 0..k {
                msg.push_bit(best.codeword.get(i));
            }
            msg
        } else {
            let mut msg = BitVec::with_capacity(self.k());
            for llr in llrs.iter().take(self.k()) {
                msg.push_bit(llr.hard_decision());
            }
            msg
        }
    }

    /// As [`Self::decode_soft`], with the query count in `queries`; a search
    /// that found no codeword is reported as a failure.
    ///
    /// # Panics
    ///
    /// Panics under the conditions of [`Self::decode_soft`].
    fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
        assert!(
            self.config.systematic,
            "SoftDecoder::decode_soft_with_result requires systematic=true in OrbGrandConfig. \
             Use OrbGrand::decode() directly for non-systematic codes."
        );
        let result = self.decode(llrs);
        if result.success() {
            let decoded = {
                let best = result.best_codeword().unwrap();
                let k = self.k();
                let mut msg = BitVec::with_capacity(k);
                for i in 0..k {
                    msg.push_bit(best.codeword.get(i));
                }
                msg
            };
            let mut dr = DecoderResult::new(decoded, 1, true, true);
            dr.queries = Some(result.query_count);
            dr
        } else {
            let mut msg = BitVec::with_capacity(self.k());
            for llr in llrs.iter().take(self.k()) {
                msg.push_bit(llr.hard_decision());
            }
            let mut dr = DecoderResult::failure(msg, 1);
            dr.queries = Some(result.query_count);
            dr
        }
    }
}

/// Compute `ln(1 + exp(x))` numerically stably.
pub fn ln_1_plus_exp(x: f64) -> f64 {
    if x > 30.0 {
        x // For large x, ln(1+exp(x)) ≈ x
    } else if x < -30.0 {
        0.0 // For very negative x, ln(1+exp(x)) ≈ 0
    } else {
        (1.0_f64 + x.exp()).ln()
    }
}

/// Compute `ln(exp(a) + exp(b))` numerically stably.
pub fn log_sum_exp(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    let max = a.max(b);
    max + ((a - max).exp() + (b - max).exp()).ln()
}

/// Compute `log P(parity(noise) = target | |L|)` for independent bit
/// flips with flip probability `p_i = 1 / (1 + exp(|L_i|))`
/// (`@/citation/Duffy2022` § III.C).
///
/// Using the characteristic-function identity `E[(-1)^{sum X_i}] =
/// prod (1 - 2 p_i) = prod tanh(|L_i|/2)`:
///
/// - `P(parity = 0)` (even target) `= 0.5 * (1 + prod tanh(|L_i|/2))`
/// - `P(parity = 1)` (odd target)  `= 0.5 * (1 − prod tanh(|L_i|/2))`
///
/// All terms live in the log domain:
///
/// ```text
/// log T = sum log tanh(|L_i|/2)  where
/// log tanh(x/2) = log1p(-exp(-x)) − log1p(exp(-x))   for x > 0.
/// ```
///
/// `target_is_odd` selects `log P(odd parity)`.  The result is `-ln 2` when
/// any `|L_i| = 0` (uniform parity).
pub fn log_prob_parity(abs_llrs: &[f64], target_is_odd: bool) -> f64 {
    let ln2 = std::f64::consts::LN_2;
    // If any bit has |L| = 0 the product is zero (uniform parity).
    let mut log_t = 0.0;
    for &absl in abs_llrs {
        let abs_l = absl.abs();
        if abs_l == 0.0 {
            return -ln2;
        }
        let e = (-abs_l).exp();
        log_t += (-e).ln_1p() - e.ln_1p();
    }
    if log_t == f64::NEG_INFINITY {
        return -ln2;
    }
    if target_is_odd {
        // log(0.5 * (1 - T)) = -ln 2 + log1p(-exp(log_t))
        -ln2 + super::sogrand::log1mexp(log_t)
    } else {
        // log(0.5 * (1 + T)) = -ln 2 + log1p(exp(log_t))
        -ln2 + log_t.exp().ln_1p()
    }
}

/// Compute the log-probability of a noise pattern given the bit positions flipped.
///
/// `log p(z|r) = sum_{i in flipped} log_flip[i] + sum_{i not flipped} log_noflip[i]`
///
/// This equals `base_log_prob + sum_{i in flipped} (log_flip[i] - log_noflip[i])`.
fn compute_noise_log_prob(
    flipped_positions: &[usize],
    flip_log_probs: &[f64],
    no_flip_log_probs: &[f64],
    base_log_prob: f64,
) -> f64 {
    let mut log_prob = base_log_prob;
    for &pos in flipped_positions {
        log_prob += flip_log_probs[pos] - no_flip_log_probs[pos];
    }
    log_prob
}

/// Iterator that generates noise patterns in ascending **combined weight**
/// `wt = IC·w + lw`, the 1-line ORBGRAND enumeration of
/// `@/citation/Duffy2022`.
///
/// For each `wt = 1, 2, …`, all valid `(w, partition)` pairs are yielded,
/// where:
///
/// - `w` is the Hamming weight (number of flipped bits);
/// - `lw = wt − IC·w` is the pure logistic weight (sum of 1-based
///   reliability ranks of the flipped bits), which must satisfy
///   `w(w+1)/2 ≤ lw ≤ w·n − w(w−1)/2`;
/// - the partition enumerates all size-`w` subsets of `{1, …, n}`
///   summing to `lw`, in ascending lexicographic order of 0-based
///   indices.
///
/// With `ic = 0` the enumeration reduces to basic ORBGRAND (ascending
/// logistic weight, Hamming weights freely interleaved). With `ic > 0`
/// the intercept penalises higher Hamming weights so that low-weight
/// patterns get priority, which is the 1-line variant used in
/// `@/citation/Yuan2025` § V.
///
/// The empty pattern `{}` is yielded first (wt = 0), representing the
/// no-flip hypothesis, and every other pattern of Hamming weight in
/// `{1, …, n}` follows exactly once.
struct LogisticWeightPatternIter {
    n: usize,
    /// 1-line ORBGRAND intercept `IC`. 0 = basic ORBGRAND.
    ic: u32,
    /// Current `wt = IC·w + lw` being enumerated.
    current_wt: usize,
    /// Patterns of the current `wt`.
    buffer: Vec<Vec<usize>>,
    buffer_idx: usize,
    emitted_empty: bool,
}

impl LogisticWeightPatternIter {
    #[cfg(test)]
    fn new(n: usize) -> Self {
        Self::with_ic(n, 0)
    }

    fn with_ic(n: usize, ic: u32) -> Self {
        Self {
            n,
            ic,
            current_wt: 0,
            buffer: Vec::new(),
            buffer_idx: 0,
            emitted_empty: false,
        }
    }

    /// Minimum logistic weight for a pattern of Hamming weight `w`: the `w`
    /// smallest 1-based indices, 1 + 2 + ... + w.
    fn min_lw_for_weight(w: usize) -> usize {
        w * (w + 1) / 2
    }

    /// Maximum logistic weight for a pattern of Hamming weight `w` over `n`
    /// positions: the `w` largest 1-based indices, (n-w+1) + ... + n.
    fn max_lw_for_weight(n: usize, w: usize) -> usize {
        if w == 0 {
            return 0;
        }
        w * n - w * (w - 1) / 2
    }

    /// Generate all patterns of exactly Hamming weight `w` (as sorted 0-based
    /// index vectors) with the given logistic weight `lw`.
    ///
    /// Logistic weight uses 1-based indices, so a pattern flipping 0-based
    /// positions `{a, b, c}` has LW = `(a+1) + (b+1) + (c+1)`.
    fn generate_patterns_for_weight_and_lw(n: usize, w: usize, lw: usize) -> Vec<Vec<usize>> {
        let mut result = Vec::new();
        if w == 0 {
            if lw == 0 {
                result.push(vec![]);
            }
            return result;
        }
        Self::enumerate_subsets_exact(n, w, lw, 1, &mut vec![], &mut result);
        result
    }

    /// Recursively enumerate subsets of exactly `remaining_count` elements
    /// from `{min_val..=n}` (1-based) that sum to `remaining_sum`.
    /// `current` accumulates the chosen 0-based indices.
    fn enumerate_subsets_exact(
        n: usize,
        remaining_count: usize,
        remaining_sum: usize,
        min_val: usize,
        current: &mut Vec<usize>,
        result: &mut Vec<Vec<usize>>,
    ) {
        if remaining_count == 0 {
            if remaining_sum == 0 {
                result.push(current.clone());
            }
            return;
        }
        if min_val > n {
            return;
        }
        // Pruning: minimum possible sum with `remaining_count` elements starting at `min_val`
        // is min_val + (min_val+1) + ... + (min_val + remaining_count - 1)
        let min_possible = remaining_count * min_val + remaining_count * (remaining_count - 1) / 2;
        if min_possible > remaining_sum {
            return;
        }
        // Maximum possible sum with `remaining_count` elements ending at `n`
        // is n + (n-1) + ... + (n - remaining_count + 1)
        let max_possible = remaining_count * n - remaining_count * (remaining_count - 1) / 2;
        if max_possible < remaining_sum {
            return;
        }

        let max_val = remaining_sum
            .saturating_sub(remaining_count * (remaining_count - 1) / 2)
            .min(n);
        for val in min_val..=max_val {
            let new_remaining = remaining_sum - val;
            let new_count = remaining_count - 1;
            if new_count > 0 {
                let min_next = new_count * (val + 1) + new_count * (new_count - 1) / 2;
                if min_next > new_remaining {
                    break;
                }
            } else if new_remaining != 0 {
                continue;
            }
            current.push(val - 1);
            Self::enumerate_subsets_exact(n, new_count, new_remaining, val + 1, current, result);
            current.pop();
        }
    }
}

impl Iterator for LogisticWeightPatternIter {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.emitted_empty {
            self.emitted_empty = true;
            return Some(Vec::new());
        }

        loop {
            if self.buffer_idx < self.buffer.len() {
                let pattern = self.buffer[self.buffer_idx].clone();
                self.buffer_idx += 1;
                return Some(pattern);
            }

            self.current_wt += 1;
            let wt = self.current_wt;

            // Upper bound on `wt`: the largest wt any length-n pattern can
            // produce (Hamming weight n, all ranks flipped).
            let wt_max = (self.ic as usize) * self.n + self.n * (self.n + 1) / 2;
            if wt > wt_max {
                return None;
            }

            self.buffer.clear();
            self.buffer_idx = 0;

            for w in 1..=self.n {
                let ic_contrib = (self.ic as usize) * w;
                if ic_contrib > wt {
                    break;
                }
                let lw = wt - ic_contrib;
                if lw < Self::min_lw_for_weight(w) {
                    // Higher w only shrinks the residual further.
                    break;
                }
                if lw > Self::max_lw_for_weight(self.n, w) {
                    continue;
                }
                let mut patterns = Self::generate_patterns_for_weight_and_lw(self.n, w, lw);
                self.buffer.append(&mut patterns);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::generic_ebch_16_11;
    use crate::traits::block::ParityCheckMatrixAccess;

    fn hamming_7_4_h() -> BitMatrix {
        gf2_core::bitmatrix![
            1, 1, 0, 1, 1, 0, 0;
            1, 0, 1, 1, 0, 1, 0;
            0, 1, 1, 1, 0, 0, 1
        ]
    }

    #[test]
    fn test_decode_finite_reliability_order_preserves_query_and_stopping_behavior() {
        // This one-check code accepts every word whose first bit is zero.
        // The hard decision violates that check, so the candidate is found
        // exactly when the first bit's reliability rank is queried.
        let h = gf2_core::bitmatrix![1, 0, 0, 0, 0];
        let llrs = [
            Llr::new(-0.4),
            Llr::new(2.0),
            Llr::new(-1.0),
            Llr::new(0.2),
            Llr::new(3.0),
        ];
        let order = Llr::reliability_permutation(&llrs);
        assert_eq!(order.ascending(), &[3, 0, 2, 1, 4]);

        let config = OrbGrandConfig {
            max_queries: 3,
            one_line_intercept: OneLineIntercept::Basic,
            ..OrbGrandConfig::default()
        };
        let result = OrbGrand::new(h, config).decode(&llrs);

        assert_eq!(result.hard_decision.count_ones(), 2);
        assert!(result.hard_decision.get(0));
        assert!(result.hard_decision.get(2));
        assert_eq!(result.query_count, 3);
        assert!(result.success());
        assert_eq!(result.best_codeword().unwrap().noise_weight, 1);
        assert!(!result.best_codeword().unwrap().codeword.get(0));
    }

    #[test]
    fn test_decode_tied_signed_zero_and_infinite_reliabilities() {
        let h = gf2_core::bitmatrix![1, 0, 0, 0, 0];
        let llrs = [
            Llr::new(-1.0),
            Llr::new(1.0),
            Llr::new(-0.0),
            Llr::new(0.0),
            Llr::infinity(),
        ];
        let order = Llr::reliability_permutation(&llrs);
        assert_eq!(order.ascending(), &[2, 3, 0, 1, 4]);

        let config = OrbGrandConfig {
            max_queries: 4,
            one_line_intercept: OneLineIntercept::Basic,
            ..OrbGrandConfig::default()
        };
        let result = OrbGrand::new(h, config).decode(&llrs);

        assert!(result.hard_decision.get(0));
        assert!(!result.hard_decision.get(1));
        assert!(!result.hard_decision.get(2));
        assert!(!result.hard_decision.get(3));
        assert!(!result.hard_decision.get(4));
        assert_eq!(result.query_count, 4);
        assert!(result.success());
        assert_eq!(result.best_codeword().unwrap().noise_weight, 1);
        assert!(!result.best_codeword().unwrap().codeword.get(0));
    }

    #[test]
    fn test_logistic_weight_iter_first_pattern_is_empty() {
        let mut iter = LogisticWeightPatternIter::new(4);
        let first = iter.next().unwrap();
        assert!(first.is_empty(), "First pattern should be empty (LW=0)");
    }

    #[test]
    fn test_logistic_weight_iter_weight_1_patterns() {
        // With IC=0, weight-2 patterns with small LW precede high-rank
        // weight-1 patterns.
        let mut iter = LogisticWeightPatternIter::new(4);
        let _ = iter.next();

        assert_eq!(iter.next().unwrap(), vec![0]); // wt=1, w=1
        assert_eq!(iter.next().unwrap(), vec![1]); // wt=2, w=1
                                                   // wt=3: w=1 → {2}, then w=2 → {0,1} (both have lw=3)
        assert_eq!(iter.next().unwrap(), vec![2]);
        assert_eq!(iter.next().unwrap(), vec![0, 1]);
        // wt=4: w=1 → {3}, then w=2 → {0,2}
        assert_eq!(iter.next().unwrap(), vec![3]);
        assert_eq!(iter.next().unwrap(), vec![0, 2]);
    }

    #[test]
    fn test_logistic_weight_iter_all_patterns_n3() {
        // For n=3 basic ORBGRAND order (by ascending wt = lw):
        //   wt=0: {}
        //   wt=1: {0}
        //   wt=2: {1}
        //   wt=3: {2}, {0,1}
        //   wt=4: {0,2}
        //   wt=5: {1,2}
        //   wt=6: {0,1,2}
        let iter = LogisticWeightPatternIter::new(3);
        let patterns: Vec<Vec<usize>> = iter.collect();

        assert_eq!(patterns.len(), 8);

        for i in 1..patterns.len() {
            let lw_prev: usize = patterns[i - 1].iter().map(|&x| x + 1).sum();
            let lw_curr: usize = patterns[i].iter().map(|&x| x + 1).sum();
            assert!(
                lw_prev <= lw_curr,
                "Logistic weight must be non-decreasing: {:?} (lw={}) vs {:?} (lw={})",
                patterns[i - 1],
                lw_prev,
                patterns[i],
                lw_curr
            );
        }
    }

    #[test]
    fn test_one_line_ic_reorders_by_weight_penalty() {
        // With IC=2 and n=4, wt = 2w + lw: {} (wt=0), {0} (wt=3),
        // {1} (wt=4), {2} (wt=5), {3} (wt=6), {0,1} (wt=7), {0,2} (wt=8), …
        let mut iter = LogisticWeightPatternIter::with_ic(4, 2);
        assert!(iter.next().unwrap().is_empty());
        assert_eq!(iter.next().unwrap(), vec![0]);
        assert_eq!(iter.next().unwrap(), vec![1]);
        assert_eq!(iter.next().unwrap(), vec![2]);
        assert_eq!(iter.next().unwrap(), vec![3]);
        assert_eq!(iter.next().unwrap(), vec![0, 1]);
    }

    #[test]
    fn test_logistic_weight_patterns_at_weight_and_lw() {
        let patterns_w1 = LogisticWeightPatternIter::generate_patterns_for_weight_and_lw(4, 1, 3);
        assert_eq!(patterns_w1.len(), 1);
        assert_eq!(patterns_w1[0], vec![2]);

        let patterns_w2 = LogisticWeightPatternIter::generate_patterns_for_weight_and_lw(4, 2, 3);
        assert_eq!(patterns_w2.len(), 1);
        assert_eq!(patterns_w2[0], vec![0, 1]);
    }

    #[test]
    fn test_logistic_weight_total_patterns() {
        for n in 0..=5 {
            let iter = LogisticWeightPatternIter::new(n);
            let count = iter.count();
            assert_eq!(
                count,
                1 << n,
                "Expected 2^{} = {} patterns for n={}, got {}",
                n,
                1 << n,
                n,
                count
            );
        }
    }

    #[test]
    fn test_decode_all_zero_codeword_high_confidence() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = decoder.decode(&llrs);

        assert!(result.success());
        let best = result.best_codeword().unwrap();
        assert_eq!(best.noise_weight, 0);
        for i in 0..7 {
            assert!(!best.codeword.get(i), "Bit {} should be 0", i);
        }
    }

    #[test]
    fn test_decode_known_codeword_no_errors() {
        use crate::linear::LinearBlockCode;
        use crate::traits::BlockEncoder;

        let code = LinearBlockCode::hamming(3);
        let h = code.parity_check().unwrap().clone();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let mut msg = BitVec::with_capacity(4);
        msg.push_bit(true);
        msg.push_bit(false);
        msg.push_bit(true);
        msg.push_bit(false);
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
        let result = decoder.decode(&llrs);

        assert!(result.success());
        let best = result.best_codeword().unwrap();
        assert_eq!(
            best.noise_weight, 0,
            "No errors, so noise weight should be 0"
        );
        assert!(result.query_count >= 1, "Should have at least one query");
    }

    #[test]
    fn test_decode_single_error_correction() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let llrs = vec![
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(-0.5), // bit 2: slightly negative → hard decision is 1 (error)
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(5.0),
            Llr::new(5.0),
        ];

        let result = decoder.decode(&llrs);
        assert!(result.success());
        let best = result.best_codeword().unwrap();

        for i in 0..7 {
            assert!(!best.codeword.get(i), "Bit {} should be 0", i);
        }
        assert_eq!(best.noise_weight, 1, "Single bit flip correction");
    }

    #[test]
    fn test_decode_query_count_tracked() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = decoder.decode(&llrs);

        assert!(result.query_count >= 1);
    }

    #[test]
    fn test_decode_max_queries_respected() {
        let h = hamming_7_4_h();
        let config = OrbGrandConfig {
            max_queries: 5,
            list_size: 1,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder = OrbGrand::new(h, config);

        let llrs = vec![
            Llr::new(-0.1),
            Llr::new(-0.1),
            Llr::new(-0.1),
            Llr::new(-0.1),
            Llr::new(-0.1),
            Llr::new(-0.1),
            Llr::new(-0.1),
        ];
        let result = decoder.decode(&llrs);
        assert!(result.query_count <= 5, "Should respect max_queries limit");
    }

    #[test]
    fn test_list_decode_returns_multiple_codewords() {
        let h = hamming_7_4_h();
        let config = OrbGrandConfig {
            max_queries: 100_000,
            list_size: 3,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder = OrbGrand::new(h, config);

        let llrs = vec![
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
        ];
        let result = decoder.decode(&llrs);

        // The list grows beyond list_size: max_queries covers all 128
        // patterns for n=7, which contain the 16 codewords of Hamming(7,4).
        assert!(
            result.codewords.len() >= 2,
            "Expected at least 2 codewords in list mode, got {}",
            result.codewords.len()
        );
    }

    #[test]
    fn test_list_decode_finds_all_hamming_codewords() {
        use crate::linear::LinearBlockCode;

        let code = LinearBlockCode::hamming(3);
        let h = code.parity_check().unwrap().clone();
        let config = OrbGrandConfig {
            max_queries: 1_000_000,
            list_size: 16, // All codewords for Hamming(7,4)
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder = OrbGrand::new(h, config);

        let llrs = vec![
            Llr::new(3.0),
            Llr::new(2.0),
            Llr::new(1.0),
            Llr::new(0.5),
            Llr::new(4.0),
            Llr::new(3.5),
            Llr::new(2.5),
        ];
        let result = decoder.decode(&llrs);

        assert!(result.success());
        let best = result.best_codeword().unwrap();
        assert_eq!(best.noise_weight, 0);

        assert_eq!(
            result.codewords.len(),
            16,
            "Hamming(7,4) has 2^4=16 codewords, found {}",
            result.codewords.len()
        );

        for cw in &result.codewords[1..] {
            assert!(
                best.noise_log_probability >= cw.noise_log_probability - 1e-10,
                "All-zero codeword should be most likely when all LLRs are positive"
            );
        }
    }

    #[test]
    fn test_even_code_reduces_queries() {
        // Extended Hamming(8,4) is an even code.
        let h_ext = gf2_core::bitmatrix![
            1, 1, 0, 1, 1, 0, 0, 0;
            1, 0, 1, 1, 0, 1, 0, 0;
            0, 1, 1, 1, 0, 0, 1, 0;
            1, 1, 1, 1, 1, 1, 1, 1
        ];

        // Use list mode with all 16 codewords so that both decoders
        // enumerate the full search space, making the query ratio
        // converge to approximately 0.5.
        let config_normal = OrbGrandConfig {
            max_queries: 1_000_000,
            list_size: 16,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder_normal = OrbGrand::new(h_ext.clone(), config_normal);

        let config_even = OrbGrandConfig {
            max_queries: 1_000_000,
            list_size: 16,
            even_code: true,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder_even = OrbGrand::new(h_ext, config_even);

        let llrs = vec![
            Llr::new(-0.3),
            Llr::new(0.4),
            Llr::new(-0.5),
            Llr::new(0.6),
            Llr::new(1.0),
            Llr::new(1.2),
            Llr::new(1.5),
            Llr::new(2.0),
        ];

        let result_normal = decoder_normal.decode(&llrs);
        let result_even = decoder_even.decode(&llrs);

        assert_eq!(result_normal.codewords.len(), 16);
        assert_eq!(result_even.codewords.len(), 16);

        assert!(
            result_even.query_count > 0 && result_normal.query_count > 0,
            "Both decoders should perform at least one query"
        );
        let ratio = result_even.query_count as f64 / result_normal.query_count as f64;
        assert!(
            (0.40..=0.60).contains(&ratio),
            "Even code optimization should approximately halve queries: \
             even={} normal={} ratio={:.3} (expected 0.40..0.60)",
            result_even.query_count,
            result_normal.query_count,
            ratio
        );
    }

    #[test]
    fn test_noise_log_probability_zero_pattern() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = decoder.decode(&llrs);

        assert!(result.success());
        let best = result.best_codeword().unwrap();

        // For zero noise pattern, log prob = sum of log(1 - 1/(1+exp(5)))
        // = sum of -ln(1 + exp(-5))
        // ≈ 7 * -0.00671 ≈ -0.047
        assert!(
            best.noise_log_probability < 0.0,
            "Log probability should be negative"
        );
        assert!(
            best.noise_log_probability > -1.0,
            "With high confidence, probability should be close to 0 (in log): got {}",
            best.noise_log_probability
        );
    }

    #[test]
    fn test_cumulative_probability_increases() {
        let h = hamming_7_4_h();
        let config = OrbGrandConfig {
            max_queries: 100,
            list_size: 1,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder = OrbGrand::new(h, config);

        let llrs = vec![
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
            Llr::new(0.5),
        ];
        let result = decoder.decode(&llrs);

        assert!(
            result.cumulative_log_probability > f64::NEG_INFINITY,
            "Cumulative probability should be > -inf after queries"
        );
    }

    #[test]
    fn test_soft_decoder_trait_decode() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let soft_decoder: &dyn SoftDecoder = &decoder;
        assert_eq!(soft_decoder.k(), 4);
        assert_eq!(soft_decoder.n(), 7);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let decoded = soft_decoder.decode_soft(&llrs);
        assert_eq!(decoded.len(), 4);
        for i in 0..4 {
            assert!(!decoded.get(i));
        }
    }

    #[test]
    fn test_soft_decoder_with_result() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = decoder.decode_soft_with_result(&llrs);

        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.len(), 4);
    }

    #[test]
    #[should_panic(expected = "LLR vector length")]
    fn test_decode_wrong_length_panics() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let llrs: Vec<Llr> = vec![Llr::new(1.0); 5];
        decoder.decode(&llrs);
    }

    #[test]
    #[should_panic(expected = "list_size must be at least 1")]
    fn test_zero_list_size_panics() {
        let h = hamming_7_4_h();
        let config = OrbGrandConfig {
            max_queries: 100,
            list_size: 0,
            even_code: false,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        OrbGrand::new(h, config);
    }

    #[test]
    fn test_decode_all_ones_codeword() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        // All-ones [1,1,1,1,1,1,1] is a valid Hamming(7,4) codeword
        // (sum of all rows of G)
        let llrs: Vec<Llr> = vec![Llr::new(-5.0); 7];
        let result = decoder.decode(&llrs);

        assert!(result.success());
        let best = result.best_codeword().unwrap();
        assert_eq!(best.noise_weight, 0);
        for i in 0..7 {
            assert!(best.codeword.get(i), "Bit {} should be 1", i);
        }
    }

    #[test]
    fn test_ml_decoding_picks_closest_codeword() {
        use crate::linear::LinearBlockCode;
        use crate::traits::BlockEncoder;

        let code = LinearBlockCode::hamming(3);
        let h = code.parity_check().unwrap().clone();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let mut msg = BitVec::with_capacity(4);
        msg.push_bit(true);
        msg.push_bit(true);
        msg.push_bit(false);
        msg.push_bit(true);
        let codeword = code.encode(&msg);

        let llrs: Vec<Llr> = (0..7)
            .map(|i| {
                if i == 0 {
                    if codeword.get(i) {
                        Llr::new(0.2)
                    } else {
                        Llr::new(-0.2)
                    }
                } else if codeword.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();

        let result = decoder.decode(&llrs);
        assert!(result.success());
        let best = result.best_codeword().unwrap();

        for i in 0..7 {
            assert_eq!(
                best.codeword.get(i),
                codeword.get(i),
                "Bit {} should be {}",
                i,
                codeword.get(i) as u8
            );
        }
    }

    #[test]
    fn test_ln_1_plus_exp_accuracy() {
        let val = ln_1_plus_exp(0.0);
        assert!((val - 2.0_f64.ln()).abs() < 1e-10);

        let val = ln_1_plus_exp(100.0);
        assert!((val - 100.0).abs() < 1e-10);

        let val = ln_1_plus_exp(-100.0);
        assert!(val.abs() < 1e-10);
    }

    #[test]
    fn test_log_sum_exp_accuracy() {
        let a = 2.0_f64.ln();
        let b = 3.0_f64.ln();
        let result = log_sum_exp(a, b);
        assert!((result - 5.0_f64.ln()).abs() < 1e-10);

        assert_eq!(log_sum_exp(f64::NEG_INFINITY, 1.0), 1.0);
        assert_eq!(log_sum_exp(1.0, f64::NEG_INFINITY), 1.0);
    }

    #[test]
    fn test_roundtrip_hamming_7_4_all_messages() {
        use crate::linear::LinearBlockCode;
        use crate::traits::BlockEncoder;

        let code = LinearBlockCode::hamming(3); // Hamming(7,4)
        let h = code.parity_check().unwrap().clone();

        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        for msg_val in 0u8..16 {
            let mut msg = BitVec::with_capacity(4);
            for bit in 0..4 {
                msg.push_bit((msg_val >> bit) & 1 == 1);
            }

            let codeword = code.encode(&msg);
            assert_eq!(codeword.len(), 7);

            let llrs: Vec<Llr> = (0..7)
                .map(|i| {
                    if codeword.get(i) {
                        Llr::new(-5.0)
                    } else {
                        Llr::new(5.0)
                    }
                })
                .collect();

            let result = decoder.decode(&llrs);
            assert!(result.success(), "Failed to decode message {:04b}", msg_val);

            let best = result.best_codeword().unwrap();
            assert_eq!(
                best.noise_weight, 0,
                "Should find exact codeword for message {:04b}",
                msg_val
            );

            for i in 0..7 {
                assert_eq!(
                    best.codeword.get(i),
                    codeword.get(i),
                    "Bit {} mismatch for message {:04b}",
                    i,
                    msg_val
                );
            }
        }
    }

    #[test]
    fn test_roundtrip_hamming_7_4_single_error_all_positions() {
        use crate::linear::LinearBlockCode;
        use crate::traits::BlockEncoder;

        let code = LinearBlockCode::hamming(3);
        let h = code.parity_check().unwrap().clone();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let mut msg = BitVec::with_capacity(4);
        msg.push_bit(true);
        msg.push_bit(false);
        msg.push_bit(true);
        msg.push_bit(true);
        let codeword = code.encode(&msg);

        for error_pos in 0..7 {
            let llrs: Vec<Llr> = (0..7)
                .map(|i| {
                    let bit = codeword.get(i);
                    if i == error_pos {
                        if bit {
                            Llr::new(0.3)
                        } else {
                            Llr::new(-0.3)
                        }
                    } else if bit {
                        Llr::new(-5.0)
                    } else {
                        Llr::new(5.0)
                    }
                })
                .collect();

            let result = decoder.decode(&llrs);
            assert!(
                result.success(),
                "Failed with error at position {}",
                error_pos
            );

            let best = result.best_codeword().unwrap();
            for i in 0..7 {
                assert_eq!(
                    best.codeword.get(i),
                    codeword.get(i),
                    "Bit {} mismatch with error at position {}",
                    i,
                    error_pos
                );
            }
        }
    }

    mod prop_tests {
        use super::*;
        use proptest::prelude::*;
        use std::collections::HashSet;

        proptest! {
            #[test]
            fn test_iterator_produces_all_patterns_exactly_once(n in 3usize..=5) {
                let iter = LogisticWeightPatternIter::new(n);
                let patterns: Vec<Vec<usize>> = iter.collect();

                let expected_count = 1usize << n;
                prop_assert_eq!(patterns.len(), expected_count,
                    "Expected {} patterns, got {}", expected_count, patterns.len());

                let mut seen = HashSet::new();
                for pattern in &patterns {
                    let key: Vec<usize> = pattern.clone();
                    prop_assert!(seen.insert(key),
                        "Duplicate pattern found: {:?}", pattern);
                }

                for mask in 0..(1u32 << n) {
                    let expected: Vec<usize> = (0..n).filter(|&i| mask & (1 << i) != 0).collect();
                    prop_assert!(seen.contains(&expected),
                        "Missing pattern: {:?}", expected);
                }
            }

            #[test]
            fn test_logistic_weight_ordering(n in 3usize..=5) {
                let iter = LogisticWeightPatternIter::new(n);
                let patterns: Vec<Vec<usize>> = iter.collect();

                let mut prev_lw = 0usize;
                for pattern in &patterns {
                    let lw: usize = pattern.iter().map(|&x| x + 1).sum();
                    prop_assert!(lw >= prev_lw,
                        "Logistic weight decreased: {} -> {} for pattern {:?}",
                        prev_lw, lw, pattern);
                    prev_lw = lw;
                }
            }
        }
    }

    #[test]
    fn test_decode_ebch_16_11_single_error() {
        use crate::traits::BlockEncoder;

        let ebch = generic_ebch_16_11();
        let h = ebch
            .parity_check_matrix()
            .expect("extended BCH parity matrix");

        let config = OrbGrandConfig {
            max_queries: 10_000,
            list_size: 1,
            even_code: true,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder = OrbGrand::new(h, config);

        let msg = BitVec::zeros(11);
        let codeword = ebch.encode(&msg);

        for error_pos in 0..16 {
            let mut received = codeword.clone();
            let bit = received.get(error_pos);
            received.set(error_pos, !bit);

            let llrs: Vec<Llr> = (0..16)
                .map(|i| {
                    if i == error_pos {
                        if received.get(i) {
                            Llr::new(-0.5)
                        } else {
                            Llr::new(0.5)
                        }
                    } else if received.get(i) {
                        Llr::new(-5.0)
                    } else {
                        Llr::new(5.0)
                    }
                })
                .collect();

            let result = decoder.decode(&llrs);
            assert!(
                result.success(),
                "eBCH(16,11) failed with error at position {}",
                error_pos
            );

            let best = result.best_codeword().unwrap();
            for i in 0..16 {
                assert_eq!(
                    best.codeword.get(i),
                    codeword.get(i),
                    "eBCH(16,11) bit {} mismatch with error at position {}",
                    i,
                    error_pos
                );
            }
        }
    }

    #[test]
    fn test_list_bler_stop_threshold_reduces_queries_at_high_snr() {
        use crate::traits::BlockEncoder;

        let ebch = generic_ebch_16_11();
        let h = ebch
            .parity_check_matrix()
            .expect("extended BCH parity matrix");

        let msg = BitVec::zeros(11);
        let codeword = ebch.encode(&msg);

        let llrs: Vec<Llr> = (0..16)
            .map(|i| {
                let is_error = i == 3;
                let mag = 6.0_f32;
                if is_error {
                    Llr::new(-mag)
                } else {
                    Llr::new(mag)
                }
            })
            .collect();

        let baseline_config = OrbGrandConfig {
            max_queries: 100_000,
            list_size: 4,
            even_code: true,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let baseline = OrbGrand::new(h.clone(), baseline_config).decode(&llrs);

        let aligned_config = OrbGrandConfig {
            max_queries: 100_000,
            list_size: 4,
            even_code: true,
            systematic: true,
            list_bler_stop_threshold: Some(1e-4),
            one_line_intercept: OneLineIntercept::Auto,
        };
        let aligned = OrbGrand::new(h, aligned_config).decode(&llrs);

        assert!(
            aligned.success(),
            "paper-aligned decode must still succeed at high SNR"
        );
        assert_eq!(
            aligned.best_codeword().unwrap().codeword,
            codeword,
            "aligned decoder must recover the transmitted codeword"
        );
        assert!(
            aligned.query_count * 3 < baseline.query_count,
            "threshold stopping should cut queries by ≥3x at high SNR: \
             baseline={}, aligned={}",
            baseline.query_count,
            aligned.query_count
        );
        assert!(
            aligned.query_count < 200,
            "aligned query count should be tiny at high SNR, got {}",
            aligned.query_count
        );
    }
}
