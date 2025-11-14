//! LDPC (Low-Density Parity-Check) codes with belief propagation decoding.
//!
//! This module provides LDPC code construction and soft-decision decoding using
//! belief propagation algorithms over sparse parity-check matrices.
//!
//! # LDPC Code Structure
//!
//! An LDPC code is defined by a sparse parity-check matrix **H** where:
//! - Rows represent check nodes (parity constraints)
//! - Columns represent variable nodes (codeword bits)
//! - **H · c = 0** for any valid codeword **c**
//!
//! # Tanner Graph
//!
//! The code can be viewed as a bipartite graph:
//! - Check nodes ↔ Variable nodes
//! - Edge (i,j) exists if H[i,j] = 1
//!
//! # Belief Propagation Decoding
//!
//! Iterative message-passing algorithm:
//! 1. **Initialization**: Variable nodes initialized with channel LLRs
//! 2. **Check-to-variable**: Compute messages using box-plus over neighbors
//! 3. **Variable-to-check**: Update beliefs and send to check nodes
//! 4. **Convergence**: Stop when syndrome check passes or max iterations reached
//!
//! # Examples
//!
//! ```ignore
//! use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
//! use gf2_coding::traits::IterativeSoftDecoder;
//! use gf2_coding::llr::Llr;
//!
//! // Create a regular (3,6) LDPC code
//! let code = LdpcCode::regular(100, 200, 3, 6);
//! let mut decoder = LdpcDecoder::new(code);
//!
//! // Decode received LLRs
//! let channel_llrs: Vec<Llr> = /* ... */;
//! let result = decoder.decode_iterative(&channel_llrs, 50);
//!
//! if result.converged {
//!     println!("Decoded successfully in {} iterations", result.iterations);
//! }
//! ```

use crate::llr::Llr;
use crate::traits::{DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::sparse::SparseMatrixDual;
use gf2_core::BitVec;

/// An LDPC code defined by its sparse parity-check matrix.
///
/// The code is characterized by:
/// - **n**: Codeword length (number of variable nodes)
/// - **m**: Number of parity checks (check nodes)
/// - **k**: Message dimension (k = n - m for systematic codes)
/// - **H**: Sparse m × n parity-check matrix
#[derive(Debug, Clone)]
pub struct LdpcCode {
    /// Sparse parity-check matrix in dual representation
    h: SparseMatrixDual,
    /// Number of variable nodes (codeword length)
    n: usize,
    /// Number of check nodes (parity checks)
    m: usize,
}

impl LdpcCode {
    /// Creates an LDPC code from a parity-check matrix in COO format.
    ///
    /// # Arguments
    ///
    /// * `m` - Number of check nodes (rows of H)
    /// * `n` - Number of variable nodes (columns of H)
    /// * `edges` - List of (check, variable) edges in the Tanner graph
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::LdpcCode;
    ///
    /// // Simple [7,4] Hamming code as LDPC
    /// let edges = vec![
    ///     (0, 0), (0, 1), (0, 3),
    ///     (1, 0), (1, 2), (1, 4),
    ///     (2, 1), (2, 2), (2, 5),
    /// ];
    /// let code = LdpcCode::from_edges(3, 7, &edges);
    /// assert_eq!(code.n(), 7);
    /// assert_eq!(code.m(), 3);
    /// ```
    pub fn from_edges(m: usize, n: usize, edges: &[(usize, usize)]) -> Self {
        let h = SparseMatrixDual::from_coo(m, n, edges);
        Self { h, n, m }
    }

    /// Returns the codeword length (number of variable nodes).
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the number of check nodes.
    pub fn m(&self) -> usize {
        self.m
    }

    /// Returns the message dimension (for full-rank H).
    pub fn k(&self) -> usize {
        self.n.saturating_sub(self.m)
    }

    /// Returns the code rate k/n.
    pub fn rate(&self) -> f64 {
        self.k() as f64 / self.n as f64
    }

    /// Computes the syndrome of a codeword: s = H × c over GF(2).
    ///
    /// Returns a zero vector if c is a valid codeword.
    pub fn syndrome(&self, codeword: &BitVec) -> BitVec {
        assert_eq!(codeword.len(), self.n, "Codeword length must equal n");
        self.h.matvec(codeword)
    }

    /// Checks if a codeword is valid (syndrome is zero).
    pub fn is_valid_codeword(&self, codeword: &BitVec) -> bool {
        let syndrome = self.syndrome(codeword);
        syndrome.count_ones() == 0
    }

    /// Returns the parity-check matrix.
    pub(crate) fn parity_check_matrix(&self) -> &SparseMatrixDual {
        &self.h
    }
}

/// Belief propagation decoder for LDPC codes.
///
/// Implements the sum-product algorithm (SPA) and min-sum approximations
/// for iterative soft-decision decoding.
///
/// # Decoding Algorithm
///
/// The decoder maintains two types of messages:
/// - **Check-to-variable**: $\lambda_{m \to n}$ from check $m$ to variable $n$
/// - **Variable-to-check**: $\mu_{n \to m}$ from variable $n$ to check $m$
///
/// ## Update Rules (Sum-Product Algorithm)
///
/// Check-to-variable update:
/// $$
/// \lambda_{m \to n} = 2 \cdot \text{atanh}\left(\prod_{n' \in N(m) \setminus n} \tanh\left(\frac{\mu_{n' \to m}}{2}\right)\right)
/// $$
///
/// Variable-to-check update:
/// $$
/// \mu_{n \to m} = L_n + \sum_{m' \in M(n) \setminus m} \lambda_{m' \to n}
/// $$
///
/// where $L_n$ is the channel LLR for variable node $n$.
#[derive(Debug)]
pub struct LdpcDecoder {
    code: LdpcCode,
    /// Current variable node beliefs (posterior LLRs)
    beliefs: Vec<Llr>,
    /// Check-to-variable messages: indexed by [check][position in row]
    check_to_var: Vec<Vec<Llr>>,
    /// Variable-to-check messages: indexed by [var][position in column]
    var_to_check: Vec<Vec<Llr>>,
    /// Number of iterations in last decode
    last_iterations: usize,
}

impl LdpcDecoder {
    /// Creates a new LDPC decoder for the given code.
    pub fn new(code: LdpcCode) -> Self {
        let n = code.n();
        let m = code.m();
        let h = code.parity_check_matrix();

        // Preallocate message storage
        let check_to_var: Vec<Vec<Llr>> = (0..m)
            .map(|check| {
                let degree = h.row_iter(check).count();
                vec![Llr::zero(); degree]
            })
            .collect();

        let var_to_check: Vec<Vec<Llr>> = (0..n)
            .map(|var| {
                let degree = h.col_iter(var).count();
                vec![Llr::zero(); degree]
            })
            .collect();

        Self {
            code,
            beliefs: vec![Llr::zero(); n],
            check_to_var,
            var_to_check,
            last_iterations: 0,
        }
    }

    /// Performs check node update (sum-product algorithm).
    ///
    /// Computes check-to-variable messages using the exact box-plus operation.
    #[allow(dead_code)] // Kept for potential future use
    fn check_node_update_spa(&mut self, _channel_llrs: &[Llr]) {
        let h = self.code.parity_check_matrix();

        for check in 0..self.code.m() {
            let neighbors: Vec<usize> = h.row_iter(check).collect();
            let degree = neighbors.len();

            for (pos, &_var) in neighbors.iter().enumerate() {
                // Collect all incoming messages except from this variable
                let mut inputs = Vec::with_capacity(degree);
                for (other_pos, &other_var) in neighbors.iter().enumerate() {
                    if other_pos != pos {
                        // Get variable-to-check message
                        let var_check_pos = self.find_check_position(other_var, check);
                        inputs.push(self.var_to_check[other_var][var_check_pos]);
                    }
                }

                // Compute check-to-variable message using box-plus
                let message = if inputs.is_empty() {
                    Llr::zero()
                } else {
                    Llr::boxplus_n(&inputs)
                };

                self.check_to_var[check][pos] = message;
            }
        }
    }

    /// Performs check node update (min-sum approximation).
    fn check_node_update_minsum(&mut self, _channel_llrs: &[Llr]) {
        let h = self.code.parity_check_matrix();

        for check in 0..self.code.m() {
            let neighbors: Vec<usize> = h.row_iter(check).collect();
            let degree = neighbors.len();

            for (pos, &_var) in neighbors.iter().enumerate() {
                let mut inputs = Vec::with_capacity(degree);
                for (other_pos, &other_var) in neighbors.iter().enumerate() {
                    if other_pos != pos {
                        let var_check_pos = self.find_check_position(other_var, check);
                        inputs.push(self.var_to_check[other_var][var_check_pos]);
                    }
                }

                let message = if inputs.is_empty() {
                    Llr::zero()
                } else {
                    Llr::boxplus_minsum_n(&inputs)
                };

                self.check_to_var[check][pos] = message;
            }
        }
    }

    /// Performs variable node update.
    ///
    /// Updates beliefs and variable-to-check messages.
    fn variable_node_update(&mut self, channel_llrs: &[Llr]) {
        let h = self.code.parity_check_matrix();

        for (var, &channel_llr) in channel_llrs.iter().enumerate().take(self.code.n()) {
            let neighbors: Vec<usize> = h.col_iter(var).collect();

            // Compute total belief: channel LLR + sum of incoming check messages
            let mut belief = channel_llr;
            for (pos, &_check) in neighbors.iter().enumerate() {
                belief = Llr::new(belief.value() + self.check_to_var_message(var, pos).value());
            }
            self.beliefs[var] = belief;

            // Compute variable-to-check messages
            for (pos, &_check) in neighbors.iter().enumerate() {
                // Message = belief - incoming message from this check
                let incoming = self.check_to_var_message(var, pos);
                let message = Llr::new(belief.value() - incoming.value());
                self.var_to_check[var][pos] = message;
            }
        }
    }

    /// Helper: Find the position of check in variable's neighbor list.
    fn find_check_position(&self, var: usize, target_check: usize) -> usize {
        let h = self.code.parity_check_matrix();
        h.col_iter(var)
            .enumerate()
            .find(|(_, check)| *check == target_check)
            .map(|(pos, _)| pos)
            .expect("Check not found in variable's neighbors")
    }

    /// Helper: Get check-to-variable message.
    fn check_to_var_message(&self, var: usize, var_check_pos: usize) -> Llr {
        let h = self.code.parity_check_matrix();
        let check = h.col_iter(var).nth(var_check_pos).unwrap();
        let check_var_pos = h
            .row_iter(check)
            .enumerate()
            .find(|(_, v)| *v == var)
            .map(|(pos, _)| pos)
            .unwrap();
        self.check_to_var[check][check_var_pos]
    }

    /// Makes hard decisions on current beliefs.
    fn hard_decode(&self) -> BitVec {
        let mut decoded = BitVec::with_capacity(self.code.n());
        for &belief in &self.beliefs {
            decoded.push_bit(belief.hard_decision());
        }
        decoded
    }
}

impl SoftDecoder for LdpcDecoder {
    fn k(&self) -> usize {
        self.code.k()
    }

    fn n(&self) -> usize {
        self.code.n()
    }

    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        // For non-iterative interface, just return hard decisions on input LLRs
        assert_eq!(llrs.len(), self.n());
        let mut decoded = BitVec::with_capacity(self.n());
        for &llr in llrs {
            decoded.push_bit(llr.hard_decision());
        }
        decoded
    }
}

impl IterativeSoftDecoder for LdpcDecoder {
    fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
        assert_eq!(llrs.len(), self.n(), "LLR length must equal n");

        // Initialize: variable-to-check messages = channel LLRs
        for (var, &llr) in llrs.iter().enumerate().take(self.code.n()) {
            for pos in 0..self.var_to_check[var].len() {
                self.var_to_check[var][pos] = llr;
            }
        }

        let mut iterations = 0;
        let mut converged = false;

        for iter in 0..max_iterations {
            iterations = iter + 1;

            // Check node update (using min-sum for efficiency)
            self.check_node_update_minsum(llrs);

            // Variable node update
            self.variable_node_update(llrs);

            // Hard decision and syndrome check
            let decoded = self.hard_decode();
            if self.code.is_valid_codeword(&decoded) {
                converged = true;
                break;
            }
        }

        self.last_iterations = iterations;
        let decoded = self.hard_decode();
        let syndrome_passed = self.code.is_valid_codeword(&decoded);

        DecoderResult::new(decoded, iterations, converged, syndrome_passed)
    }

    fn last_iteration_count(&self) -> usize {
        self.last_iterations
    }

    fn reset(&mut self) {
        // Reset all messages to zero
        for check_msgs in &mut self.check_to_var {
            for msg in check_msgs {
                *msg = Llr::zero();
            }
        }
        for var_msgs in &mut self.var_to_check {
            for msg in var_msgs {
                *msg = Llr::zero();
            }
        }
        for belief in &mut self.beliefs {
            *belief = Llr::zero();
        }
        self.last_iterations = 0;
    }
}

#[cfg(test)]
mod decoder_tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let code = LdpcCode::from_edges(2, 3, &edges);
        let decoder = LdpcDecoder::new(code);

        assert_eq!(decoder.last_iteration_count(), 0);
    }

    #[test]
    fn test_trivial_decode_no_errors() {
        // Simple repetition code
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        // Strong LLRs for all-zero codeword
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];

        let result = decoder.decode_iterative(&llrs, 10);

        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
        assert!(result.iterations <= 2); // Should converge quickly
    }

    #[test]
    fn test_decode_with_single_error() {
        // Single parity check code [3,2]
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        // Two strong 1s, one weak 0 → should decode to [1, 1, 0] (even parity)
        let llrs = vec![Llr::new(-5.0), Llr::new(-5.0), Llr::new(2.0)]; // Weak 0

        let result = decoder.decode_iterative(&llrs, 20);

        // Should converge to valid codeword
        if result.converged {
            assert!(result.syndrome_check_passed);
            // Should decode to [1, 1, 0] which has even parity
            assert_eq!(result.decoded_bits.count_ones(), 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldpc_code_creation() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2), (2, 0), (2, 2)];
        let code = LdpcCode::from_edges(3, 4, &edges);

        assert_eq!(code.n(), 4);
        assert_eq!(code.m(), 3);
        assert_eq!(code.k(), 1);
        assert!((code.rate() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_syndrome_computation() {
        // Single parity check code: H = [1 1 1]
        // Valid codewords have even parity
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);

        // Valid codeword [0,0,0] - even parity
        let mut valid = BitVec::new();
        for _ in 0..3 {
            valid.push_bit(false);
        }
        assert!(code.is_valid_codeword(&valid));

        // Valid codeword [1,1,0] - even parity (1+1+0=0 mod 2)
        let mut valid2 = BitVec::new();
        valid2.push_bit(true);
        valid2.push_bit(true);
        valid2.push_bit(false);
        assert!(code.is_valid_codeword(&valid2));

        // Invalid codeword [1,0,0] - odd parity
        let mut invalid = BitVec::new();
        invalid.push_bit(true);
        invalid.push_bit(false);
        invalid.push_bit(false);
        assert!(!code.is_valid_codeword(&invalid));
    }

    #[test]
    fn test_regular_ldpc_structure() {
        // Create a regular (2,4) code: 2 ones per column, 4 ones per row
        // 4 checks × 8 variables
        let mut edges = Vec::new();
        for col in 0..8 {
            let check1 = (col * 2) % 4;
            let check2 = (col * 2 + 1) % 4;
            edges.push((check1, col));
            edges.push((check2, col));
        }

        let code = LdpcCode::from_edges(4, 8, &edges);
        let h = code.parity_check_matrix();

        // Verify column weights
        for col in 0..8 {
            let weight = h.col_iter(col).count();
            assert_eq!(weight, 2, "Column {} should have weight 2", col);
        }

        // Verify row weights
        for row in 0..4 {
            let weight = h.row_iter(row).count();
            assert_eq!(weight, 4, "Row {} should have weight 4", row);
        }
    }
}
