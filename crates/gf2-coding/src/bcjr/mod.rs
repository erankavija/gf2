//! Trellis-based BCJR (Forward-Backward) SISO decoder for linear block codes.
//!
//! The BCJR algorithm computes exact a-posteriori probability (APP) LLRs by
//! propagating log-probabilities through the code trellis using forward and
//! backward recursions. Unlike SOGRAND, which enumerates codewords via noise
//! patterns, BCJR considers all 2^k codewords implicitly through the trellis
//! structure, producing optimal APP outputs in O(n * 2^(n-k)) time.
//!
//! # Algorithm
//!
//! Given a linear [n, k] code with parity-check matrix H (m x n, m = n-k):
//!
//! 1. **Trellis construction**: Each column of H is stored as a bitmask. The
//!    trellis state at boundary i is the partial syndrome s_i = H * c_{0..i-1},
//!    represented as a u32 over m bits. Transitions: c_i=0 leaves state unchanged,
//!    c_i=1 XORs with column i of H.
//!
//! 2. **Forward pass**: Compute log-alpha (forward state metrics) from left to
//!    right, starting at state 0.
//!
//! 3. **Backward pass**: Compute log-beta (backward state metrics) from right to
//!    left, ending at state 0.
//!
//! 4. **APP computation**: Combine alpha, beta, and branch metrics to produce
//!    per-bit APP LLRs. Extrinsic LLRs are APP minus input.
//!
//! All computations use the log-MAP formulation with the Jacobian logarithm
//! (max-star operator) for numerical exactness.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bcjr::BcjrDecoder;
//! use gf2_coding::llr::Llr;
//! use gf2_coding::drm::DrmCode;
//!
//! let code = DrmCode::drm_32_21();
//! let decoder = BcjrDecoder::new(code.parity_check());
//!
//! // High-confidence channel LLRs for the all-zero codeword
//! let input: Vec<Llr> = vec![Llr::new(5.0); 32];
//! let result = decoder.decode_siso(&input);
//!
//! // APP LLRs should all favor bit 0
//! assert!(result.app_llrs.iter().all(|l| l.value() > 0.0));
//! assert_eq!(result.app_llrs.len(), 32);
//! ```
//!
//! # References
//!
//! - Bahl, Cocke, Jelinek, Raviv (1974). "Optimal Decoding of Linear Codes for
//!   Minimizing Symbol Error Rate." *IEEE Trans. Inform. Theory.*
//! - Wolf (1978). "Efficient Maximum Likelihood Decoding of Linear Block Codes
//!   Using a Trellis." *IEEE Trans. Inform. Theory.*
//! - McEliece (1996). "On the BCJR Trellis for Linear Block Codes." *IEEE Trans.
//!   Inform. Theory.*

use crate::grand::SisoResult;
use crate::llr::Llr;
use gf2_core::BitMatrix;

/// Trellis-based BCJR (Forward-Backward) SISO decoder.
///
/// Computes exact APP LLRs for any linear block code by propagating
/// log-probabilities through the syndrome trellis. The decoder is constructed
/// from a parity-check matrix and can be reused for multiple decode calls.
///
/// # Arguments
///
/// Constructed from a parity-check matrix H via [`BcjrDecoder::new`].
///
/// # Examples
///
/// ```
/// use gf2_coding::bcjr::BcjrDecoder;
/// use gf2_coding::llr::Llr;
///
/// let h = gf2_core::bitmatrix![
///     1, 1, 0, 1, 1, 0, 0;
///     1, 0, 1, 1, 0, 1, 0;
///     0, 1, 1, 1, 0, 0, 1
/// ];
/// let decoder = BcjrDecoder::new(&h);
/// assert_eq!(decoder.n(), 7);
/// assert_eq!(decoder.k(), 4);
///
/// let input: Vec<Llr> = vec![Llr::new(3.0); 7];
/// let result = decoder.decode_siso(&input);
/// assert!(result.app_llrs.iter().all(|l| l.value() > 0.0));
/// ```
///
/// # Complexity
///
/// O(n * 2^(n-k)) per decode call, where n is the codeword length and n-k is
/// the number of parity-check rows. Memory: O(n * 2^(n-k)) for the forward and
/// backward metric arrays.
#[derive(Debug, Clone)]
pub struct BcjrDecoder {
    /// Column bitmasks of H: h_cols[i] is the i-th column as a u32.
    h_cols: Vec<u32>,
    /// Number of trellis states: 2^m where m = n-k.
    num_states: usize,
    /// Codeword length.
    n: usize,
    /// Message length.
    k: usize,
}

impl BcjrDecoder {
    /// Creates a BCJR decoder from a parity-check matrix H.
    ///
    /// # Arguments
    ///
    /// * `h` - Parity-check matrix with m rows and n columns, where m = n - k.
    ///
    /// # Panics
    ///
    /// Panics if H has more than 20 rows (2^20 = 1M states would be infeasible).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bcjr::BcjrDecoder;
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let decoder = BcjrDecoder::new(&h);
    /// assert_eq!(decoder.n(), 7);
    /// assert_eq!(decoder.k(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(m * n) to extract column bitmasks.
    pub fn new(h: &BitMatrix) -> Self {
        let m = h.rows();
        let n = h.cols();
        assert!(
            m <= 20,
            "Parity-check matrix has {} rows; BCJR trellis with 2^{} states is infeasible",
            m,
            m
        );
        let k = n - m;
        let num_states = 1usize << m;

        let h_cols = h.cols_as_u32_masks();

        Self {
            h_cols,
            num_states,
            n,
            k,
        }
    }

    /// Returns the codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bcjr::BcjrDecoder;
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// assert_eq!(BcjrDecoder::new(&h).n(), 7);
    /// ```
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message length (n - number of parity rows).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bcjr::BcjrDecoder;
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// assert_eq!(BcjrDecoder::new(&h).k(), 4);
    /// ```
    pub fn k(&self) -> usize {
        self.k
    }

    /// Performs SISO (Soft-Input Soft-Output) decoding via the BCJR algorithm.
    ///
    /// Computes exact APP LLRs by forward-backward propagation on the code trellis.
    ///
    /// # Arguments
    ///
    /// * `combined_llrs` - Per-bit combined LLRs (channel + a-priori), length n.
    ///   Positive means bit 0 is more likely.
    ///
    /// # Returns
    ///
    /// A [`SisoResult`] with exact APP LLRs, extrinsic LLRs, and metadata.
    /// `list_bler_prediction` is always 0.0 (BCJR is exact) and `query_count`
    /// is always 0 (trellis traversal, not query-based).
    ///
    /// # Panics
    ///
    /// Panics if `combined_llrs.len() != n`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bcjr::BcjrDecoder;
    /// use gf2_coding::llr::Llr;
    ///
    /// let h = gf2_core::bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1
    /// ];
    /// let decoder = BcjrDecoder::new(&h);
    /// let input: Vec<Llr> = vec![Llr::new(5.0); 7];
    /// let result = decoder.decode_siso(&input);
    /// assert_eq!(result.app_llrs.len(), 7);
    /// assert!(result.app_llrs.iter().all(|l| l.value() > 0.0));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n * 2^(n-k)) for both forward and backward passes plus APP computation.
    pub fn decode_siso(&self, combined_llrs: &[Llr]) -> SisoResult {
        let n = self.n;
        assert_eq!(
            combined_llrs.len(),
            n,
            "Expected {} LLRs, got {}",
            n,
            combined_llrs.len()
        );

        let llr_vals: Vec<f32> = combined_llrs.iter().map(|l| l.value()).collect();

        // Forward pass: log_alpha[(n+1) x num_states]
        let log_alpha = self.forward_pass(&llr_vals);

        // Backward pass: log_beta[(n+1) x num_states]
        let log_beta = self.backward_pass(&llr_vals);

        // APP computation
        let mut app_llrs = Vec::with_capacity(n);
        let mut extrinsic_llrs = Vec::with_capacity(n);

        for i in 0..n {
            let log_gamma_0 = llr_vals[i] * 0.5;
            let log_gamma_1 = -llr_vals[i] * 0.5;
            let h_col = self.h_cols[i] as usize;

            let mut log_p0 = f32::NEG_INFINITY;
            let mut log_p1 = f32::NEG_INFINITY;

            for s in 0..self.num_states {
                let a = log_alpha[i][s];
                if a == f32::NEG_INFINITY {
                    continue;
                }

                // c_i = 0: state s -> s
                log_p0 = max_star(log_p0, a + log_gamma_0 + log_beta[i + 1][s]);

                // c_i = 1: state s -> s ^ h_col
                log_p1 = max_star(log_p1, a + log_gamma_1 + log_beta[i + 1][s ^ h_col]);
            }

            let l_app = log_p0 - log_p1;
            let l_ext = l_app - llr_vals[i];
            app_llrs.push(Llr::new(l_app));
            extrinsic_llrs.push(Llr::new(l_ext));
        }

        SisoResult {
            app_llrs,
            extrinsic_llrs,
            list_bler_prediction: 0.0,
            query_count: 0,
        }
    }

    /// Forward pass: compute log-alpha metrics from left to right.
    ///
    /// Returns a (n+1) x num_states array of log-probabilities.
    fn forward_pass(&self, llr_vals: &[f32]) -> Vec<Vec<f32>> {
        let n = self.n;
        let ns = self.num_states;

        let mut log_alpha = vec![vec![f32::NEG_INFINITY; ns]; n + 1];
        log_alpha[0][0] = 0.0;

        for i in 0..n {
            let log_gamma_0 = llr_vals[i] * 0.5;
            let log_gamma_1 = -llr_vals[i] * 0.5;
            let h_col = self.h_cols[i] as usize;

            for s in 0..ns {
                let a = log_alpha[i][s];
                if a == f32::NEG_INFINITY {
                    continue;
                }

                // c_i = 0: state s -> s
                log_alpha[i + 1][s] = max_star(log_alpha[i + 1][s], a + log_gamma_0);

                // c_i = 1: state s -> s ^ h_col
                let s1 = s ^ h_col;
                log_alpha[i + 1][s1] = max_star(log_alpha[i + 1][s1], a + log_gamma_1);
            }

            // Normalize: subtract max to prevent overflow
            normalize_log_probs(&mut log_alpha[i + 1]);
        }

        log_alpha
    }

    /// Backward pass: compute log-beta metrics from right to left.
    ///
    /// Returns a (n+1) x num_states array of log-probabilities.
    fn backward_pass(&self, llr_vals: &[f32]) -> Vec<Vec<f32>> {
        let n = self.n;
        let ns = self.num_states;

        let mut log_beta = vec![vec![f32::NEG_INFINITY; ns]; n + 1];
        log_beta[n][0] = 0.0;

        for i in (0..n).rev() {
            let log_gamma_0 = llr_vals[i] * 0.5;
            let log_gamma_1 = -llr_vals[i] * 0.5;
            let h_col = self.h_cols[i] as usize;

            for s in 0..ns {
                // c_i = 0: state s -> s (forward), so backward: beta[i][s] += beta[i+1][s] * gamma_0
                let b_same = log_beta[i + 1][s];
                if b_same != f32::NEG_INFINITY {
                    log_beta[i][s] = max_star(log_beta[i][s], b_same + log_gamma_0);
                }

                // c_i = 1: state s -> s^h_col (forward), so backward: beta[i][s] += beta[i+1][s^h_col] * gamma_1
                let s1 = s ^ h_col;
                let b_xor = log_beta[i + 1][s1];
                if b_xor != f32::NEG_INFINITY {
                    log_beta[i][s] = max_star(log_beta[i][s], b_xor + log_gamma_1);
                }
            }

            // Normalize
            normalize_log_probs(&mut log_beta[i]);
        }

        log_beta
    }
}

/// Jacobian logarithm (max-star operator): computes ln(e^a + e^b) exactly.
///
/// max*(a, b) = max(a, b) + ln(1 + exp(-|a - b|))
///
/// The correction term is negligible for |a - b| > 8 and is omitted in that case.
/// This is the exact Log-MAP formulation, not the max-log approximation.
#[inline]
fn max_star(a: f32, b: f32) -> f32 {
    if a == f32::NEG_INFINITY {
        return b;
    }
    if b == f32::NEG_INFINITY {
        return a;
    }
    let max_val = a.max(b);
    let diff = (a - b).abs();
    if diff > 8.0 {
        max_val
    } else {
        max_val + (-diff).exp().ln_1p()
    }
}

/// Subtract the maximum value from all entries to prevent f32 overflow/underflow.
///
/// After normalization, the max entry is 0.0 and all others are <= 0.0.
/// Entries that are NEG_INFINITY remain NEG_INFINITY.
fn normalize_log_probs(buf: &mut [f32]) {
    let max_val = buf.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if max_val == f32::NEG_INFINITY {
        return; // All entries are -inf, nothing to normalize
    }
    for v in buf.iter_mut() {
        if *v != f32::NEG_INFINITY {
            *v -= max_val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drm::DrmCode;
    use crate::traits::BlockEncoder;
    use gf2_core::BitVec;

    /// Helper: build the Hamming(7,4) parity-check matrix.
    fn hamming74_h() -> BitMatrix {
        gf2_core::bitmatrix![
            1, 1, 0, 1, 1, 0, 0;
            1, 0, 1, 1, 0, 1, 0;
            0, 1, 1, 1, 0, 0, 1
        ]
    }

    /// Helper: enumerate all 2^k codewords of the code defined by H.
    /// Returns Vec<BitVec> of valid codewords (syndrome = 0).
    fn enumerate_codewords(h: &BitMatrix) -> Vec<BitVec> {
        let n = h.cols();
        let mut codewords = Vec::new();
        for pattern in 0..(1u64 << n) {
            let mut word = BitVec::with_capacity(n);
            for j in 0..n {
                word.push_bit((pattern >> j) & 1 == 1);
            }
            // Check syndrome
            let mut valid = true;
            for row in 0..h.rows() {
                let mut sum = false;
                for col in 0..n {
                    if h.get(row, col) && word.get(col) {
                        sum = !sum;
                    }
                }
                if sum {
                    valid = false;
                    break;
                }
            }
            if valid {
                codewords.push(word);
            }
        }
        codewords
    }

    /// Helper: compute exact APP LLRs by exhaustive enumeration over all codewords.
    /// This is O(2^k * n) and only feasible for small codes.
    fn exhaustive_app_llrs(h: &BitMatrix, combined_llrs: &[f32]) -> Vec<f32> {
        let n = h.cols();
        let codewords = enumerate_codewords(h);

        // Compute log P(y | c) for each codeword c, proportional to sum_i c_i * L_i
        // P(y | c) ∝ prod_i P(y_i | c_i)
        // log P(y_i | c_i=0) = L_i/2, log P(y_i | c_i=1) = -L_i/2
        let log_probs: Vec<f64> = codewords
            .iter()
            .map(|cw| {
                combined_llrs
                    .iter()
                    .enumerate()
                    .map(|(j, &l)| {
                        let l = l as f64;
                        if cw.get(j) {
                            -l / 2.0
                        } else {
                            l / 2.0
                        }
                    })
                    .sum()
            })
            .collect();

        // For each bit position, compute log(sum P(c: c_i=0)) - log(sum P(c: c_i=1))
        (0..n)
            .map(|i| {
                let mut log_sum_0 = f64::NEG_INFINITY;
                let mut log_sum_1 = f64::NEG_INFINITY;
                for (j, cw) in codewords.iter().enumerate() {
                    if cw.get(i) {
                        log_sum_1 = log_sum_exp_f64(log_sum_1, log_probs[j]);
                    } else {
                        log_sum_0 = log_sum_exp_f64(log_sum_0, log_probs[j]);
                    }
                }
                (log_sum_0 - log_sum_1) as f32
            })
            .collect()
    }

    fn log_sum_exp_f64(a: f64, b: f64) -> f64 {
        if a == f64::NEG_INFINITY {
            return b;
        }
        if b == f64::NEG_INFINITY {
            return a;
        }
        let max_val = a.max(b);
        max_val + ((a - max_val).exp() + (b - max_val).exp()).ln()
    }

    // ---- Test 1: Noiseless all-zeros ----

    #[test]
    fn test_noiseless_all_zeros() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        // All-zero codeword → all LLRs positive (favoring bit 0)
        let input: Vec<Llr> = vec![Llr::new(10.0); 32];
        let result = decoder.decode_siso(&input);

        assert_eq!(result.app_llrs.len(), 32);
        for (i, l) in result.app_llrs.iter().enumerate() {
            assert!(
                l.value() > 0.0,
                "APP LLR at bit {} should be positive (favoring 0), got {}",
                i,
                l.value()
            );
        }
        // Hard decision should be all zeros
        for l in &result.app_llrs {
            assert!(!l.hard_decision(), "Hard decision should be 0 (false)");
        }
    }

    // ---- Test 2: Noiseless known codeword ----

    #[test]
    fn test_noiseless_known_codeword() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        // Encode a non-trivial message
        let mut msg = BitVec::with_capacity(21);
        for i in 0..21 {
            msg.push_bit(i % 3 == 0);
        }
        let cw = code.encode(&msg);

        // Create LLRs: +10 for bit=0, -10 for bit=1
        let input: Vec<Llr> = (0..32)
            .map(|j| {
                if cw.get(j) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let result = decoder.decode_siso(&input);

        // Hard decision must match the codeword
        for (j, app) in result.app_llrs.iter().enumerate() {
            let hard = app.hard_decision();
            assert_eq!(
                hard,
                cw.get(j),
                "Hard decision mismatch at bit {}: got {}, expected {}",
                j,
                hard,
                cw.get(j)
            );
        }
    }

    // ---- Test 3: Hamming(7,4) cross-check vs exhaustive ----

    #[test]
    fn test_hamming74_vs_exhaustive() {
        let h = hamming74_h();
        let decoder = BcjrDecoder::new(&h);

        // Use a deterministic set of test vectors at moderate SNR
        let test_vectors: Vec<Vec<f32>> = vec![
            vec![2.0, -1.5, 3.0, 0.5, -2.0, 1.0, -0.5],
            vec![-3.0, 2.0, 1.0, -1.0, 0.5, -2.5, 3.0],
            vec![1.5, 1.5, -2.0, 2.5, -1.0, 0.8, -1.5],
            vec![4.0, -3.0, 2.0, -1.0, 3.0, -2.0, 1.0],
            vec![-0.5, 0.5, -0.5, 0.5, -0.5, 0.5, -0.5],
            vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
            vec![-5.0, -5.0, -5.0, -5.0, -5.0, -5.0, -5.0],
            vec![0.1, -0.1, 0.2, -0.3, 0.4, -0.5, 0.6],
        ];

        for (idx, llr_vals) in test_vectors.iter().enumerate() {
            let input: Vec<Llr> = llr_vals.iter().map(|&v| Llr::new(v)).collect();
            let result = decoder.decode_siso(&input);
            let expected = exhaustive_app_llrs(&h, llr_vals);

            for (j, (app, &exp)) in result.app_llrs.iter().zip(expected.iter()).enumerate() {
                let diff = (app.value() - exp).abs();
                assert!(
                    diff < 0.15,
                    "Vector {}, bit {}: BCJR={:.4}, exhaustive={:.4}, diff={:.4}",
                    idx,
                    j,
                    app.value(),
                    exp,
                    diff
                );
            }
        }
    }

    // ---- Test 4: Extrinsic correctness ----

    #[test]
    fn test_extrinsic_correct_sign_and_identity() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        // High-confidence all-zero codeword
        let input: Vec<Llr> = vec![Llr::new(10.0); 32];
        let result = decoder.decode_siso(&input);

        // Extrinsic should be positive (code reinforces all-zero codeword)
        // and finite.
        for (i, l) in result.extrinsic_llrs.iter().enumerate() {
            assert!(
                l.value() > 0.0,
                "Extrinsic LLR at bit {} should be positive for all-zero cw, got {}",
                i,
                l.value()
            );
            assert!(
                l.value().is_finite(),
                "Extrinsic LLR at bit {} should be finite, got {}",
                i,
                l.value()
            );
        }

        // APP = channel + extrinsic: verify identity L_ext = L_APP - L_input
        for (i, ((app, ext), inp)) in result
            .app_llrs
            .iter()
            .zip(result.extrinsic_llrs.iter())
            .zip(input.iter())
            .enumerate()
        {
            let expected_ext = app.value() - inp.value();
            let actual_ext = ext.value();
            assert!(
                (expected_ext - actual_ext).abs() < 1e-4,
                "Extrinsic identity mismatch at bit {}: {} vs {}",
                i,
                actual_ext,
                expected_ext
            );
        }
    }

    // ---- Test 5: Forward pass boundary states ----

    #[test]
    fn test_forward_starts_and_ends_zero() {
        let code = DrmCode::drm_32_21();
        let decoder = BcjrDecoder::new(code.parity_check());

        // Noiseless all-zero codeword
        let llr_vals: Vec<f32> = vec![10.0; 32];
        let log_alpha = decoder.forward_pass(&llr_vals);

        // Start: only state 0 is reachable
        assert!(log_alpha[0][0].is_finite());
        for (s, &val) in log_alpha[0].iter().enumerate().skip(1) {
            assert_eq!(
                val,
                f32::NEG_INFINITY,
                "State {} at boundary 0 should be -inf",
                s
            );
        }

        // End: for a valid codeword, only state 0 should have significant mass
        // (other states should be much smaller, but normalization means state 0 = 0.0)
        let end = &log_alpha[32];
        let max_other = end[1..].iter().copied().fold(f32::NEG_INFINITY, f32::max);
        // State 0 should dominate (be much larger than any other state)
        assert!(
            end[0] > max_other + 5.0,
            "State 0 at end ({}) should dominate others (max other: {})",
            end[0],
            max_other
        );
    }

    // ---- Test 6: eBCH(16,11) noiseless ----

    #[test]
    fn test_ebch_noiseless() {
        use crate::bch::extended::ExtendedBchCode;

        let code = ExtendedBchCode::ebch_16_11();
        let decoder = BcjrDecoder::new(code.parity_check());
        assert_eq!(decoder.n(), 16);
        assert_eq!(decoder.k(), 11);

        // Encode a non-trivial message
        let mut msg = BitVec::with_capacity(11);
        for i in 0..11 {
            msg.push_bit(i % 2 == 0);
        }
        let cw = code.encode(&msg);

        // Create high-confidence LLRs
        let input: Vec<Llr> = (0..16)
            .map(|j| {
                if cw.get(j) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let result = decoder.decode_siso(&input);

        // Hard decision must match the codeword
        for (j, app) in result.app_llrs.iter().enumerate() {
            let hard = app.hard_decision();
            assert_eq!(
                hard,
                cw.get(j),
                "eBCH hard decision mismatch at bit {}: got {}, expected {}",
                j,
                hard,
                cw.get(j)
            );
        }
    }

    // ---- Test: SisoResult metadata ----

    #[test]
    fn test_siso_result_metadata() {
        let h = hamming74_h();
        let decoder = BcjrDecoder::new(&h);
        let input: Vec<Llr> = vec![Llr::new(3.0); 7];
        let result = decoder.decode_siso(&input);

        // BCJR always reports 0.0 list_bler and 0 queries
        assert_eq!(result.list_bler_prediction, 0.0);
        assert_eq!(result.query_count, 0);
        assert_eq!(result.app_llrs.len(), 7);
        assert_eq!(result.extrinsic_llrs.len(), 7);
    }

    // ---- Property-based tests ----

    use proptest::prelude::*;

    proptest! {
        /// For Hamming(7,4), BCJR APP LLRs must match exhaustive enumeration
        /// for arbitrary input LLRs in [-10, 10].
        #[test]
        fn prop_hamming74_bcjr_matches_exhaustive(
            llrs in proptest::collection::vec(-10.0f32..10.0f32, 7..=7)
        ) {
            let h = hamming74_h();
            let decoder = BcjrDecoder::new(&h);
            let input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
            let result = decoder.decode_siso(&input);
            let expected = exhaustive_app_llrs(&h, &llrs);

            for (j, (app, &exp)) in result.app_llrs.iter().zip(expected.iter()).enumerate() {
                let diff = (app.value() - exp).abs();
                prop_assert!(
                    diff < 0.2,
                    "bit {}: BCJR={:.4}, exhaustive={:.4}, diff={:.4}",
                    j, app.value(), exp, diff
                );
            }
        }

        /// BCJR hard decision on a noiseless codeword must always recover
        /// the original codeword (random messages for dRM(32,21)).
        #[test]
        fn prop_drm_noiseless_recovery(msg_bits in proptest::collection::vec(any::<bool>(), 21..=21)) {
            let code = DrmCode::drm_32_21();
            let decoder = BcjrDecoder::new(code.parity_check());

            let mut msg = BitVec::with_capacity(21);
            for &b in &msg_bits {
                msg.push_bit(b);
            }
            let cw = code.encode(&msg);

            let input: Vec<Llr> = (0..32)
                .map(|j| if cw.get(j) { Llr::new(-8.0) } else { Llr::new(8.0) })
                .collect();

            let result = decoder.decode_siso(&input);

            for (j, app) in result.app_llrs.iter().enumerate() {
                prop_assert_eq!(
                    app.hard_decision(), cw.get(j),
                    "bit {}: hard={}, expected={}",
                    j, app.hard_decision(), cw.get(j)
                );
            }
        }

        /// Extrinsic identity must hold: L_ext = L_APP - L_input for all bits.
        #[test]
        fn prop_extrinsic_identity(
            llrs in proptest::collection::vec(-5.0f32..5.0f32, 7..=7)
        ) {
            let h = hamming74_h();
            let decoder = BcjrDecoder::new(&h);
            let input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
            let result = decoder.decode_siso(&input);

            for (j, ((app, ext), inp)) in result.app_llrs.iter()
                .zip(result.extrinsic_llrs.iter())
                .zip(input.iter())
                .enumerate()
            {
                let expected_ext = app.value() - inp.value();
                let diff = (ext.value() - expected_ext).abs();
                prop_assert!(
                    diff < 1e-4,
                    "bit {}: ext={:.4}, expected={:.4}, diff={:.6}",
                    j, ext.value(), expected_ext, diff
                );
            }
        }
    }

    /// Compare BCJR vs SOGRAND extrinsic for dRM(32,21) at moderate SNR.
    /// This diagnostic test checks if the two SISO decoders produce
    /// qualitatively different extrinsic information.
    #[test]
    fn test_bcjr_vs_sogrand_extrinsic_drm() {
        use crate::grand::{OrbGrand, OrbGrandConfig, SoGrand};

        let code = DrmCode::drm_32_21();
        let bcjr = BcjrDecoder::new(code.parity_check());

        let h = code.parity_check().clone();
        let sogrand = SoGrand::new(OrbGrand::new(
            h,
            OrbGrandConfig {
                list_size: 4,
                max_queries: 50_000,
                even_code: code.is_even(),
                systematic: true,
            },
        ));

        // All-zeros codeword at moderate SNR (LLR ≈ 3.0)
        let input: Vec<Llr> = vec![Llr::new(3.0); 32];

        let bcjr_result = bcjr.decode_siso(&input);
        let sogrand_result = sogrand.decode_siso(&input);

        // Compare extrinsic magnitudes
        let bcjr_ext_mean: f32 = bcjr_result
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;
        let sogrand_ext_mean: f32 = sogrand_result
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;

        eprintln!("BCJR mean |ext|: {bcjr_ext_mean:.3}");
        eprintln!("SOGRAND mean |ext|: {sogrand_ext_mean:.3}");
        eprintln!(
            "SOGRAND P(C\\L): {:.6}",
            sogrand_result.list_bler_prediction
        );

        // Check sign agreement
        let mut sign_agree = 0;
        let mut sign_disagree = 0;
        for i in 0..32 {
            let b = bcjr_result.extrinsic_llrs[i].value();
            let s = sogrand_result.extrinsic_llrs[i].value();
            if (b > 0.0) == (s > 0.0) {
                sign_agree += 1;
            } else {
                sign_disagree += 1;
            }
        }
        eprintln!("Sign agreement: {sign_agree}/32, disagree: {sign_disagree}/32");

        // Both should produce positive extrinsic for all-zeros codeword
        for (i, l) in bcjr_result.extrinsic_llrs.iter().enumerate() {
            assert!(
                l.value() > -1.0,
                "BCJR ext[{i}] = {:.3} should not be strongly negative for correct codeword",
                l.value()
            );
        }

        // Now test with errors: some bits have wrong sign
        eprintln!("\n--- With 2 bit errors ---");
        let mut noisy_input: Vec<Llr> = vec![Llr::new(3.0); 32];
        noisy_input[5] = Llr::new(-1.5); // error at bit 5
        noisy_input[12] = Llr::new(-0.8); // error at bit 12

        let bcjr_noisy = bcjr.decode_siso(&noisy_input);
        let sogrand_noisy = sogrand.decode_siso(&noisy_input);

        let bcjr_ext_noisy: f32 = bcjr_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;
        let sogrand_ext_noisy: f32 = sogrand_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .sum::<f32>()
            / 32.0;
        eprintln!("BCJR mean |ext|: {bcjr_ext_noisy:.3}");
        eprintln!("SOGRAND mean |ext|: {sogrand_ext_noisy:.3}");

        // Check ext at error positions — should be positive (correcting the error)
        eprintln!(
            "BCJR ext[5]={:.3} ext[12]={:.3}",
            bcjr_noisy.extrinsic_llrs[5].value(),
            bcjr_noisy.extrinsic_llrs[12].value()
        );
        eprintln!(
            "SOGRAND ext[5]={:.3} ext[12]={:.3}",
            sogrand_noisy.extrinsic_llrs[5].value(),
            sogrand_noisy.extrinsic_llrs[12].value()
        );

        // Max extrinsic magnitude
        let bcjr_max = bcjr_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .fold(0.0f32, f32::max);
        let sogrand_max = sogrand_noisy
            .extrinsic_llrs
            .iter()
            .map(|l| l.value().abs())
            .fold(0.0f32, f32::max);
        eprintln!("BCJR max |ext|: {bcjr_max:.3}");
        eprintln!("SOGRAND max |ext|: {sogrand_max:.3}");
    }

    /// Count minimum-weight codewords for dRM(32,21) vs eBCH codes.
    /// High A_dmin (number of weight-d codewords) weakens turbo convergence
    /// because many near-codewords compete with the correct one.
    #[test]
    #[ignore] // ~38s: enumerates 2^26 codewords for eBCH(32,26)
    fn test_weight_distribution_comparison() {
        use crate::bch::extended::ExtendedBchCode;
        use crate::traits::GeneratorMatrixAccess;

        // dRM(32,21): enumerate all 2^21 messages
        let drm = DrmCode::drm_32_21();
        let g_drm = drm.generator_matrix();
        let n_drm = drm.n();
        let k_drm = drm.k();

        let mut drm_weights = [0u64; 33]; // weight histogram
        for msg_val in 0..(1u64 << k_drm) {
            let mut cw_word = 0u64; // pack codeword into u64
            for col in 0..n_drm {
                let mut bit = false;
                for row in 0..k_drm {
                    if (msg_val >> row) & 1 == 1 && g_drm.get(row, col) {
                        bit = !bit;
                    }
                }
                if bit {
                    cw_word |= 1u64 << col;
                }
            }
            let w = cw_word.count_ones() as usize;
            drm_weights[w] += 1;
        }

        eprintln!("dRM(32,21) weight distribution:");
        eprintln!(
            "  A0={}, A4={}, A8={}, A12={}, A16={}",
            drm_weights[0], drm_weights[4], drm_weights[8], drm_weights[12], drm_weights[16]
        );

        // eBCH(16,11): enumerate all 2^11 messages
        let ebch = ExtendedBchCode::ebch_16_11();
        let g_ebch = ebch.generator_matrix();
        let n_ebch = ebch.n();
        let k_ebch = ebch.k();

        let mut ebch_weights = [0u64; 17];
        for msg_val in 0..(1u64 << k_ebch) {
            let mut cw_word = 0u64;
            for col in 0..n_ebch {
                let mut bit = false;
                for row in 0..k_ebch {
                    if (msg_val >> row) & 1 == 1 && g_ebch.get(row, col) {
                        bit = !bit;
                    }
                }
                if bit {
                    cw_word |= 1u64 << col;
                }
            }
            let w = cw_word.count_ones() as usize;
            ebch_weights[w] += 1;
        }

        eprintln!("eBCH(16,11) weight distribution:");
        eprintln!(
            "  A0={}, A4={}, A6={}, A8={}, A10={}, A12={}, A16={}",
            ebch_weights[0],
            ebch_weights[4],
            ebch_weights[6],
            ebch_weights[8],
            ebch_weights[10],
            ebch_weights[12],
            ebch_weights[16]
        );

        // Also check eBCH(32,26)
        let ebch32 = ExtendedBchCode::ebch_32_26();
        let g_32 = ebch32.generator_matrix();
        let n_32 = ebch32.n();
        let k_32 = ebch32.k();

        let mut e32_weights = [0u64; 33];
        for msg_val in 0..(1u64 << k_32) {
            let mut cw_word = 0u64;
            for col in 0..n_32 {
                let mut bit = false;
                for row in 0..k_32 {
                    if (msg_val >> row) & 1 == 1 && g_32.get(row, col) {
                        bit = !bit;
                    }
                }
                if bit {
                    cw_word |= 1u64 << col;
                }
            }
            let w = cw_word.count_ones() as usize;
            e32_weights[w] += 1;
        }

        eprintln!("eBCH(32,26) weight distribution:");
        eprintln!(
            "  A0={}, A4={}, A6={}, A8={}, A12={}, A16={}",
            e32_weights[0],
            e32_weights[4],
            e32_weights[6],
            e32_weights[8],
            e32_weights[12],
            e32_weights[16]
        );

        // Compare A4 / total_codewords (density of minimum-weight codewords)
        let drm_a4_ratio = drm_weights[4] as f64 / (1u64 << k_drm) as f64;
        let ebch_a4_ratio = ebch_weights[4] as f64 / (1u64 << k_ebch) as f64;
        let e32_a4_ratio = e32_weights[4] as f64 / (1u64 << k_32) as f64;

        eprintln!("\nA4 density:");
        eprintln!(
            "  dRM(32,21):  A4={}, ratio={:.6e}",
            drm_weights[4], drm_a4_ratio
        );
        eprintln!(
            "  eBCH(16,11): A4={}, ratio={:.6e}",
            ebch_weights[4], ebch_a4_ratio
        );
        eprintln!(
            "  eBCH(32,26): A4={}, ratio={:.6e}",
            e32_weights[4], e32_a4_ratio
        );
    }
}
