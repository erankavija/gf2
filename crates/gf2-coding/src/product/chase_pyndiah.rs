//! Chase-Pyndiah soft-input soft-output (SISO) decoder for turbo product codes.
//!
//! Implements the classical Chase-Pyndiah algorithm for iterative block turbo
//! decoding, as described in:
//!
//! - Chase, D. (1972). "A class of algorithms for decoding block codes with channel
//!   measurement information." *IEEE Trans. Inform. Theory.*
//! - Pyndiah, R.M. (1998). "Near-optimum decoding of product codes: Block turbo
//!   codes." *IEEE Trans. Commun.*
//!
//! # Algorithm Overview
//!
//! For each component decode (row or column):
//!
//! 1. **Chase search**: Identify the `p` least reliable bit positions (by |LLR|),
//!    enumerate 2^p test patterns by flipping subsets of these positions, compute
//!    the syndrome of each candidate. Valid codewords (zero syndrome) are kept as-is;
//!    invalid candidates are re-encoded from their systematic part.
//!    The maximum-likelihood (ML) codeword maximizes the bipolar correlation
//!    `M = sum_j L_j * (1 - 2*bit_j)`.
//!
//! 2. **Pyndiah soft output**: For each bit position, find the best competitor
//!    codeword (highest correlation among codewords that differ from ML at that
//!    position). The soft output is
//!    `W_i = c_ML_i_bipolar * (M_ML - M_comp_i) / 2` when a competitor exists,
//!    or `W_i = c_ML_i_bipolar * beta * min_j |L_j|` as a reliability fallback.
//!
//! 3. **Extrinsic extraction**: `L_E = W - L_input`, scaled by the per-half-iteration
//!    factor `alpha_h`.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::product::{ChasePyndiahConfig, ChasePyndiahDecoder, ProductCode};
//! use gf2_coding::bch::extended::ExtendedBchCode;
//! use gf2_coding::traits::BlockEncoder;
//! use gf2_coding::llr::Llr;
//! use gf2_core::BitVec;
//!
//! let component = ExtendedBchCode::ebch_16_11();
//! let product = ProductCode::new(component.clone());
//! let decoder = ChasePyndiahDecoder::new(component, ChasePyndiahConfig::default());
//!
//! let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
//! let result = decoder.decode(&llrs);
//! assert!(result.converged);
//! assert_eq!(result.decoded_bits.len(), product.k());
//! ```

use crate::llr::Llr;
use gf2_core::{BitMatrix, BitVec};

use super::{ProductCode, ProductComponent, TurboDecoderResult};

/// Configuration for the Chase-Pyndiah turbo product code decoder.
///
/// Controls the number of turbo iteration pairs, the Chase search depth `p`,
/// and the per-half-iteration alpha/beta schedules from Pyndiah (1998).
///
/// # Examples
///
/// ```
/// use gf2_coding::product::ChasePyndiahConfig;
///
/// let config = ChasePyndiahConfig::default();
/// assert_eq!(config.max_iterations, 8);
/// assert_eq!(config.p, 4);
/// assert_eq!(config.alpha.len(), 8);
/// assert_eq!(config.beta.len(), 8);
/// ```
#[derive(Debug, Clone)]
pub struct ChasePyndiahConfig {
    /// Maximum number of row-column iteration pairs.
    pub max_iterations: usize,

    /// Number of least reliable positions to flip in the Chase search.
    ///
    /// The search generates 2^p candidate codewords per component decode.
    /// Typical values are 3-5. Larger values improve ML approximation at
    /// exponential cost.
    pub p: usize,

    /// Per-half-iteration extrinsic scaling schedule.
    ///
    /// `alpha[h]` scales the extrinsic LLRs at half-iteration `h`.
    /// Half-iteration 0 is the first row step, 1 is the first column step,
    /// 2 is the second row step, etc.
    ///
    /// If the schedule is shorter than the total number of half-iterations,
    /// it wraps: `alpha[h % alpha.len()]`.
    pub alpha: Vec<f32>,

    /// Per-half-iteration reliability fallback schedule.
    ///
    /// `beta[h]` is used in the Pyndiah soft-output formula when no competing
    /// codeword differs from ML at a given bit position. The fallback soft
    /// output is `c_ML_i_bipolar * beta * min_j |L_j|`.
    pub beta: Vec<f32>,
}

impl Default for ChasePyndiahConfig {
    /// Creates a default configuration with Pyndiah (1998) schedules.
    ///
    /// - 8 turbo iteration pairs
    /// - Chase search depth p = 4 (16 test patterns)
    /// - Alpha schedule: [0.0, 0.2, 0.3, 0.5, 0.7, 0.9, 1.0, 1.0]
    /// - Beta schedule: [0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.2, 1.2]
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ChasePyndiahConfig;
    ///
    /// let config = ChasePyndiahConfig::default();
    /// assert_eq!(config.max_iterations, 8);
    /// assert_eq!(config.p, 4);
    /// assert!((config.alpha[0] - 0.0).abs() < 1e-10);
    /// assert!((config.beta[0] - 0.2).abs() < 1e-10);
    /// ```
    fn default() -> Self {
        Self {
            max_iterations: 8,
            p: 4,
            alpha: vec![0.0, 0.2, 0.3, 0.5, 0.7, 0.9, 1.0, 1.0],
            beta: vec![0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.2, 1.2],
        }
    }
}

/// Chase-Pyndiah turbo product code decoder.
///
/// Performs iterative block turbo decoding using the Chase-Pyndiah SISO
/// algorithm for component-level soft decoding. The turbo loop alternates
/// between row-wise and column-wise Chase-Pyndiah decodes, exchanging
/// extrinsic information with per-half-iteration alpha/beta schedules.
///
/// The type parameter `C` is the component code, which must implement
/// [`ProductComponent`] and [`Clone`].
///
/// # Examples
///
/// ```
/// use gf2_coding::product::{ChasePyndiahConfig, ChasePyndiahDecoder, ProductCode};
/// use gf2_coding::bch::extended::ExtendedBchCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_coding::llr::Llr;
/// use gf2_core::BitVec;
///
/// let component = ExtendedBchCode::ebch_16_11();
/// let product = ProductCode::new(component.clone());
/// let decoder = ChasePyndiahDecoder::new(component, ChasePyndiahConfig::default());
///
/// let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
/// let result = decoder.decode(&llrs);
/// assert!(result.converged);
/// ```
///
/// # Complexity
///
/// O(I * n * 2^p * n) where I is the number of iteration pairs, n is the
/// component code length, and p is the Chase search depth. Each iteration
/// performs 2n component decodes (n rows + n columns), each examining 2^p
/// candidate codewords.
pub struct ChasePyndiahDecoder<C: ProductComponent> {
    /// Component code for encoding and syndrome checks.
    component: C,
    /// Decoder configuration.
    config: ChasePyndiahConfig,
    /// Component codeword length.
    n: usize,
    /// Component message length.
    k: usize,
    /// Parity-check columns as bitmasks for fast syndrome computation.
    ///
    /// `h_cols[j]` is a bitmask where bit `r` is set if H[r][j] == 1.
    /// The syndrome of a candidate word is XOR of `h_cols[j]` for all set bits j.
    h_cols: Vec<u32>,
    /// Product code for validity checking and message extraction.
    product_code: ProductCode<C>,
}

impl<C: ProductComponent + Clone> ChasePyndiahDecoder<C> {
    /// Creates a new Chase-Pyndiah decoder for the given component code.
    ///
    /// Precomputes H-column bitmasks from the component parity-check matrix
    /// for fast syndrome evaluation during the Chase search.
    ///
    /// # Arguments
    ///
    /// * `component` - The component (n, k) code implementing [`ProductComponent`].
    /// * `config` - Decoder configuration with schedules and search depth.
    ///
    /// # Panics
    ///
    /// Panics if the parity-check matrix has more than 32 rows (i.e., n - k > 32),
    /// since syndrome bitmasks are stored as `u32`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{ChasePyndiahConfig, ChasePyndiahDecoder};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let decoder = ChasePyndiahDecoder::new(component, ChasePyndiahConfig::default());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n * (n - k)) for building the H-column bitmask table.
    pub fn new(component: C, config: ChasePyndiahConfig) -> Self {
        let n = component.comp_n();
        let k = component.comp_k();
        let h = component.comp_parity_check();
        let n_checks = h.rows();
        assert!(
            n_checks <= 32,
            "Chase-Pyndiah requires n-k <= 32, got {n_checks}"
        );

        // Build column bitmasks: h_cols[j] bit r = H[r][j]
        let h_cols: Vec<u32> = (0..n)
            .map(|j| {
                let mut mask = 0u32;
                for r in 0..n_checks {
                    if h.get(r, j) {
                        mask |= 1u32 << r;
                    }
                }
                mask
            })
            .collect();

        let product_code = ProductCode::new(component.clone());
        Self {
            component,
            config,
            n,
            k,
            h_cols,
            product_code,
        }
    }

    /// Decodes a received product codeword from channel LLRs.
    ///
    /// The turbo loop iterates between row-wise and column-wise Chase-Pyndiah
    /// SISO decodes, exchanging extrinsic information. Early termination occurs
    /// when the hard-decision matrix forms a valid product codeword.
    ///
    /// # Arguments
    ///
    /// * `channel_llrs` - Channel LLRs of length n^2. Positive means bit 0
    ///   is more likely; negative means bit 1.
    ///
    /// # Returns
    ///
    /// A [`TurboDecoderResult`] containing decoded message bits and statistics.
    ///
    /// # Panics
    ///
    /// Panics if `channel_llrs.len() != n^2`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{ChasePyndiahConfig, ChasePyndiahDecoder, ProductCode};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_coding::llr::Llr;
    /// use gf2_core::BitVec;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let product = ProductCode::new(component.clone());
    /// let config = ChasePyndiahConfig { max_iterations: 3, ..ChasePyndiahConfig::default() };
    /// let decoder = ChasePyndiahDecoder::new(component, config);
    ///
    /// let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
    /// let result = decoder.decode(&llrs);
    /// assert!(result.converged);
    /// assert_eq!(result.decoded_bits.len(), product.k());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(I * n * 2^p * n) per iteration pair.
    pub fn decode(&self, channel_llrs: &[Llr]) -> TurboDecoderResult {
        let n = self.n;
        let n_sq = n * n;
        assert_eq!(
            channel_llrs.len(),
            n_sq,
            "Channel LLR length {} must equal n^2 = {}",
            channel_llrs.len(),
            n_sq
        );

        // Reshape channel LLRs into n x n matrix (row-major)
        let l_ch: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| channel_llrs[i * n + j].value()).collect())
            .collect();

        // Initialize a-priori LLRs to zero
        let mut l_a: Vec<Vec<f32>> = vec![vec![0.0; n]; n];

        let mut half_iter: usize = 0;

        for iteration in 0..self.config.max_iterations {
            // === Row step ===
            let alpha_h = self.config.alpha[half_iter.min(self.config.alpha.len() - 1)];
            let beta_h = self.config.beta[half_iter.min(self.config.beta.len() - 1)];

            let mut l_e: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for i in 0..n {
                let input: Vec<f32> = (0..n).map(|j| l_ch[i][j] + l_a[i][j]).collect();
                let w = self.chase_pyndiah_siso(&input, beta_h);
                for j in 0..n {
                    l_e[i][j] = w[j] - input[j];
                }
            }

            // Check early termination on L_ch + L_A + L_E = W
            let l_total_row: Vec<Vec<f32>> = (0..n)
                .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j] + l_e[i][j]).collect())
                .collect();
            if self.check_early_termination(&l_total_row) {
                let decoded = self.extract_decoded_message(&l_total_row);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries: 0,
                    queries_per_bit: 0.0,
                };
            }

            // Update L_A = alpha_h * L_E
            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha_h * l_e[i][j];
                }
            }

            half_iter += 1;

            // === Column step ===
            let alpha_h = self.config.alpha[half_iter.min(self.config.alpha.len() - 1)];
            let beta_h = self.config.beta[half_iter.min(self.config.beta.len() - 1)];

            let mut l_e: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for j in 0..n {
                let input: Vec<f32> = (0..n).map(|i| l_ch[i][j] + l_a[i][j]).collect();
                let w = self.chase_pyndiah_siso(&input, beta_h);
                for i in 0..n {
                    l_e[i][j] = w[i] - input[i];
                }
            }

            // Check early termination on column APP
            let l_total_col: Vec<Vec<f32>> = (0..n)
                .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j] + l_e[i][j]).collect())
                .collect();
            if self.check_early_termination(&l_total_col) {
                let decoded = self.extract_decoded_message(&l_total_col);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries: 0,
                    queries_per_bit: 0.0,
                };
            }

            // Update L_A = alpha_h * L_E
            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha_h * l_e[i][j];
                }
            }

            half_iter += 1;
        }

        // Maximum iterations reached without convergence
        let final_llrs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j]).collect())
            .collect();
        let decoded = self.extract_decoded_message(&final_llrs);

        TurboDecoderResult {
            decoded_bits: decoded,
            iterations: self.config.max_iterations,
            converged: false,
            total_queries: 0,
            queries_per_bit: 0.0,
        }
    }

    /// Performs one Chase-Pyndiah SISO decode on a single row or column.
    ///
    /// # Arguments
    ///
    /// * `input` - Combined channel + a-priori LLRs (length n).
    /// * `beta` - Reliability fallback parameter for the current half-iteration.
    ///
    /// # Returns
    ///
    /// Soft output vector W of length n (the Pyndiah soft decisions).
    ///
    /// # Complexity
    ///
    /// O(2^p * n) for the Chase search plus O(n) for soft-output computation.
    fn chase_pyndiah_siso(&self, input: &[f32], _beta: f32) -> Vec<f32> {
        let n = self.n;
        let k = self.k;
        let p = self.config.p.min(n); // cap p at n

        // Step 1: Hard decision and reliability
        let hard: Vec<bool> = input.iter().map(|&l| l < 0.0).collect();
        let reliability: Vec<f32> = input.iter().map(|&l| l.abs()).collect();

        // Find p least reliable positions
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by(|&a, &b| reliability[a].partial_cmp(&reliability[b]).unwrap());
        let least_reliable: Vec<usize> = indices[..p].to_vec();

        // min reliability over all positions (for fallback formula)
        let _min_reliability = reliability.iter().copied().fold(f32::INFINITY, f32::min);

        // Step 2: Generate 2^p test patterns and find codewords
        let num_patterns = 1usize << p;
        let mut codewords: Vec<Vec<bool>> = Vec::with_capacity(num_patterns);
        let mut correlations: Vec<f32> = Vec::with_capacity(num_patterns);

        for pattern in 0..num_patterns {
            // Build candidate by flipping subsets of least reliable positions
            let mut candidate: Vec<bool> = hard.clone();
            for (bit_idx, &pos) in least_reliable.iter().enumerate() {
                if pattern & (1 << bit_idx) != 0 {
                    candidate[pos] = !candidate[pos];
                }
            }

            // Check syndrome and attempt correction
            let syndrome = self.compute_syndrome(&candidate);
            let codeword = if syndrome == 0 {
                // Valid codeword
                Some(candidate)
            } else {
                // Try single-error correction: find H column matching syndrome
                let corrected = (0..n).find(|&j| self.h_cols[j] == syndrome).map(|j| {
                    let mut c = candidate.clone();
                    c[j] = !c[j];
                    c
                });
                if corrected.is_some() {
                    corrected
                } else {
                    // Re-encode from systematic bits as fallback
                    let mut msg = BitVec::with_capacity(k);
                    for &bit in candidate.iter().take(k) {
                        msg.push_bit(bit);
                    }
                    let encoded = self.component.encode(&msg);
                    Some((0..n).map(|j| encoded.get(j)).collect())
                }
            };

            if let Some(cw) = codeword {
                // Compute bipolar correlation: M = sum_j L_j * (1 - 2*bit_j)
                let corr: f32 = (0..n)
                    .map(|j| {
                        let bipolar = if cw[j] { -1.0f32 } else { 1.0 };
                        input[j] * bipolar
                    })
                    .sum();

                codewords.push(cw);
                correlations.push(corr);
            }
        }

        // If no codewords found, return the input unchanged (no code information)
        if codewords.is_empty() {
            return input.to_vec();
        }

        // Find ML codeword (maximum correlation)
        let (ml_idx, &ml_corr) = correlations
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        let ml_codeword = &codewords[ml_idx];

        // Step 3: Pyndiah soft output
        let mut w = vec![0.0f32; n];
        for i in 0..n {
            let ml_bipolar = if ml_codeword[i] { -1.0f32 } else { 1.0 };

            // Find best competitor: highest correlation among codewords differing at position i
            let mut best_comp_corr: Option<f32> = None;
            for (idx, corr) in correlations.iter().enumerate() {
                if idx == ml_idx {
                    continue;
                }
                if codewords[idx][i] != ml_codeword[i] {
                    best_comp_corr = Some(match best_comp_corr {
                        None => *corr,
                        Some(prev) => prev.max(*corr),
                    });
                }
            }

            w[i] = match best_comp_corr {
                Some(comp_corr) => ml_bipolar * (ml_corr - comp_corr) / 2.0,
                // No competitor found: no code information for this bit.
                // Return the input LLR (extrinsic = 0) to avoid eroding the
                // channel signal. The Pyndiah (1998) fallback beta*min_reliability
                // is designed for the received-signal domain (|r|~1) and produces
                // values too small in the LLR domain, creating destructive extrinsic.
                None => input[i],
            };
        }

        w
    }

    /// Computes the syndrome of a candidate word using precomputed H-column bitmasks.
    ///
    /// # Arguments
    ///
    /// * `candidate` - A boolean vector of length n representing the candidate codeword.
    ///
    /// # Returns
    ///
    /// The syndrome as a u32 bitmask. Zero indicates a valid codeword.
    ///
    /// # Complexity
    ///
    /// O(n).
    fn compute_syndrome(&self, candidate: &[bool]) -> u32 {
        let mut syndrome = 0u32;
        for (j, &bit) in candidate.iter().enumerate() {
            if bit {
                syndrome ^= self.h_cols[j];
            }
        }
        syndrome
    }

    /// Checks if the hard decision on the given LLR matrix forms a valid product codeword.
    ///
    /// # Arguments
    ///
    /// * `llr_matrix` - n x n matrix of LLR values.
    ///
    /// # Returns
    ///
    /// `true` if the hard-decision matrix is a valid product codeword.
    fn check_early_termination(&self, llr_matrix: &[Vec<f32>]) -> bool {
        let n = self.n;
        let mut matrix = BitMatrix::zeros(n, n);
        for (i, row) in llr_matrix.iter().enumerate().take(n) {
            for (j, &val) in row.iter().enumerate().take(n) {
                if val < 0.0 {
                    matrix.set(i, j, true);
                }
            }
        }
        self.product_code.is_valid_codeword(&matrix)
    }

    /// Extracts k^2 decoded message bits from the hard decision on an LLR matrix.
    ///
    /// For a systematic code, the message bits are in the top-left k x k submatrix.
    ///
    /// # Arguments
    ///
    /// * `llr_matrix` - n x n matrix of LLR values.
    ///
    /// # Returns
    ///
    /// A bit vector of length k^2.
    fn extract_decoded_message(&self, llr_matrix: &[Vec<f32>]) -> BitVec {
        let k = self.k;
        let mut msg = BitVec::with_capacity(k * k);
        for row in llr_matrix.iter().take(k) {
            for &val in row.iter().take(k) {
                msg.push_bit(val < 0.0);
            }
        }
        msg
    }

    /// Returns a reference to the decoder configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{ChasePyndiahConfig, ChasePyndiahDecoder};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let decoder = ChasePyndiahDecoder::new(component, ChasePyndiahConfig::default());
    /// assert_eq!(decoder.config().max_iterations, 8);
    /// ```
    pub fn config(&self) -> &ChasePyndiahConfig {
        &self.config
    }

    /// Returns a reference to the component code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{ChasePyndiahConfig, ChasePyndiahDecoder, ProductComponent};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let decoder = ChasePyndiahDecoder::new(component, ChasePyndiahConfig::default());
    /// assert_eq!(decoder.component().comp_n(), 16);
    /// ```
    pub fn component(&self) -> &C {
        &self.component
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bch::extended::ExtendedBchCode;
    use crate::traits::BlockEncoder;

    #[test]
    fn test_config_default() {
        let config = ChasePyndiahConfig::default();
        assert_eq!(config.max_iterations, 8);
        assert_eq!(config.p, 4);
        assert_eq!(config.alpha.len(), 8);
        assert_eq!(config.beta.len(), 8);
        assert!((config.alpha[0] - 0.0).abs() < 1e-10);
        assert!((config.alpha[1] - 0.2).abs() < 1e-10);
        assert!((config.alpha[7] - 1.0).abs() < 1e-10);
        assert!((config.beta[0] - 0.2).abs() < 1e-10);
        assert!((config.beta[7] - 1.2).abs() < 1e-10);
    }

    #[test]
    fn test_syndrome_check_valid_codeword() {
        let component = ExtendedBchCode::ebch_16_11();
        let decoder = ChasePyndiahDecoder::new(component.clone(), ChasePyndiahConfig::default());

        // Encode a message and verify zero syndrome
        let mut msg = BitVec::with_capacity(11);
        for i in 0..11 {
            msg.push_bit(i % 3 == 0);
        }
        let codeword = component.encode(&msg);
        let candidate: Vec<bool> = (0..16).map(|j| codeword.get(j)).collect();

        let syndrome = decoder.compute_syndrome(&candidate);
        assert_eq!(
            syndrome, 0,
            "Valid codeword must have zero syndrome, got {syndrome:#010b}"
        );

        // Flip one bit and verify nonzero syndrome
        let mut corrupted = candidate.clone();
        corrupted[0] = !corrupted[0];
        let syndrome = decoder.compute_syndrome(&corrupted);
        assert_ne!(syndrome, 0, "Corrupted codeword must have nonzero syndrome");
    }

    #[test]
    fn test_chase_search_noiseless() {
        let component = ExtendedBchCode::ebch_16_11();
        let decoder = ChasePyndiahDecoder::new(
            component.clone(),
            ChasePyndiahConfig {
                p: 3,
                ..ChasePyndiahConfig::default()
            },
        );

        // All-zeros codeword with high-confidence positive LLRs
        let input: Vec<f32> = vec![10.0; 16];
        let w = decoder.chase_pyndiah_siso(&input, 0.5);

        // All soft outputs should be positive (matching bit=0 decision)
        for (i, &wi) in w.iter().enumerate() {
            assert!(
                wi > 0.0,
                "Soft output W[{i}] = {wi} should be positive for all-zeros input"
            );
        }
    }

    #[test]
    fn test_decode_all_zeros_high_snr() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = ChasePyndiahConfig {
            max_iterations: 4,
            p: 3,
            ..ChasePyndiahConfig::default()
        };
        let decoder = ChasePyndiahDecoder::new(component, config);

        // All-zeros product codeword at high SNR
        let llrs: Vec<Llr> = vec![Llr::new(8.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.converged, "Should converge at high SNR");
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "Decoded message should be all-zeros"
        );
    }
}
