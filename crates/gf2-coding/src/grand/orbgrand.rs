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
}

impl Default for OrbGrandConfig {
    fn default() -> Self {
        Self {
            max_queries: 1_000_000,
            list_size: 1,
            even_code: false,
            systematic: true,
        }
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

        // Generate noise patterns in logistic-weight order using the
        // partition-based enumeration.
        let pattern_iter = LogisticWeightPatternIter::new(self.n);

        let mut list_full = false;
        // After the list is full, continue accumulating probability up to
        // this many additional patterns. For n=16 (65536 total), this covers
        // all patterns. For n=32 (4B total), this limits the overhead to a
        // bounded number of extra iterations instead of running to max_queries.
        let post_list_budget = (1usize << self.n.min(16)).min(self.config.max_queries);
        let mut post_list_count = 0usize;

        for pattern in pattern_iter {
            if query_count >= self.config.max_queries {
                break;
            }
            if list_full {
                post_list_count += 1;
                // Stop if cumulative is near 1.0 or budget exhausted
                if cumulative_log_prob > -1e-6 || post_list_count >= post_list_budget {
                    break;
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

            // Accumulate cumulative probability for ALL tested patterns,
            // including those skipped by even-code optimization. This is
            // critical for SOGRAND: if cumulative probability is too low,
            // P(C\L) dominates and APP LLRs collapse to channel LLRs.
            cumulative_log_prob = log_sum_exp(cumulative_log_prob, noise_log_prob);

            // Even code optimization: skip syndrome check if parity won't match.
            // Probability is already accumulated above.
            if self.config.even_code {
                let noise_parity = noise_weight % 2 == 1;
                if hard_parity ^ noise_parity {
                    continue;
                }
            }

            // If the list is already full, continue iterating to accumulate
            // probability but don't check for more codewords.
            if list_full {
                continue;
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

                if codewords.len() >= self.config.list_size {
                    list_full = true;
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

/// Iterator that generates noise patterns in **weight-tiered** logistic-weight order.
///
/// Patterns are grouped by Hamming weight and enumerated tier by tier:
///
/// 1. **Weight 0**: the empty pattern `{}`.
/// 2. **Weight 1**: single-bit patterns `{i}`, ordered by reliability index
///    (least reliable first, i.e., ascending `i`).
/// 3. **Weight 2**: two-bit patterns `{i, j}`, ordered by the sum of their
///    1-based reliability indices (logistic weight), with ties broken
///    lexicographically.
/// 4. And so on for higher Hamming weights.
///
/// Within each weight tier, the logistic weight of a pattern `{i_1, ..., i_w}`
/// (0-based indices into the reliability-sorted order) is
/// `(i_1 + 1) + ... + (i_w + 1)`.
///
/// This ordering ensures that all weight-`w` patterns are enumerated before
/// any weight-`(w+1)` pattern, matching the standard 1-line ORBGRAND definition.
struct LogisticWeightPatternIter {
    n: usize,
    /// Current Hamming weight tier being enumerated.
    current_weight: usize,
    /// Current logistic weight within the current Hamming weight tier.
    current_lw: usize,
    /// Buffer of patterns at the current (weight, lw), to be yielded.
    buffer: Vec<Vec<usize>>,
    /// Index into the buffer.
    buffer_idx: usize,
}

impl LogisticWeightPatternIter {
    fn new(n: usize) -> Self {
        // Start with Hamming weight 0, logistic weight 0: the empty pattern
        Self {
            n,
            current_weight: 0,
            current_lw: 0,
            buffer: vec![vec![]],
            buffer_idx: 0,
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
        loop {
            // Yield buffered patterns at current (weight, lw)
            if self.buffer_idx < self.buffer.len() {
                let pattern = self.buffer[self.buffer_idx].clone();
                self.buffer_idx += 1;
                return Some(pattern);
            }

            // Advance within the current weight tier
            self.current_lw += 1;

            let max_lw = Self::max_lw_for_weight(self.n, self.current_weight);
            if self.current_lw > max_lw {
                // Move to next weight tier
                self.current_weight += 1;
                if self.current_weight > self.n {
                    return None;
                }
                self.current_lw = Self::min_lw_for_weight(self.current_weight);
            }

            self.buffer = Self::generate_patterns_for_weight_and_lw(
                self.n,
                self.current_weight,
                self.current_lw,
            );
            self.buffer_idx = 0;
            // If this (weight, lw) has no patterns, loop to next lw
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
        // For n=4, weight-1 patterns in LW order:
        // LW=1: {0}, LW=2: {1}, LW=3: {2}, LW=4: {3}
        // All weight-1 patterns come before any weight-2 pattern.
        let mut iter = LogisticWeightPatternIter::new(4);
        let _ = iter.next(); // Skip empty pattern (LW=0)

        assert_eq!(iter.next().unwrap(), vec![0]); // LW=1
        assert_eq!(iter.next().unwrap(), vec![1]); // LW=2
        assert_eq!(iter.next().unwrap(), vec![2]); // LW=3
        assert_eq!(iter.next().unwrap(), vec![3]); // LW=4

        // Next should be weight-2 patterns, starting with {0,1} (LW=3)
        assert_eq!(iter.next().unwrap(), vec![0, 1]); // LW=3
    }

    #[test]
    fn test_logistic_weight_iter_all_patterns_n3() {
        // For n=3, weight-tiered order:
        // Weight 0: {} (LW=0)
        // Weight 1: {0} (LW=1), {1} (LW=2), {2} (LW=3)
        // Weight 2: {0,1} (LW=3), {0,2} (LW=4), {1,2} (LW=5)
        // Weight 3: {0,1,2} (LW=6)
        let iter = LogisticWeightPatternIter::new(3);
        let patterns: Vec<Vec<usize>> = iter.collect();

        assert_eq!(patterns.len(), 8); // 2^3 = 8 patterns total

        // Check weight-tiered ordering: Hamming weight must be non-decreasing
        for i in 1..patterns.len() {
            let w_prev = patterns[i - 1].len();
            let w_curr = patterns[i].len();
            assert!(
                w_prev <= w_curr,
                "Patterns must be in non-decreasing Hamming weight order: {} vs {}",
                w_prev,
                w_curr
            );
        }

        // Within each weight tier, logistic weight must be non-decreasing
        let mut i = 0;
        while i < patterns.len() {
            let w = patterns[i].len();
            let start = i;
            while i < patterns.len() && patterns[i].len() == w {
                i += 1;
            }
            // Check LW ordering within this tier
            for j in (start + 1)..i {
                let lw_prev: usize = patterns[j - 1].iter().map(|&x| x + 1).sum();
                let lw_curr: usize = patterns[j].iter().map(|&x| x + 1).sum();
                assert!(
                    lw_prev <= lw_curr,
                    "Within weight {}, LW must be non-decreasing: {} vs {}",
                    w,
                    lw_prev,
                    lw_curr
                );
            }
        }
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
        assert_eq!(
            result.query_count, 1,
            "Should find codeword on first query (empty pattern)"
        );
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

        // Hamming(7,4) has 16 codewords, so with enough queries we should find several
        assert!(
            result.codewords.len() >= 2,
            "Expected at least 2 codewords in list mode, got {}",
            result.codewords.len()
        );
        assert!(result.codewords.len() <= 3, "Should not exceed list_size=3");
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
        };
        let decoder_normal = OrbGrand::new(h_ext.clone(), config_normal);

        let config_even = OrbGrandConfig {
            max_queries: 1_000_000,
            list_size: 16,
            even_code: true,
            systematic: true,
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
            fn test_weight_tiered_ordering(n in 3usize..=5) {
                let iter = LogisticWeightPatternIter::new(n);
                let patterns: Vec<Vec<usize>> = iter.collect();

                // Group patterns by Hamming weight and verify ordering
                let mut prev_weight = 0usize;
                let mut prev_lw = 0usize;

                for pattern in &patterns {
                    let weight = pattern.len();
                    let lw: usize = pattern.iter().map(|&x| x + 1).sum();

                    if weight == prev_weight {
                        // Within same weight: LW must be non-decreasing
                        prop_assert!(lw >= prev_lw,
                            "Within weight {}, LW decreased: {} -> {} for pattern {:?}",
                            weight, prev_lw, lw, pattern);
                    } else {
                        // Weight must increase (never decrease)
                        prop_assert!(weight > prev_weight,
                            "Weight decreased: {} -> {} for pattern {:?}",
                            prev_weight, weight, pattern);
                    }
                    prev_weight = weight;
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
}
