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
    /// Cached generator matrix (computed lazily)
    #[allow(clippy::type_complexity)]
    cached_generator: std::sync::Arc<std::sync::Mutex<Option<gf2_core::BitMatrix>>>,
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
        Self {
            h,
            n,
            m,
            cached_generator: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
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

    /// Creates an LDPC code from a quasi-cyclic structure.
    ///
    /// Quasi-cyclic (QC) LDPC codes have parity-check matrices composed of
    /// circulant submatrices. This structure is used in standards like DVB-T2,
    /// 5G NR, and WiFi 802.11n.
    ///
    /// # Arguments
    ///
    /// * `qc` - Quasi-cyclic structure with base matrix and expansion factor
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
    ///
    /// // Simple 2×2 base matrix with 3×3 circulant blocks
    /// let base_matrix = vec![vec![0, 1], vec![1, 0]];
    /// let qc = QuasiCyclicLdpc::new(base_matrix, 3);
    /// let code = LdpcCode::from_quasi_cyclic(&qc);
    ///
    /// assert_eq!(code.m(), 6); // 2 base rows × 3
    /// assert_eq!(code.n(), 6); // 2 base cols × 3
    /// ```
    pub fn from_quasi_cyclic(qc: &QuasiCyclicLdpc) -> Self {
        let edges = qc.to_edges();
        let m = qc.expanded_rows();
        let n = qc.expanded_cols();
        Self::from_edges(m, n, &edges)
    }

    /// Computes the generator matrix from the parity-check matrix.
    ///
    /// Uses Gaussian elimination to convert H to systematic form [P^T | I_m],
    /// then constructs G = [I_k | P] where k = n - m.
    ///
    /// This is expensive (O(m²·k)) and cached after first computation.
    ///
    /// Returns None if H is not full rank.
    fn compute_generator_matrix(&self) -> Option<gf2_core::BitMatrix> {
        use gf2_core::BitMatrix;

        let k = self.k();
        let m = self.m;

        if k == 0 {
            return Some(BitMatrix::zeros(0, self.n));
        }

        // Convert sparse H to dense for Gaussian elimination
        let mut h_dense = self.h.to_dense();

        // Perform Gaussian elimination to get H in systematic form
        // We want to transform H to [P^T | I_m] form
        // This requires column permutations to move linearly independent columns to the right

        // Forward elimination with column selection
        let mut col_permutation = Vec::with_capacity(self.n);
        let mut used_cols = vec![false; self.n];

        for row in 0..m {
            // Find a pivot column (any unused column with 1 in this row)
            let pivot_col = (0..self.n).find(|&col| !used_cols[col] && h_dense.get(row, col));

            // No pivot found - matrix is rank deficient
            let pivot_col = pivot_col?;
            used_cols[pivot_col] = true;
            col_permutation.push(pivot_col);

            // Eliminate this column from all other rows
            for other_row in 0..m {
                if other_row != row && h_dense.get(other_row, pivot_col) {
                    // XOR row into other_row
                    for c in 0..self.n {
                        let val = h_dense.get(row, c) ^ h_dense.get(other_row, c);
                        h_dense.set(other_row, c, val);
                    }
                }
            }
        }

        // H is now in row echelon form with col_permutation defining the systematic positions
        // Extract systematic (information) bit positions (unused columns)
        let systematic_positions: Vec<usize> = used_cols
            .iter()
            .enumerate()
            .filter_map(|(col, &used)| if !used { Some(col) } else { None })
            .collect();

        assert_eq!(
            systematic_positions.len(),
            k,
            "Should have k systematic positions"
        );

        // Build generator matrix G (k × n)
        // For systematic codes: G = [I_k | P]
        // where P is derived from the parity part of H
        let mut g = BitMatrix::zeros(k, self.n);

        // Set identity part (systematic positions)
        for (i, &sys_col) in systematic_positions.iter().enumerate() {
            g.set(i, sys_col, true);
        }

        // Set parity part
        // For each systematic bit position, we need to find which parity checks it affects
        for (msg_idx, &sys_col) in systematic_positions.iter().enumerate() {
            for (check_idx, &parity_col) in col_permutation.iter().enumerate() {
                if h_dense.get(check_idx, sys_col) {
                    // This systematic bit affects this parity check
                    g.set(msg_idx, parity_col, true);
                }
            }
        }

        Some(g)
    }
}

impl crate::traits::GeneratorMatrixAccess for LdpcCode {
    fn k(&self) -> usize {
        self.k()
    }

    fn n(&self) -> usize {
        self.n
    }

    fn generator_matrix(&self) -> gf2_core::BitMatrix {
        let mut cache = self.cached_generator.lock().unwrap();
        if let Some(ref g) = *cache {
            g.clone()
        } else {
            let g = self
                .compute_generator_matrix()
                .expect("LDPC parity-check matrix is not full rank");
            *cache = Some(g.clone());
            g
        }
    }

    fn is_systematic(&self) -> bool {
        // LDPC codes are not naturally systematic unless specially constructed
        // We'd need to analyze the generator matrix to determine this
        // For now, return false conservatively
        false
    }
}

/// A circulant matrix for quasi-cyclic LDPC codes.
///
/// A circulant matrix is a square matrix where each row is a right-shifted
/// version of the previous row. In QC-LDPC codes, circulants are used as
/// building blocks for the parity-check matrix.
///
/// # Structure
///
/// For a Z×Z circulant with shift s, the first row has a single 1 in column s,
/// and each subsequent row shifts right by one position (with wraparound).
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::CirculantMatrix;
///
/// // Identity circulant (shift 0, size 3):
/// // [1 0 0]
/// // [0 1 0]
/// // [0 0 1]
/// let identity = CirculantMatrix::new(0, 3);
///
/// // Shift-1 circulant:
/// // [0 1 0]
/// // [0 0 1]
/// // [1 0 0]
/// let shift1 = CirculantMatrix::new(1, 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CirculantMatrix {
    /// Right-shift amount (0 = identity)
    shift: usize,
    /// Size of the circulant (Z×Z)
    size: usize,
}

impl CirculantMatrix {
    /// Creates a new circulant matrix.
    ///
    /// # Arguments
    ///
    /// * `shift` - Right-shift amount (must be < size)
    /// * `size` - Dimension of the square circulant matrix
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::CirculantMatrix;
    ///
    /// let circ = CirculantMatrix::new(2, 5);
    /// assert_eq!(circ.shift(), 2);
    /// assert_eq!(circ.size(), 5);
    /// ```
    pub fn new(shift: usize, size: usize) -> Self {
        Self { shift, size }
    }

    /// Returns the shift value.
    pub fn shift(&self) -> usize {
        self.shift
    }

    /// Returns the size of the circulant.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Generates edges (row, col) for this circulant block in a larger matrix.
    ///
    /// # Arguments
    ///
    /// * `base_row` - Base row index in the base matrix
    /// * `base_col` - Base column index in the base matrix
    ///
    /// # Returns
    ///
    /// Vector of (row, col) edges representing the circulant's 1-positions
    pub fn to_edges(&self, base_row: usize, base_col: usize) -> Vec<(usize, usize)> {
        let row_offset = base_row * self.size;
        let col_offset = base_col * self.size;

        (0..self.size)
            .map(|i| {
                let row = row_offset + i;
                let col = col_offset + ((i + self.shift) % self.size);
                (row, col)
            })
            .collect()
    }
}

/// Quasi-cyclic LDPC code structure.
///
/// QC-LDPC codes have parity-check matrices composed of circulant submatrices
/// arranged according to a base matrix. This structure enables efficient
/// encoding/decoding and is used in modern communication standards.
///
/// # Structure
///
/// - **Base matrix**: mb × nb matrix of shift values
/// - **Expansion factor** Z: Size of each circulant block
/// - **Expanded matrix**: (mb·Z) × (nb·Z) parity-check matrix H
///
/// Each entry in the base matrix:
/// - **-1**: Zero block (all zeros)
/// - **0 to Z-1**: Circulant block with corresponding shift
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
///
/// // DVB-T2-like structure (simplified)
/// let base_matrix = vec![
///     vec![0, 1, 2],
///     vec![1, 0, -1],  // -1 = zero block
/// ];
/// let expansion_factor = 360;
///
/// let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
/// let code = LdpcCode::from_quasi_cyclic(&qc);
///
/// assert_eq!(code.m(), 2 * 360);
/// assert_eq!(code.n(), 3 * 360);
/// ```
#[derive(Debug, Clone)]
pub struct QuasiCyclicLdpc {
    /// Base matrix with shift values (-1 = zero block, 0..Z-1 = circulant shift)
    base_matrix: Vec<Vec<i32>>,
    /// Expansion factor (circulant size)
    expansion_factor: usize,
}

impl QuasiCyclicLdpc {
    /// Creates a new quasi-cyclic LDPC structure.
    ///
    /// # Arguments
    ///
    /// * `base_matrix` - Matrix of shift values (-1 for zero blocks)
    /// * `expansion_factor` - Size Z of each circulant block
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - Base matrix is empty
    /// - Rows have inconsistent lengths
    /// - Expansion factor is zero
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let base_matrix = vec![
    ///     vec![0, 1, -1],
    ///     vec![2, -1, 0],
    /// ];
    /// let qc = QuasiCyclicLdpc::new(base_matrix, 4);
    ///
    /// assert_eq!(qc.base_rows(), 2);
    /// assert_eq!(qc.base_cols(), 3);
    /// assert_eq!(qc.expansion_factor(), 4);
    /// ```
    pub fn new(base_matrix: Vec<Vec<i32>>, expansion_factor: usize) -> Self {
        assert!(
            !base_matrix.is_empty(),
            "Base matrix must have at least one row"
        );
        assert!(expansion_factor > 0, "Expansion factor must be positive");

        let cols = base_matrix[0].len();
        assert!(
            base_matrix.iter().all(|row| row.len() == cols),
            "All rows in base matrix must have the same length"
        );

        Self {
            base_matrix,
            expansion_factor,
        }
    }

    /// Returns the number of rows in the base matrix.
    pub fn base_rows(&self) -> usize {
        self.base_matrix.len()
    }

    /// Returns the number of columns in the base matrix.
    pub fn base_cols(&self) -> usize {
        self.base_matrix[0].len()
    }

    /// Returns the expansion factor.
    pub fn expansion_factor(&self) -> usize {
        self.expansion_factor
    }

    /// Returns the number of rows in the expanded matrix.
    pub fn expanded_rows(&self) -> usize {
        self.base_rows() * self.expansion_factor
    }

    /// Returns the number of columns in the expanded matrix.
    pub fn expanded_cols(&self) -> usize {
        self.base_cols() * self.expansion_factor
    }

    /// Expands the quasi-cyclic structure to a list of edges.
    ///
    /// Converts the base matrix with circulant blocks into a sparse edge list
    /// suitable for creating an LDPC code.
    ///
    /// # Returns
    ///
    /// Vector of (row, col) edges representing 1-positions in the expanded matrix
    ///
    /// # Panics
    ///
    /// Panics if any shift value is invalid (not -1 and not in range 0..Z)
    pub fn to_edges(&self) -> Vec<(usize, usize)> {
        let z = self.expansion_factor;
        let mut edges = Vec::new();

        for (base_row, row) in self.base_matrix.iter().enumerate() {
            for (base_col, &shift) in row.iter().enumerate() {
                if shift == -1 {
                    // Zero block - no edges
                    continue;
                }

                assert!(
                    shift >= 0 && (shift as usize) < z,
                    "Shift value {} at position ({},{}) must be -1 or in range [0,{})",
                    shift,
                    base_row,
                    base_col,
                    z
                );

                let circ = CirculantMatrix::new(shift as usize, z);
                let block_edges = circ.to_edges(base_row, base_col);
                edges.extend(block_edges);
            }
        }

        edges
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

#[cfg(test)]
mod generator_matrix_access_tests {
    use super::*;
    use crate::traits::GeneratorMatrixAccess;

    #[test]
    fn test_ldpc_generator_matrix_dimensions() {
        // Small Hamming(7,4) as LDPC
        let edges = vec![
            (0, 0),
            (0, 1),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 4),
            (2, 1),
            (2, 2),
            (2, 5),
        ];
        let code = LdpcCode::from_edges(3, 7, &edges);
        let g = code.generator_matrix();
        assert_eq!(g.rows(), 4); // k = n - m = 7 - 3
        assert_eq!(g.cols(), 7);
    }

    #[test]
    fn test_ldpc_generator_parity_orthogonality() {
        // Small hand-constructed example with full-rank H
        let edges = vec![(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (1, 3)];
        let code = LdpcCode::from_edges(2, 4, &edges);
        let g = code.generator_matrix();
        let h = code.parity_check_matrix().to_dense();

        // Verify G·H^T = 0
        let h_t = h.transpose();
        let product = &g * &h_t;

        for i in 0..product.rows() {
            for j in 0..product.cols() {
                assert!(!product.get(i, j), "G·H^T must be zero at ({}, {})", i, j);
            }
        }
    }

    #[test]
    fn test_ldpc_encoding_via_generator_produces_valid_codewords() {
        let edges = vec![
            (0, 0),
            (0, 1),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 4),
            (2, 1),
            (2, 2),
            (2, 5),
        ];
        let code = LdpcCode::from_edges(3, 7, &edges);
        let g = code.generator_matrix();

        // Each row of G should be a valid codeword
        for i in 0..code.k() {
            let mut row = BitVec::new();
            for j in 0..code.n() {
                row.push_bit(g.get(i, j));
            }
            assert!(
                code.is_valid_codeword(&row),
                "Row {} of G must be a valid codeword",
                i
            );
        }
    }

    #[test]
    fn test_ldpc_generator_cached() {
        // Use a full-rank example
        let edges = vec![
            (0, 0),
            (0, 1),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 4),
            (2, 1),
            (2, 2),
            (2, 5),
        ];
        let code = LdpcCode::from_edges(3, 7, &edges);
        let g1 = code.generator_matrix();
        let g2 = code.generator_matrix();
        assert_eq!(g1, g2);
    }

    #[test]
    fn test_ldpc_small_identity_h() {
        // Simple test with H = [I_2 | P]
        // H = [1 0 | 1 1]
        //     [0 1 | 1 0]
        // Then G = [1 1 | 1 0]
        //          [1 0 | 0 1]
        let edges = vec![
            (0, 0),
            (0, 2),
            (0, 3), // First check
            (1, 1),
            (1, 2), // Second check
        ];
        let code = LdpcCode::from_edges(2, 4, &edges);
        let g = code.generator_matrix();

        assert_eq!(g.rows(), 2); // k = 4 - 2 = 2
        assert_eq!(g.cols(), 4);

        // Verify it's a valid generator (all rows are codewords)
        for i in 0..code.k() {
            let mut row = BitVec::new();
            for j in 0..code.n() {
                row.push_bit(g.get(i, j));
            }
            assert!(code.is_valid_codeword(&row));
        }
    }

    #[test]
    fn test_ldpc_regular_3_6() {
        // Regular (3,6) LDPC code - small version
        let edges = vec![
            // Each variable node connects to 3 checks
            (0, 0),
            (1, 0),
            (2, 0), // v0
            (0, 1),
            (1, 1),
            (3, 1), // v1
            (0, 2),
            (2, 2),
            (3, 2), // v2
            (1, 3),
            (2, 3),
            (3, 3), // v3
            (0, 4),
            (2, 4),
            (3, 4), // v4
            (1, 5),
            (2, 5),
            (3, 5), // v5
        ];
        let code = LdpcCode::from_edges(4, 6, &edges);
        let g = code.generator_matrix();

        assert_eq!(g.rows(), 2); // k = 6 - 4 = 2
        assert_eq!(g.cols(), 6);

        // Verify orthogonality
        let h = code.parity_check_matrix().to_dense();
        let h_t = h.transpose();
        let product = &g * &h_t;

        for i in 0..product.rows() {
            for j in 0..product.cols() {
                assert!(!product.get(i, j), "G·H^T must be zero");
            }
        }
    }
}
