//! Ordered Reliability Bits GRAND (ORBGRAND) decoder.
//!
//! ORBGRAND is a soft-input universal decoder that works with any linear block code.
//! It generates noise patterns in order of decreasing likelihood, using the
//! logistic-weight ordering derived from channel soft information (LLRs).
//!
//! # Algorithm
//!
//! 1. Compute hard decisions from LLRs to form the received word `y`.
//! 2. Sort bit positions by reliability (`|LLR|`), producing a permutation `pi`.
//! 3. Generate noise patterns in logistic-weight order: weight-1 patterns on the
//!    least reliable bits first, then weight-2 combinations, etc.
//! 4. For each noise pattern `z`, check if `y XOR z` is a valid codeword via
//!    syndrome check: `H * (y XOR z) = 0`.
//! 5. Return the first valid codeword found (ML decoding for the code).
//!
//! # Logistic Weight
//!
//! The logistic weight of a noise pattern `z` is defined as the sum of the
//! reliability indices of the flipped bit positions. If the bits are sorted by
//! increasing reliability (least reliable = index 1), then the logistic weight
//! of a pattern flipping positions `{i_1, ..., i_w}` is `i_1 + ... + i_w`.
//! Patterns with smaller logistic weight are more likely.
//!
//! # List Decoding
//!
//! ORBGRAND supports list decoding: after finding the first valid codeword,
//! it can continue to find up to `L` codewords. Each codeword is annotated
//! with its noise pattern log-probability `ln p(z|r)`. Use
//! [`ScoredCodeword::noise_probability()`] for the linear-domain value `p(z|r)`.
//!
//! # Even Code Optimization
//!
//! If all codewords have even Hamming weight (an *even code*), noise patterns
//! whose weight parity does not match the received word's parity can be skipped.
//! This halves the query space.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
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
//! let config = OrbGrandConfig::default();
//! let decoder = OrbGrand::new(h, config);
//!
//! // Soft-input: high confidence in all bits (a valid all-zero codeword)
//! let llrs = vec![
//!     Llr::new(5.0), Llr::new(5.0), Llr::new(5.0), Llr::new(5.0),
//!     Llr::new(5.0), Llr::new(5.0), Llr::new(5.0),
//! ];
//! let result = decoder.decode(&llrs);
//! assert!(result.success());
//! ```

use crate::llr::Llr;
use crate::traits::{DecoderResult, SoftDecoder};
use gf2_core::sparse::SpBitMatrix;
use gf2_core::BitMatrix;
use gf2_core::BitVec;

/// Configuration for the ORBGRAND decoder.
///
/// # Examples
///
/// ```
/// use gf2_coding::grand::OrbGrandConfig;
///
/// // Default: list size 1, 1M queries, not even code
/// let config = OrbGrandConfig::default();
/// assert_eq!(config.list_size, 1);
/// assert_eq!(config.max_queries, 1_000_000);
/// assert!(!config.even_code);
///
/// // Custom configuration
/// let config = OrbGrandConfig {
///     max_queries: 10_000,
///     list_size: 5,
///     even_code: true,
///     systematic: true,
///     list_bler_stop_threshold: Some(1e-4),
/// };
/// ```
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

    /// Paper-aligned early-stop criterion on the running list-BLER estimate.
    ///
    /// When `Some(t)`, the search terminates as soon as the list has at
    /// least one codeword AND `P(C \ L) < t`, OR the list has `list_size`
    /// codewords (whichever fires first), in addition to the `max_queries`
    /// backstop. This matches the rule used in Yuan–Médard–Galligan–Duffy
    /// SO-GRAND: "lists are added to until L=4 OR the predicted list-BLER
    /// is below 1e-4" (Figs 1/3/8; 1e-5 for the GLDPC configuration).
    ///
    /// When `None` (default), the legacy stop rule is used: exhaust
    /// `max_queries` or reach cumulative probability ≈ 1 with
    /// `list_size` codewords.
    pub list_bler_stop_threshold: Option<f64>,

    /// 1-line ORBGRAND intercept (`IC` in Duffy–An–Médard 2022).
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
    ///   slope heuristic from the paper:
    ///   `β = (|L|_{(n/2)} − |L|_{(1)}) / (n/2 − 1)`,
    ///   `IC = max(round(|L|_{(1)} / β − 1), 0)`.
    pub one_line_intercept: OneLineIntercept,
}

/// Intercept selection for 1-line ORBGRAND (see
/// [`OrbGrandConfig::one_line_intercept`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneLineIntercept {
    /// Recompute `IC` from the sorted `|LLR|` distribution per decode
    /// (paper default).
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
/// `|LLR|` magnitudes, using the slope heuristic from Duffy–An–Médard 2022.
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
///
/// # Examples
///
/// ```
/// use gf2_coding::grand::ScoredCodeword;
/// use gf2_core::BitVec;
///
/// let cw = ScoredCodeword {
///     codeword: BitVec::zeros(7),
///     noise_log_probability: -1.5,
///     noise_weight: 1,
/// };
/// assert_eq!(cw.noise_weight, 1);
/// ```
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
    /// Returns the noise probability in the linear domain: `exp(noise_log_probability)`.
    ///
    /// This is `p(z | r)`, the probability of the noise pattern that produced
    /// this codeword given the received signal.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::ScoredCodeword;
    /// use gf2_core::BitVec;
    ///
    /// let cw = ScoredCodeword {
    ///     codeword: BitVec::zeros(7),
    ///     noise_log_probability: 0.0,
    ///     noise_weight: 0,
    /// };
    /// assert!((cw.noise_probability() - 1.0).abs() < 1e-10);
    /// ```
    pub fn noise_probability(&self) -> f64 {
        self.noise_log_probability.exp()
    }
}

/// Result of an ORBGRAND decoding operation.
///
/// Contains the hard-decision vector, the list of found codewords (ordered by
/// decreasing noise log-probability), the number of queries performed, and
/// cumulative log-probability information.
///
/// # Examples
///
/// ```
/// use gf2_coding::grand::{OrbGrand, OrbGrandConfig, OrbGrandResult};
/// use gf2_coding::llr::Llr;
/// use gf2_core::BitVec;
///
/// // See OrbGrand::decode for full usage examples.
/// let result = OrbGrandResult {
///     hard_decision: BitVec::zeros(7),
///     codewords: vec![],
///     query_count: 0,
///     cumulative_log_probability: f64::NEG_INFINITY,
/// };
/// assert!(!result.success());
/// ```
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

    /// Log of the cumulative probability of all tested noise patterns.
    /// This is `ln(sum_z p(z|r))` summed over all tested patterns `z`.
    /// Use [`cumulative_probability()`](Self::cumulative_probability) for the linear-domain value.
    pub cumulative_log_probability: f64,
}

impl OrbGrandResult {
    /// Returns the cumulative probability in the linear domain: `exp(cumulative_log_probability)`.
    ///
    /// This is `sum_z p(z|r)` over all tested noise patterns.
    pub fn cumulative_probability(&self) -> f64 {
        self.cumulative_log_probability.exp()
    }

    /// Returns `true` if at least one valid codeword was found.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::OrbGrandResult;
    /// use gf2_core::BitVec;
    ///
    /// let result = OrbGrandResult {
    ///     hard_decision: BitVec::zeros(7),
    ///     codewords: vec![],
    ///     query_count: 100,
    ///     cumulative_log_probability: f64::NEG_INFINITY,
    /// };
    /// assert!(!result.success());
    /// ```
    pub fn success(&self) -> bool {
        !self.codewords.is_empty()
    }

    /// Returns the most likely codeword, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::OrbGrandResult;
    /// use gf2_core::BitVec;
    ///
    /// let result = OrbGrandResult {
    ///     hard_decision: BitVec::zeros(7),
    ///     codewords: vec![],
    ///     query_count: 0,
    ///     cumulative_log_probability: f64::NEG_INFINITY,
    /// };
    /// assert!(result.best_codeword().is_none());
    /// ```
    pub fn best_codeword(&self) -> Option<&ScoredCodeword> {
        self.codewords.first()
    }
}

/// Ordered Reliability Bits GRAND (ORBGRAND) decoder.
///
/// A universal soft-input decoder for linear block codes that achieves
/// near-maximum-likelihood decoding by testing noise patterns in weight-tiered
/// logistic-weight order, using soft reliability information from LLRs.
///
/// # Arguments
///
/// The decoder is constructed with a parity-check matrix `H` and a
/// configuration struct controlling query limits and list size.
///
/// # Examples
///
/// ```
/// use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
/// use gf2_coding::llr::Llr;
/// use gf2_core::BitMatrix;
///
/// // Hamming(7,4) parity-check matrix
/// let h = gf2_core::bitmatrix![
///     1, 1, 0, 1, 1, 0, 0;
///     1, 0, 1, 1, 0, 1, 0;
///     0, 1, 1, 1, 0, 0, 1
/// ];
///
/// let decoder = OrbGrand::new(h, OrbGrandConfig::default());
/// assert_eq!(decoder.n(), 7);
/// ```
///
/// # Complexity
///
/// Worst-case query complexity is O(2^n) but in practice, for codes at
/// moderate SNR, convergence is much faster. The logistic-weight ordering
/// ensures the most likely patterns are tested first.
pub struct OrbGrand {
    /// Sparse parity-check matrix for efficient syndrome computation.
    h_sparse: SpBitMatrix,

    /// Number of codeword bits.
    n: usize,

    /// Number of check (redundancy) bits: rows of H.
    n_minus_k: usize,

    /// Decoder configuration.
    config: OrbGrandConfig,
}

impl OrbGrand {
    /// Creates a new ORBGRAND decoder.
    ///
    /// # Arguments
    ///
    /// * `h` - Parity-check matrix (r x n) where r = n - k.
    /// * `config` - Decoder configuration controlling query limits and list size.
    ///
    /// # Panics
    ///
    /// Panics if `config.list_size` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
    /// use gf2_core::BitMatrix;
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let decoder = OrbGrand::new(h, OrbGrandConfig::default());
    /// assert_eq!(decoder.n(), 7);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let decoder = OrbGrand::new(h, OrbGrandConfig::default());
    /// assert_eq!(decoder.n(), 7);
    /// ```
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message length (k = n - number of parity checks).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let decoder = OrbGrand::new(h, OrbGrandConfig::default());
    /// assert_eq!(decoder.k(), 4);
    /// ```
    pub fn k(&self) -> usize {
        self.n - self.n_minus_k
    }

    /// Decodes a received word given soft-input LLRs.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Log-likelihood ratios for each of the `n` codeword bit positions.
    ///   Positive LLR means bit 0 is more likely; negative means bit 1 is more likely.
    ///
    /// # Returns
    ///
    /// An [`OrbGrandResult`] containing the hard-decision vector, the list of
    /// found codewords (up to `list_size`) sorted by decreasing noise
    /// log-probability, the total query count, and cumulative log-probability.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
    /// use gf2_coding::llr::Llr;
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let decoder = OrbGrand::new(h, OrbGrandConfig::default());
    ///
    /// // All-zero codeword with high confidence
    /// let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
    /// let result = decoder.decode(&llrs);
    /// assert!(result.success());
    /// assert_eq!(result.codewords[0].noise_weight, 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(Q * n) where Q is the number of noise patterns tested and n is the
    /// code length. Each query requires a syndrome computation costing O(nnz(H)).
    pub fn decode(&self, llrs: &[Llr]) -> OrbGrandResult {
        assert_eq!(
            llrs.len(),
            self.n,
            "LLR vector length {} must equal code length {}",
            llrs.len(),
            self.n
        );

        // Step 1: Hard decisions
        let mut hard = BitVec::zeros(self.n);
        for (i, &llr) in llrs.iter().enumerate() {
            if llr.hard_decision() {
                hard.set(i, true);
            }
        }

        // Step 2: Sort bit positions by reliability (ascending |LLR|)
        // pi[j] = the j-th least reliable bit position (0-indexed)
        let mut pi: Vec<usize> = (0..self.n).collect();
        pi.sort_by(|&a, &b| {
            llrs[a]
                .magnitude()
                .partial_cmp(&llrs[b].magnitude())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Pre-compute syndrome of hard-decision word: s = H * y
        let base_syndrome = self.h_sparse.matvec(&hard);

        // Pre-compute syndrome columns: syndrome_col[j] = H * e_j (column j of H)
        let syndrome_cols: Vec<BitVec> = (0..self.n)
            .map(|j| {
                let mut ej = BitVec::zeros(self.n);
                ej.set(j, true);
                self.h_sparse.matvec(&ej)
            })
            .collect();

        // Pre-compute log-probabilities for bit flips
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

        // Base log-probability: all bits unflipped
        let base_log_prob: f64 = no_flip_log_probs.iter().sum();

        // Even code optimization: determine required parity of noise pattern
        let hard_parity = hard.parity(); // true if odd weight

        let mut codewords = Vec::new();
        let mut query_count: usize = 0;
        let mut cumulative_log_prob = f64::NEG_INFINITY;

        // Resolve the 1-line ORBGRAND intercept. `Auto` uses the slope
        // heuristic on the sorted `|LLR|` distribution; `Basic` forces
        // IC=0 (basic ORBGRAND); `Fixed(k)` pins a user-supplied value.
        let ic = match self.config.one_line_intercept {
            OneLineIntercept::Basic => 0u32,
            OneLineIntercept::Fixed(k) => k,
            OneLineIntercept::Auto => {
                let sorted_abs: Vec<f64> =
                    pi.iter().map(|&idx| llrs[idx].magnitude() as f64).collect();
                auto_one_line_intercept(&sorted_abs)
            }
        };

        // Generate noise patterns in ascending combined-weight order
        // (`wt = IC·w + lw`). With IC=0 this is basic ORBGRAND.
        let pattern_iter = LogisticWeightPatternIter::with_ic(self.n, ic);

        // Test all patterns up to max_queries. All found codewords are
        // collected (the list grows beyond list_size). Cumulative probability
        // is accumulated for all tested patterns. This matches the SO-GRAND
        // paper: both the list L and cumulative S_Q reflect the same set of
        // Q tested patterns, keeping P(C\L) consistent.
        //
        // list_size controls early termination for hard-decision-only callers:
        // when at least list_size codewords are found AND cumulative probability
        // is near 1.0, we can stop early. For SOGRAND callers, max_queries
        // is the primary budget control unless `list_bler_stop_threshold` is set.
        let mut has_min_list = false;

        // Paper-aligned stopping: when `list_bler_stop_threshold` is set,
        // track the running list-BLER incrementally so we can stop as soon
        // as `P(C \ L) < threshold` with a non-empty list, or the list has
        // filled to `list_size`.
        let log_codebook_ratio = super::sogrand::log_codebook_ratio(self.n, self.k());
        let mut log_sum_list = f64::NEG_INFINITY;

        for pattern in pattern_iter {
            if query_count >= self.config.max_queries {
                break;
            }
            // Early exit when we have enough codewords AND cumulative
            // probability is near 1.0 (no more useful patterns to test).
            if has_min_list && cumulative_log_prob > -1e-6 {
                break;
            }
            // Paper-aligned early exit: list has ≥ list_size codewords,
            // or predicted list-BLER has dropped below the configured
            // threshold with a non-empty list.
            if let Some(threshold) = self.config.list_bler_stop_threshold {
                if codewords.len() >= self.config.list_size {
                    break;
                }
                if !codewords.is_empty() {
                    let log_one_minus_cum = super::sogrand::log1mexp(cumulative_log_prob);
                    let log_not_found = log_one_minus_cum + log_codebook_ratio;
                    let log_denom = log_sum_exp(log_sum_list, log_not_found);
                    let log_p_not_in_list = log_not_found - log_denom;
                    if log_p_not_in_list.exp() < threshold {
                        break;
                    }
                }
            }

            // pattern is a sorted list of indices into pi (1-based logistic indices)
            // Convert to actual bit positions
            let bit_positions: Vec<usize> = pattern.iter().map(|&idx| pi[idx]).collect();
            let noise_weight = bit_positions.len();

            // Compute noise log-probability for EVERY pattern (needed for
            // cumulative probability accounting in SOGRAND soft output).
            let noise_log_prob = compute_noise_log_prob(
                &bit_positions,
                &flip_log_probs,
                &no_flip_log_probs,
                base_log_prob,
            );

            // Accumulate cumulative probability for ALL tested patterns.
            // Both the codeword list and cumulative probability must reflect
            // the same set of tested patterns for P(C\L) to be consistent.
            cumulative_log_prob = log_sum_exp(cumulative_log_prob, noise_log_prob);

            // Even code optimization: skip syndrome check if parity won't match.
            // Probability is already accumulated above.
            if self.config.even_code {
                let noise_parity = noise_weight % 2 == 1;
                if hard_parity ^ noise_parity {
                    continue;
                }
            }

            query_count += 1;

            // Compute syndrome of y XOR z incrementally:
            // H*(y XOR z) = H*y XOR H*z = base_syndrome XOR (XOR of H columns for flipped bits)
            let mut syndrome = base_syndrome.clone();
            for &pos in &bit_positions {
                syndrome.bit_xor_into(&syndrome_cols[pos]);
            }

            // Check if syndrome is zero (valid codeword)
            let is_zero = syndrome.count_ones() == 0;

            if is_zero {
                // Construct the codeword y XOR z
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

        // Sort codewords by noise_log_probability descending (most likely first)
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

    /// Decodes using soft information (LLRs) and returns message bits.
    ///
    /// Extracts the first `k` bits of the best codeword as the decoded message.
    ///
    /// # Panics
    ///
    /// Panics if the decoder was configured with `systematic: false`.
    /// Use [`OrbGrand::decode`] directly for non-systematic codes.
    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        assert!(
            self.config.systematic,
            "SoftDecoder::decode_soft requires systematic=true in OrbGrandConfig. \
             Use OrbGrand::decode() directly for non-systematic codes."
        );
        let result = self.decode(llrs);
        if let Some(best) = result.best_codeword() {
            // Extract first k bits (assumes systematic code)
            let k = self.k();
            let mut msg = BitVec::with_capacity(k);
            for i in 0..k {
                msg.push_bit(best.codeword.get(i));
            }
            msg
        } else {
            // Fall back to hard decision on first k bits
            let mut msg = BitVec::with_capacity(self.k());
            for llr in llrs.iter().take(self.k()) {
                msg.push_bit(llr.hard_decision());
            }
            msg
        }
    }

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
/// `wt = IC·w + lw`, matching the 1-line ORBGRAND enumeration of
/// Duffy–An–Médard 2022 (*IEEE Trans. Signal Proc.* 70, 4528–4542).
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
/// Yuan–Médard–Galligan–Duffy SO-GRAND (§ V).
///
/// The empty pattern `{}` is yielded first (wt = 0), representing the
/// no-flip hypothesis, and every other pattern of Hamming weight in
/// `{1, …, n}` follows exactly once.
struct LogisticWeightPatternIter {
    n: usize,
    /// 1-line ORBGRAND intercept (`IC` in the paper). 0 = basic ORBGRAND.
    ic: u32,
    /// Current `wt = IC·w + lw` being enumerated. Starts at `usize::MAX`
    /// until the empty pattern is yielded, then rolls over to 1.
    current_wt: usize,
    /// Buffer of patterns for the current `wt`, to be yielded one at a time.
    buffer: Vec<Vec<usize>>,
    /// Cursor into `buffer`.
    buffer_idx: usize,
    /// Whether the empty pattern has already been yielded.
    emitted_empty: bool,
}

impl LogisticWeightPatternIter {
    /// Convenience constructor for basic ORBGRAND (IC = 0). Identical to
    /// `LogisticWeightPatternIter::with_ic(n, 0)`. Retained for tests and
    /// proptest harnesses; production decodes call `with_ic` directly.
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

    /// Minimum logistic weight for a pattern of Hamming weight `w` over `n` positions.
    /// Choosing the `w` smallest 1-based indices: 1 + 2 + ... + w = w*(w+1)/2.
    fn min_lw_for_weight(w: usize) -> usize {
        w * (w + 1) / 2
    }

    /// Maximum logistic weight for a pattern of Hamming weight `w` over `n` positions.
    /// Choosing the `w` largest 1-based indices: (n-w+1) + ... + n.
    fn max_lw_for_weight(n: usize, w: usize) -> usize {
        if w == 0 {
            return 0;
        }
        // sum from (n-w+1) to n = w*n - w*(w-1)/2
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

        // Upper bound for the current element
        let max_val = remaining_sum
            .saturating_sub(remaining_count * (remaining_count - 1) / 2)
            .min(n);
        for val in min_val..=max_val {
            // Check remaining can still be satisfied
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
            current.push(val - 1); // Convert to 0-based
            Self::enumerate_subsets_exact(n, new_count, new_remaining, val + 1, current, result);
            current.pop();
        }
    }
}

impl Iterator for LogisticWeightPatternIter {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        // Special-case the empty pattern (wt = 0, w = 0).
        if !self.emitted_empty {
            self.emitted_empty = true;
            return Some(Vec::new());
        }

        loop {
            // Drain the buffer for the current `wt`.
            if self.buffer_idx < self.buffer.len() {
                let pattern = self.buffer[self.buffer_idx].clone();
                self.buffer_idx += 1;
                return Some(pattern);
            }

            // Advance to the next `wt` and rebuild the buffer with all
            // valid `(w, lw)` pairs for it.
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

            // For this wt, enumerate over Hamming weight w.
            // Constraint: lw = wt - IC·w ∈ [w(w+1)/2, w·n − w(w−1)/2].
            for w in 1..=self.n {
                let ic_contrib = (self.ic as usize) * w;
                if ic_contrib > wt {
                    break;
                }
                let lw = wt - ic_contrib;
                if lw < Self::min_lw_for_weight(w) {
                    // The residual logistic weight is smaller than the
                    // minimum achievable with this w. Higher w will only
                    // make the IC contribution larger, so the residual
                    // would shrink further — break out.
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

    // =====================================================================
    // Helper: Hamming(7,4) parity-check matrix
    // =====================================================================
    fn hamming_7_4_h() -> BitMatrix {
        gf2_core::bitmatrix![
            1, 1, 0, 1, 1, 0, 0;
            1, 0, 1, 1, 0, 1, 0;
            0, 1, 1, 1, 0, 0, 1
        ]
    }

    // =====================================================================
    // Logistic weight pattern iterator tests (TDD: written first)
    // =====================================================================

    #[test]
    fn test_logistic_weight_iter_first_pattern_is_empty() {
        let mut iter = LogisticWeightPatternIter::new(4);
        let first = iter.next().unwrap();
        assert!(first.is_empty(), "First pattern should be empty (LW=0)");
    }

    #[test]
    fn test_logistic_weight_iter_weight_1_patterns() {
        // For n=4 basic ORBGRAND (IC=0), patterns are enumerated by
        // ascending logistic weight, not by Hamming weight. So after the
        // initial single-bit run, weight-2 patterns with small LW appear
        // before high-rank weight-1 patterns.
        let mut iter = LogisticWeightPatternIter::new(4);
        let _ = iter.next(); // Skip empty pattern (wt=0)

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

        assert_eq!(patterns.len(), 8); // 2^3 = 8 patterns total

        // Logistic weight (sum of 1-based ranks) must be non-decreasing —
        // this is the defining invariant of basic ORBGRAND.
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
        // With IC=2 and n=4, the combined weight is wt = 2w + lw. A weight-2
        // pattern {0,1} has lw=3 but wt = 2·2+3 = 7, so it comes AFTER
        // {3} (w=1, lw=4, wt=0+4=4) and even after {0,1,2} (w=3, lw=6,
        // wt=6+6=12)? No, wt(0,1,2)=6+6=12; wt(0,1)=4+3=7; wt(3)=2+4=6.
        // So ordering for first few patterns: {} (wt=0), {0} (wt=3),
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
        // Weight 1, LW=3 with n=4: single-bit pattern {2} (1-based: {3})
        let patterns_w1 = LogisticWeightPatternIter::generate_patterns_for_weight_and_lw(4, 1, 3);
        assert_eq!(patterns_w1.len(), 1);
        assert_eq!(patterns_w1[0], vec![2]);

        // Weight 2, LW=3 with n=4: two-bit pattern {0,1} (1-based: {1,2})
        let patterns_w2 = LogisticWeightPatternIter::generate_patterns_for_weight_and_lw(4, 2, 3);
        assert_eq!(patterns_w2.len(), 1);
        assert_eq!(patterns_w2[0], vec![0, 1]);
    }

    #[test]
    fn test_logistic_weight_total_patterns() {
        // For n bits, total patterns should be 2^n
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

    // =====================================================================
    // Syndrome check tests
    // =====================================================================

    #[test]
    fn test_decode_all_zero_codeword_high_confidence() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        // All-zero codeword with high confidence (positive LLRs → bit 0)
        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = decoder.decode(&llrs);

        assert!(result.success());
        let best = result.best_codeword().unwrap();
        assert_eq!(best.noise_weight, 0);
        // All bits should be 0
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

        // Encode message [1,0,1,0] to get a valid codeword
        let mut msg = BitVec::with_capacity(4);
        msg.push_bit(true);
        msg.push_bit(false);
        msg.push_bit(true);
        msg.push_bit(false);
        let codeword = code.encode(&msg);

        // Create LLRs matching the codeword: negative for 1-bits, positive for 0-bits
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
        // The codeword is found on the first query, but ORBGRAND continues
        // testing patterns up to max_queries to collect additional codewords
        // and accumulate cumulative probability for SOGRAND soft output.
        assert!(result.query_count >= 1, "Should have at least one query");
    }

    #[test]
    fn test_decode_single_error_correction() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        // True codeword: [0,0,0,0,0,0,0] (all-zero)
        // Received with error on bit 2: [0,0,1,0,0,0,0]
        // LLRs: bit 2 has low confidence (small positive = almost flipped)
        let llrs = vec![
            Llr::new(5.0),  // bit 0: high confidence 0
            Llr::new(5.0),  // bit 1: high confidence 0
            Llr::new(-0.5), // bit 2: slightly negative → hard decision is 1 (error)
            Llr::new(5.0),  // bit 3: high confidence 0
            Llr::new(5.0),  // bit 4: high confidence 0
            Llr::new(5.0),  // bit 5: high confidence 0
            Llr::new(5.0),  // bit 6: high confidence 0
        ];

        let result = decoder.decode(&llrs);
        assert!(result.success());
        let best = result.best_codeword().unwrap();

        // Should correct to all-zero codeword
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

        // The all-zero codeword with positive LLRs should be found immediately
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

        // Use LLRs that produce a non-codeword hard decision
        // so the decoder has to search
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

    // =====================================================================
    // List decoding tests
    // =====================================================================

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

        // Low confidence on all bits — many patterns are plausible
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

        // Hamming(7,4) has 16 codewords. With max_queries=100K (covering all
        // 128 patterns for n=7), ORBGRAND finds all 16. The list grows beyond
        // list_size because ORBGRAND collects all codewords found during the
        // full query sweep for accurate SOGRAND soft output.
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

        // High confidence: all bits are 0
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

        // First codeword should be the most likely (all-zero, since all LLRs positive)
        assert!(result.success());
        let best = result.best_codeword().unwrap();
        assert_eq!(best.noise_weight, 0);

        // Should find all 16 codewords of Hamming(7,4)
        assert_eq!(
            result.codewords.len(),
            16,
            "Hamming(7,4) has 2^4=16 codewords, found {}",
            result.codewords.len()
        );

        // First codeword (all-zero) should have highest log probability
        for cw in &result.codewords[1..] {
            assert!(
                best.noise_log_probability >= cw.noise_log_probability - 1e-10,
                "All-zero codeword should be most likely when all LLRs are positive"
            );
        }
    }

    // =====================================================================
    // Even code optimization tests
    // =====================================================================

    #[test]
    fn test_even_code_reduces_queries() {
        // All Hamming(7,4) codewords have weight 0, 3, 4, or 7 —
        // not all even, so Hamming(7,4) is NOT an even code.
        // Let's test with a code that IS even.
        // Extended Hamming(8,4): add an overall parity bit.
        // H matrix for extended Hamming(8,4):
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

        // Use LLRs with varied reliabilities so patterns span many weights
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

        // Both should find all 16 codewords
        assert_eq!(result_normal.codewords.len(), 16);
        assert_eq!(result_even.codewords.len(), 16);

        // Even code optimization should roughly halve the number of queries
        // (it skips patterns with wrong parity)
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

    // =====================================================================
    // Probability tracking tests
    // =====================================================================

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

        // Cumulative log probability should be finite (not -inf) after queries
        assert!(
            result.cumulative_log_probability > f64::NEG_INFINITY,
            "Cumulative probability should be > -inf after queries"
        );
    }

    // =====================================================================
    // SoftDecoder trait implementation tests
    // =====================================================================

    #[test]
    fn test_soft_decoder_trait_decode() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        // Use SoftDecoder trait
        let soft_decoder: &dyn SoftDecoder = &decoder;
        assert_eq!(soft_decoder.k(), 4);
        assert_eq!(soft_decoder.n(), 7);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let decoded = soft_decoder.decode_soft(&llrs);
        assert_eq!(decoded.len(), 4);
        // All-zero message
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

    // =====================================================================
    // Edge cases
    // =====================================================================

    #[test]
    #[should_panic(expected = "LLR vector length")]
    fn test_decode_wrong_length_panics() {
        let h = hamming_7_4_h();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        let llrs: Vec<Llr> = vec![Llr::new(1.0); 5]; // Wrong length
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

    // =====================================================================
    // ML decoding correctness test
    // =====================================================================

    #[test]
    fn test_ml_decoding_picks_closest_codeword() {
        use crate::linear::LinearBlockCode;
        use crate::traits::BlockEncoder;

        let code = LinearBlockCode::hamming(3);
        let h = code.parity_check().unwrap().clone();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        // Encode a known message to get a valid codeword
        let mut msg = BitVec::with_capacity(4);
        msg.push_bit(true);
        msg.push_bit(true);
        msg.push_bit(false);
        msg.push_bit(true);
        let codeword = code.encode(&msg);

        // Simulate single error on bit 0: flip it in the LLRs
        // Bit 0 gets wrong hard decision with low confidence
        let llrs: Vec<Llr> = (0..7)
            .map(|i| {
                if i == 0 {
                    // Error: flip decision with low confidence
                    if codeword.get(i) {
                        Llr::new(0.2) // Was 1, received as barely-0
                    } else {
                        Llr::new(-0.2) // Was 0, received as barely-1
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

        // The ML codeword should match the original codeword (correcting the error)
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

    // =====================================================================
    // Numerical utility tests
    // =====================================================================

    #[test]
    fn test_ln_1_plus_exp_accuracy() {
        // For small x, ln(1+exp(x)) ≈ ln(2)
        let val = ln_1_plus_exp(0.0);
        assert!((val - 2.0_f64.ln()).abs() < 1e-10);

        // For large x, ln(1+exp(x)) ≈ x
        let val = ln_1_plus_exp(100.0);
        assert!((val - 100.0).abs() < 1e-10);

        // For very negative x, ln(1+exp(x)) ≈ 0
        let val = ln_1_plus_exp(-100.0);
        assert!(val.abs() < 1e-10);
    }

    #[test]
    fn test_log_sum_exp_accuracy() {
        let a = 2.0_f64.ln();
        let b = 3.0_f64.ln();
        let result = log_sum_exp(a, b);
        assert!((result - 5.0_f64.ln()).abs() < 1e-10);

        // Identity: log_sum_exp(-inf, x) = x
        assert_eq!(log_sum_exp(f64::NEG_INFINITY, 1.0), 1.0);
        assert_eq!(log_sum_exp(1.0, f64::NEG_INFINITY), 1.0);
    }

    // =====================================================================
    // Full encode-decode roundtrip with LinearBlockCode
    // =====================================================================

    #[test]
    fn test_roundtrip_hamming_7_4_all_messages() {
        use crate::linear::LinearBlockCode;
        use crate::traits::BlockEncoder;

        let code = LinearBlockCode::hamming(3); // Hamming(7,4)
        let h = code.parity_check().unwrap().clone();

        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        // Test all 16 possible 4-bit messages
        for msg_val in 0u8..16 {
            let mut msg = BitVec::with_capacity(4);
            for bit in 0..4 {
                msg.push_bit((msg_val >> bit) & 1 == 1);
            }

            let codeword = code.encode(&msg);
            assert_eq!(codeword.len(), 7);

            // Create LLRs from codeword (high confidence, no noise)
            let llrs: Vec<Llr> = (0..7)
                .map(|i| {
                    if codeword.get(i) {
                        Llr::new(-5.0) // bit 1
                    } else {
                        Llr::new(5.0) // bit 0
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

            // Verify the decoded codeword matches
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

        // Message [1,0,1,1]
        let mut msg = BitVec::with_capacity(4);
        msg.push_bit(true);
        msg.push_bit(false);
        msg.push_bit(true);
        msg.push_bit(true);
        let codeword = code.encode(&msg);

        // Test single error at each position
        for error_pos in 0..7 {
            // Create LLRs: all high confidence except the error position
            let llrs: Vec<Llr> = (0..7)
                .map(|i| {
                    let bit = codeword.get(i);
                    if i == error_pos {
                        // Error: flip the bit with low confidence
                        if bit {
                            Llr::new(0.3) // Was 1, received as 0 (barely)
                        } else {
                            Llr::new(-0.3) // Was 0, received as 1 (barely)
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

    // =====================================================================
    // Property-based tests
    // =====================================================================

    mod prop_tests {
        use super::*;
        use proptest::prelude::*;
        use std::collections::HashSet;

        proptest! {
            /// For small n (3-5), the iterator produces ALL 2^n patterns exactly once.
            #[test]
            fn test_iterator_produces_all_patterns_exactly_once(n in 3usize..=5) {
                let iter = LogisticWeightPatternIter::new(n);
                let patterns: Vec<Vec<usize>> = iter.collect();

                // Should produce exactly 2^n patterns
                let expected_count = 1usize << n;
                prop_assert_eq!(patterns.len(), expected_count,
                    "Expected {} patterns, got {}", expected_count, patterns.len());

                // Convert to sets for uniqueness check
                let mut seen = HashSet::new();
                for pattern in &patterns {
                    let key: Vec<usize> = pattern.clone();
                    prop_assert!(seen.insert(key),
                        "Duplicate pattern found: {:?}", pattern);
                }

                // Verify every subset of {0..n-1} appears
                for mask in 0..(1u32 << n) {
                    let expected: Vec<usize> = (0..n).filter(|&i| mask & (1 << i) != 0).collect();
                    prop_assert!(seen.contains(&expected),
                        "Missing pattern: {:?}", expected);
                }
            }

            /// For small n (3-5), within each weight class patterns are ordered by
            /// logistic weight sum (non-decreasing).
            #[test]
            fn test_logistic_weight_ordering(n in 3usize..=5) {
                // Basic ORBGRAND (IC=0) enumerates patterns by ascending
                // logistic weight, NOT by Hamming weight. The defining
                // invariant is: lw(pattern_i) ≤ lw(pattern_{i+1}).
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
        use crate::bch::extended::ExtendedBchCode;
        use crate::traits::BlockEncoder;

        let ebch = ExtendedBchCode::ebch_16_11();
        let h = ebch.parity_check().clone();

        let config = OrbGrandConfig {
            max_queries: 10_000,
            list_size: 1,
            even_code: true,
            systematic: true,
            list_bler_stop_threshold: None,
            one_line_intercept: OneLineIntercept::Auto,
        };
        let decoder = OrbGrand::new(h, config);

        // Encode the all-zero message
        let msg = BitVec::zeros(11);
        let codeword = ebch.encode(&msg);

        // Introduce a single error at each position
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

    /// Paper-aligned stopping rule: with `list_bler_stop_threshold = Some(t)`,
    /// the decoder must stop as soon as the predicted list-BLER drops below
    /// `t` with at least one codeword found. At high SNR this fires almost
    /// immediately — query count should be a tiny fraction of `max_queries`
    /// and the recovered codeword must still match the transmitted one.
    #[test]
    fn test_list_bler_stop_threshold_reduces_queries_at_high_snr() {
        use crate::bch::extended::ExtendedBchCode;
        use crate::traits::BlockEncoder;

        let ebch = ExtendedBchCode::ebch_16_11();
        let h = ebch.parity_check().clone();

        let msg = BitVec::zeros(11);
        let codeword = ebch.encode(&msg);

        // High-reliability channel: strong +ve LLRs for zero bits (flipped for
        // the single noise bit at position 3). ORBGRAND should find the
        // correct codeword within a handful of queries.
        let llrs: Vec<Llr> = (0..16)
            .map(|i| {
                let is_error = i == 3;
                let mag = 6.0_f32;
                if is_error {
                    // Bit was transmitted as 0, received as 1 → negative LLR.
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
