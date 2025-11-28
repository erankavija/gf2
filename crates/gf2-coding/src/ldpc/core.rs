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
use gf2_core::sparse::SpBitMatrixDual;
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
    h: SpBitMatrixDual,
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
        let h = SpBitMatrixDual::from_coo(m, n, edges);
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
    pub(crate) fn parity_check_matrix(&self) -> &SpBitMatrixDual {
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

    /// Creates a DVB-T2 short frame LDPC code.
    ///
    /// Short frames have n=16200 bits with expansion factor Z=360.
    ///
    /// # Arguments
    ///
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::LdpcCode;
    /// use gf2_coding::CodeRate;
    ///
    /// let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    /// assert_eq!(code.n(), 16200);
    /// assert_eq!(code.k(), 7200);
    /// ```
    ///
    /// # References
    ///
    /// ETSI EN 302 755 V1.4.1 (DVB-T2 standard)
    pub fn dvb_t2_short(rate: crate::bch::CodeRate) -> Self {
        use crate::ldpc::dvb_t2::{builder, dvb_t2_matrices, params};

        let params = params::DvbParams::for_code(params::FrameSize::Short, rate);
        let table = match rate {
            crate::bch::CodeRate::Rate1_2 => dvb_t2_matrices::SHORT_RATE_1_2_TABLE,
            crate::bch::CodeRate::Rate3_5 => dvb_t2_matrices::SHORT_RATE_3_5_TABLE,
            crate::bch::CodeRate::Rate2_3 => dvb_t2_matrices::SHORT_RATE_2_3_TABLE,
            crate::bch::CodeRate::Rate3_4 => dvb_t2_matrices::SHORT_RATE_3_4_TABLE,
            crate::bch::CodeRate::Rate4_5 => dvb_t2_matrices::SHORT_RATE_4_5_TABLE,
            crate::bch::CodeRate::Rate5_6 => dvb_t2_matrices::SHORT_RATE_5_6_TABLE,
        };

        let edges = builder::build_dvb_edges(table, &params);
        Self::from_edges(params.m, params.n, &edges)
    }

    /// Creates a DVB-T2 normal frame LDPC code.
    ///
    /// Normal frames have n=64800 bits with expansion factor Z=360.
    ///
    /// # Arguments
    ///
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::LdpcCode;
    /// use gf2_coding::CodeRate;
    ///
    /// let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    /// assert_eq!(code.n(), 64800);
    /// assert_eq!(code.k(), 32400);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the table for the requested rate is not yet implemented.
    ///
    /// # References
    ///
    /// ETSI EN 302 755 V1.4.1 (DVB-T2 standard)
    pub fn dvb_t2_normal(rate: crate::bch::CodeRate) -> Self {
        use crate::ldpc::dvb_t2::{builder, dvb_t2_matrices, params};

        let params = params::DvbParams::for_code(params::FrameSize::Normal, rate);
        let table = match rate {
            crate::bch::CodeRate::Rate1_2 => dvb_t2_matrices::NORMAL_RATE_1_2_TABLE,
            crate::bch::CodeRate::Rate3_5 => dvb_t2_matrices::NORMAL_RATE_3_5_TABLE,
            crate::bch::CodeRate::Rate2_3 => dvb_t2_matrices::NORMAL_RATE_2_3_TABLE,
            crate::bch::CodeRate::Rate3_4 => dvb_t2_matrices::NORMAL_RATE_3_4_TABLE,
            crate::bch::CodeRate::Rate4_5 => dvb_t2_matrices::NORMAL_RATE_4_5_TABLE,
            crate::bch::CodeRate::Rate5_6 => dvb_t2_matrices::NORMAL_RATE_5_6_TABLE,
        };

        let edges = builder::build_dvb_edges(table, &params);
        Self::from_edges(params.m, params.n, &edges)
    }

    /// Computes the generator matrix from the parity-check matrix.
    ///
    /// Uses RREF (Reduced Row Echelon Form) from gf2-core to convert H to systematic
    /// form [P^T | I_m], then constructs G = [I_k | P] where k = n - m.
    ///
    /// Uses optimized word-level operations with SIMD acceleration when available.
    /// Cached after first computation.
    ///
    /// Returns None if H is not full rank.
    fn compute_generator_matrix(&self) -> Option<gf2_core::BitMatrix> {
        use gf2_core::alg::rref::rref;
        use gf2_core::BitMatrix;

        let k = self.k();
        let m = self.m;

        if k == 0 {
            return Some(BitMatrix::zeros(0, self.n));
        }

        // Convert sparse H to dense for RREF
        let h_dense = self.h.to_dense();

        // Use gf2-core's optimized RREF with word-level operations and SIMD acceleration
        // pivot_from_right=false for left-to-right pivoting (standard order)
        let rref_result = rref(&h_dense, false);

        if rref_result.rank != m {
            return None; // Matrix is rank deficient
        }

        let h_dense = rref_result.reduced;
        let col_permutation = rref_result.pivot_cols;

        // Extract systematic (information) bit positions (non-pivot columns)
        let all_cols: Vec<usize> = (0..self.n).collect();
        let systematic_positions: Vec<usize> = all_cols
            .into_iter()
            .filter(|c| !col_permutation.contains(c))
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

    /// Decodes multiple LLR blocks in batch (parallel).
    ///
    /// Each block is decoded independently using thread-local decoders.
    /// Uses rayon for parallel decoding across CPU cores.
    ///
    /// # Performance
    ///
    /// Expected: 4-8× speedup on 8-core CPU for batches > 10 blocks.
    ///
    /// # Thread Safety
    ///
    /// Each parallel task creates its own decoder instance. The code parameter
    /// is cloned (cheap - uses Arc internally for large matrices).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
    /// use gf2_coding::llr::Llr;
    ///
    /// let edges = vec![(0, 0), (0, 1), (0, 2)];
    /// let code = LdpcCode::from_edges(1, 3, &edges);
    ///
    /// let llr_blocks: Vec<Vec<Llr>> = (0..100)
    ///     .map(|_| vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)])
    ///     .collect();
    ///
    /// let results = LdpcDecoder::decode_batch(&code, &llr_blocks, 10);
    /// assert_eq!(results.len(), 100);
    /// ```
    pub fn decode_batch(
        code: &LdpcCode,
        llr_blocks: &[Vec<Llr>],
        max_iterations: usize,
    ) -> Vec<DecoderResult> {
        use rayon::prelude::*;

        (0..llr_blocks.len())
            .into_par_iter()
            .map(|i| {
                let mut decoder = Self::new(code.clone());
                decoder.decode_iterative(&llr_blocks[i], max_iterations)
            })
            .collect()
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

        // Reset all messages to ensure clean state
        for check_msgs in &mut self.check_to_var {
            for msg in check_msgs {
                *msg = Llr::zero();
            }
        }

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
        let decoded_codeword = self.hard_decode();
        let syndrome_passed = self.code.is_valid_codeword(&decoded_codeword);

        // Extract message bits from systematic codeword [message | parity]
        let k = self.code.k();
        let mut message = BitVec::with_capacity(k);
        for i in 0..k {
            message.push_bit(decoded_codeword.get(i));
        }

        DecoderResult::new(message, iterations, converged, syndrome_passed)
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

/// LDPC encoder for systematic encoding.
///
/// Encodes messages into systematic LDPC codewords: [message | parity].
/// Uses Richardson-Urbanke preprocessing for efficient encoding.
///
/// # Examples
///
/// ```no_run
/// use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_coding::CodeRate;
/// use gf2_core::BitVec;
///
/// let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
/// let encoder = LdpcEncoder::new(code);
///
/// let message = BitVec::zeros(encoder.k());
/// let codeword = encoder.encode(&message);
///
/// assert_eq!(codeword.len(), encoder.n());
/// ```
pub struct LdpcEncoder {
    code: LdpcCode,
    encoding_matrices: std::sync::Arc<crate::ldpc::encoding::RuEncodingMatrices>,
}

impl LdpcEncoder {
    /// Creates a new LDPC encoder WITHOUT cache.
    ///
    /// Preprocesses the parity-check matrix for efficient encoding.
    /// This operation is expensive (2-10 seconds for DVB-T2 codes) but
    /// done only once per encoder instance.
    ///
    /// For faster encoder creation when working with multiple encoders
    /// of the same configuration, use [`LdpcEncoder::with_cache`].
    ///
    /// # Panics
    ///
    /// Panics if the parity-check matrix preprocessing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
    /// use gf2_coding::CodeRate;
    ///
    /// let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    /// let encoder = LdpcEncoder::new(code);
    /// // Takes 2-3 seconds, but no cache needed
    /// ```
    pub fn new(code: LdpcCode) -> Self {
        let encoding_matrices =
            crate::ldpc::encoding::RuEncodingMatrices::preprocess(code.parity_check_matrix())
                .expect("Failed to preprocess LDPC code for encoding");

        Self {
            code,
            encoding_matrices: std::sync::Arc::new(encoding_matrices),
        }
    }

    /// Creates a new LDPC encoder WITH cache (opt-in performance boost).
    ///
    /// Uses the provided cache to avoid expensive preprocessing when creating
    /// multiple encoders for the same LDPC code configuration.
    ///
    /// - First call: preprocesses and caches (2-10 seconds)
    /// - Subsequent calls: instant (<1μs)
    ///
    /// # Arguments
    ///
    /// * `code` - LDPC code to encode with
    /// * `cache` - Encoding cache to use
    ///
    /// # Panics
    ///
    /// Panics if the parity-check matrix preprocessing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
    /// use gf2_coding::ldpc::encoding::EncodingCache;
    /// use gf2_coding::CodeRate;
    ///
    /// let cache = EncodingCache::new();
    /// let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    ///
    /// // First call: slow but caches
    /// let enc1 = LdpcEncoder::with_cache(code.clone(), &cache);
    ///
    /// // Second call: instant
    /// let enc2 = LdpcEncoder::with_cache(code, &cache);
    /// ```
    pub fn with_cache(code: LdpcCode, cache: &crate::ldpc::encoding::EncodingCache) -> Self {
        let key = crate::ldpc::encoding::CacheKey::from_params(
            code.n(),
            code.k(),
            code.parity_check_matrix(),
        );

        let encoding_matrices = cache
            .get_or_compute(key, code.parity_check_matrix())
            .expect("Failed to preprocess LDPC code for encoding");

        Self {
            code,
            encoding_matrices,
        }
    }
}

impl LdpcEncoder {
    /// Encodes multiple messages in batch.
    ///
    /// Currently sequential. Parallel version coming soon once Sync bounds resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let edges = vec![(0, 0), (0, 1), (0, 2)];
    /// let code = LdpcCode::from_edges(1, 3, &edges);
    /// let encoder = LdpcEncoder::new(code);
    ///
    /// let messages: Vec<BitVec> = (0..100)
    ///     .map(|_| BitVec::from_bytes_le(&[0b00]))
    ///     .collect();
    /// let codewords = encoder.encode_batch(&messages);
    /// assert_eq!(codewords.len(), 100);
    /// ```
    pub fn encode_batch(&self, messages: &[BitVec]) -> Vec<BitVec> {
        use crate::traits::BlockEncoder;
        messages.iter().map(|msg| self.encode(msg)).collect()
    }
}

impl crate::traits::BlockEncoder for LdpcEncoder {
    fn k(&self) -> usize {
        self.code.k()
    }

    fn n(&self) -> usize {
        self.code.n()
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.k(),
            "Message length {} must equal k = {}",
            message.len(),
            self.k()
        );

        self.encoding_matrices.encode(message)
    }
}

#[cfg(test)]
mod decoder_tests {
    use super::*;
    use crate::traits::BlockEncoder;

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

    #[test]
    fn test_consecutive_decodes_no_state_leakage() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        // First decode: all-zero codeword (even parity: 0+0+0=0)
        let llrs1 = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result1 = decoder.decode_iterative(&llrs1, 10);
        assert!(result1.converged);
        assert!(result1.syndrome_check_passed);
        assert_eq!(result1.decoded_bits.count_ones(), 0);

        // Second decode: [1,1,0] codeword (even parity: 1+1+0=0)
        let llrs2 = vec![Llr::new(-10.0), Llr::new(-10.0), Llr::new(10.0)];
        let result2 = decoder.decode_iterative(&llrs2, 10);
        assert!(result2.converged);
        assert!(result2.syndrome_check_passed);
        assert_eq!(result2.decoded_bits.count_ones(), 2);

        // Third decode: back to all-zero
        let result3 = decoder.decode_iterative(&llrs1, 10);
        assert!(result3.converged);
        assert!(result3.syndrome_check_passed);
        assert_eq!(result3.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_decoder_reset_clears_state() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        // Decode once
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result1 = decoder.decode_iterative(&llrs, 10);
        assert!(result1.iterations > 0);

        // Reset should clear iteration count
        decoder.reset();
        assert_eq!(decoder.last_iteration_count(), 0);

        // Decode again - should work correctly
        let result2 = decoder.decode_iterative(&llrs, 10);
        assert!(result2.converged);
        assert_eq!(result2.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_encoder_batch_processing() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let encoder = LdpcEncoder::new(code.clone());

        // Create test messages
        let messages: Vec<BitVec> = vec![
            BitVec::from_bytes_le(&[0b00]),
            BitVec::from_bytes_le(&[0b01]),
            BitVec::from_bytes_le(&[0b10]),
            BitVec::from_bytes_le(&[0b11]),
        ]
        .into_iter()
        .map(|bv| {
            let mut msg = BitVec::with_capacity(2);
            msg.push_bit(bv.get(0));
            msg.push_bit(bv.get(1));
            msg
        })
        .collect();

        // Batch encode
        let codewords = encoder.encode_batch(&messages);

        // Verify batch results match individual encodes
        assert_eq!(codewords.len(), 4);
        for (msg, cw) in messages.iter().zip(codewords.iter()) {
            let expected = encoder.encode(msg);
            assert_eq!(cw.len(), expected.len());
            for i in 0..cw.len() {
                assert_eq!(cw.get(i), expected.get(i));
            }
        }
    }

    #[test]
    fn test_decoder_batch_processing() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);

        // Create test LLR blocks (all-zero and [1,1,0] codewords)
        let llr_blocks: Vec<Vec<Llr>> = vec![
            vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)], // [0,0,0]
            vec![Llr::new(-10.0), Llr::new(-10.0), Llr::new(10.0)], // [1,1,0]
            vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)], // [0,0,0]
        ];

        // Batch decode
        let results = LdpcDecoder::decode_batch(&code, &llr_blocks, 10);

        // Verify batch results
        assert_eq!(results.len(), 3);
        assert!(results[0].converged);
        assert_eq!(results[0].decoded_bits.count_ones(), 0);
        assert!(results[1].converged);
        assert_eq!(results[1].decoded_bits.count_ones(), 2);
        assert!(results[2].converged);
        assert_eq!(results[2].decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_batch_processing_empty_input() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let encoder = LdpcEncoder::new(code.clone());

        let empty_messages: Vec<BitVec> = vec![];
        let codewords = encoder.encode_batch(&empty_messages);
        assert_eq!(codewords.len(), 0);

        let empty_llrs: Vec<Vec<Llr>> = vec![];
        let results = LdpcDecoder::decode_batch(&code, &empty_llrs, 10);
        assert_eq!(results.len(), 0);
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
            let row = g.row_as_bitvec(i);
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
            let row = g.row_as_bitvec(i);
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
