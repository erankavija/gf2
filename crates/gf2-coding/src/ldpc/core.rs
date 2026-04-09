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
    /// Cached systematic column positions (computed lazily via RREF).
    /// These are the k column indices in the full codeword that carry message
    /// bits. Computed once and reused by both the generator matrix and decoder.
    cached_systematic_cols: std::sync::Arc<std::sync::Mutex<Option<Vec<usize>>>>,
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
            cached_systematic_cols: std::sync::Arc::new(std::sync::Mutex::new(None)),
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

    /// Computes the systematic column positions from the parity-check matrix.
    ///
    /// Uses a fast heuristic first: if every row of H has at least one nonzero
    /// entry in columns k..n, then columns 0..k-1 are the systematic positions.
    /// This is O(nnz) and covers DVB-T2, random LDPC, and most standard codes.
    ///
    /// Falls back to full RREF with right-to-left pivoting only when needed
    /// (e.g., 5G NR BG2 where rows 40-41 have entries only in systematic columns).
    /// The RREF convention matches [`RuEncodingMatrices::preprocess`](crate::ldpc::encoding::RuEncodingMatrices)
    /// so decoder message extraction is consistent with the encoder.
    ///
    /// Returns `None` if H is rank deficient (RREF path only).
    fn compute_systematic_cols(&self) -> Option<Vec<usize>> {
        let k = self.k();
        let m = self.m;

        if k == 0 {
            return Some(Vec::new());
        }

        // Fast path: check if columns 0..k can serve as systematic positions.
        // This holds when every row of H has at least one nonzero entry in
        // columns k..n (the natural parity region). O(nnz) scan.
        let all_rows_touch_parity = (0..m).all(|row| self.h.row_iter(row).any(|col| col >= k));

        if all_rows_touch_parity {
            // Standard case: systematic columns are [0, 1, ..., k-1]
            return Some((0..k).collect());
        }

        // Slow path: some rows have entries only in columns 0..k.
        // Must run full RREF to determine the actual systematic positions.
        use gf2_core::alg::rref::rref;

        let h_dense = self.h.to_dense();

        // Use right-to-left pivoting to match the RU encoder convention
        let rref_result = rref(&h_dense, true);

        if rref_result.rank != m {
            return None;
        }

        let pivot_set: std::collections::HashSet<usize> =
            rref_result.pivot_cols.iter().copied().collect();
        let systematic_cols: Vec<usize> = (0..self.n).filter(|c| !pivot_set.contains(c)).collect();
        debug_assert_eq!(systematic_cols.len(), k);

        Some(systematic_cols)
    }

    /// Returns the systematic column positions (cached after first computation).
    ///
    /// These are the k column indices in the full codeword that carry message
    /// bits when encoding with Richardson-Urbanke systematic encoding. The
    /// positions are determined via RREF of the parity-check matrix H with
    /// right-to-left pivoting, matching the encoder's convention.
    ///
    /// Message bit `i` corresponds to codeword position `systematic_cols[i]`.
    ///
    /// # Panics
    ///
    /// Panics if H is rank deficient.
    ///
    /// # Complexity
    ///
    /// First call: O(m * n * min(m, n)) for RREF. Subsequent calls: O(1).
    pub(crate) fn systematic_cols(&self) -> Vec<usize> {
        let mut cache = self.cached_systematic_cols.lock().unwrap();
        if let Some(ref cols) = *cache {
            cols.clone()
        } else {
            let cols = self
                .compute_systematic_cols()
                .expect("LDPC parity-check matrix is not full rank");
            *cache = Some(cols.clone());
            cols
        }
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

/// Decoder algorithm selection for LDPC belief propagation.
///
/// Different check-node update rules trade off accuracy against speed.
/// Normalized and offset min-sum variants improve upon standard min-sum
/// by compensating for the overestimation bias inherent in the min approximation.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::DecoderAlgorithm;
///
/// let algo = DecoderAlgorithm::NormalizedMinSum(0.875);
/// let algo2 = DecoderAlgorithm::OffsetMinSum(0.5);
/// let algo3 = DecoderAlgorithm::MinSum;
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum DecoderAlgorithm {
    /// Standard min-sum approximation.
    ///
    /// Check-to-variable message:
    /// $$\lambda_{m \to n} = \prod \text{sign} \cdot \min |L_i|$$
    #[default]
    MinSum,
    /// Normalized min-sum: scales the min-sum output by a factor $\alpha \in (0, 1]$.
    ///
    /// Reduces overestimation bias. Typical values: 0.75--0.95.
    ///
    /// # Valid range
    ///
    /// `alpha` must be finite and in `(0.0, 1.0]`.
    NormalizedMinSum(f32),
    /// Offset min-sum: subtracts a non-negative offset $\beta$ from the min magnitude.
    ///
    /// Reduces overestimation bias. Typical values: 0.25--0.5.
    ///
    /// # Valid range
    ///
    /// `beta` must be finite and `>= 0.0`.
    OffsetMinSum(f32),
    /// Exact sum-product algorithm (box-plus).
    ///
    /// Uses $\tanh / \text{atanh}$ computations. Most accurate but slowest.
    SumProduct,
}

/// Configuration for the LDPC belief propagation decoder.
///
/// Controls the decoding algorithm and convergence behavior.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
///
/// // Default: MinSum with early termination enabled
/// let config = DecoderConfig::default();
/// assert!(config.early_termination());
///
/// // Normalized min-sum with custom parameters
/// let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecoderConfig {
    /// The check-node update algorithm
    algorithm: DecoderAlgorithm,
    /// Whether to stop early when syndrome check passes before max iterations
    early_termination: bool,
}

impl DecoderConfig {
    /// Creates a new decoder configuration.
    ///
    /// # Arguments
    ///
    /// * `algorithm` - The check-node update algorithm to use
    /// * `early_termination` - If `true`, decoding stops as soon as the syndrome check passes
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `NormalizedMinSum(alpha)` has `alpha` that is not finite or not in `(0.0, 1.0]`
    /// - `OffsetMinSum(beta)` has `beta` that is not finite or is negative
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    ///
    /// let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
    /// ```
    ///
    /// ```should_panic
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    ///
    /// // alpha = 0.0 is out of valid range (0.0, 1.0]
    /// let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.0), true);
    /// ```
    ///
    /// ```should_panic
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    ///
    /// // negative beta is invalid
    /// let config = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(-0.1), true);
    /// ```
    pub fn new(algorithm: DecoderAlgorithm, early_termination: bool) -> Self {
        match algorithm {
            DecoderAlgorithm::NormalizedMinSum(alpha) => {
                assert!(
                    alpha.is_finite() && alpha > 0.0 && alpha <= 1.0,
                    "NormalizedMinSum alpha must be finite and in (0.0, 1.0], got {}",
                    alpha
                );
            }
            DecoderAlgorithm::OffsetMinSum(beta) => {
                assert!(
                    beta.is_finite() && beta >= 0.0,
                    "OffsetMinSum beta must be finite and >= 0.0, got {}",
                    beta
                );
            }
            _ => {}
        }
        Self {
            algorithm,
            early_termination,
        }
    }

    /// Returns the configured algorithm.
    pub fn algorithm(&self) -> DecoderAlgorithm {
        self.algorithm
    }

    /// Returns whether early termination is enabled.
    pub fn early_termination(&self) -> bool {
        self.early_termination
    }
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            algorithm: DecoderAlgorithm::MinSum,
            early_termination: true,
        }
    }
}

/// Belief propagation decoder for LDPC codes.
///
/// Implements the sum-product algorithm (SPA) and min-sum approximations
/// for iterative soft-decision decoding, with configurable algorithm variants
/// and optional early termination.
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
///
/// ## Algorithm Variants
///
/// - [`DecoderAlgorithm::MinSum`] — Standard min-sum (default, fastest)
/// - [`DecoderAlgorithm::NormalizedMinSum`] — Scaled min-sum with correction factor
/// - [`DecoderAlgorithm::OffsetMinSum`] — Min-sum with offset correction
/// - [`DecoderAlgorithm::SumProduct`] — Exact box-plus (most accurate, slowest)
#[derive(Debug)]
pub struct LdpcDecoder {
    code: LdpcCode,
    /// Current variable node beliefs (posterior LLRs)
    beliefs: Vec<Llr>,
    /// Check-to-variable messages: indexed by [check][position in row]
    check_to_var: Vec<Vec<Llr>>,
    /// Variable-to-check messages: indexed by [var][position in column]
    var_to_check: Vec<Vec<Llr>>,
    /// Cached check node neighbors (pre-computed at construction)
    check_neighbors: Vec<Vec<usize>>,
    /// Cached variable node neighbors (pre-computed at construction)
    var_neighbors: Vec<Vec<usize>>,
    /// Temporary buffer for check node computations (reused to avoid allocations)
    temp_inputs: Vec<Llr>,
    /// Number of iterations in last decode
    last_iterations: usize,
    /// Decoder configuration (algorithm, early termination)
    config: DecoderConfig,
    /// Systematic column positions: message bit `i` corresponds to codeword
    /// position `systematic_cols[i]`. Computed lazily via RREF of H on first decode.
    /// Using `Option` avoids the expensive RREF at construction time for large codes.
    systematic_cols: Option<Vec<usize>>,
}

impl LdpcDecoder {
    /// Creates a new LDPC decoder for the given code with default configuration.
    ///
    /// Uses [`DecoderAlgorithm::MinSum`] with early termination enabled.
    ///
    /// # Arguments
    ///
    /// * `code` - The LDPC code to decode
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
    ///
    /// let edges = vec![(0, 0), (0, 1), (0, 2)];
    /// let code = LdpcCode::from_edges(1, 3, &edges);
    /// let decoder = LdpcDecoder::new(code);
    /// ```
    pub fn new(code: LdpcCode) -> Self {
        Self::with_config(code, DecoderConfig::default())
    }

    /// Creates a new LDPC decoder with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `code` - The LDPC code to decode
    /// * `config` - Decoder configuration (algorithm, early termination)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, DecoderAlgorithm, DecoderConfig};
    ///
    /// let edges = vec![(0, 0), (0, 1), (0, 2)];
    /// let code = LdpcCode::from_edges(1, 3, &edges);
    /// let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
    /// let decoder = LdpcDecoder::with_config(code, config);
    /// ```
    pub fn with_config(code: LdpcCode, config: DecoderConfig) -> Self {
        let n = code.n();
        let m = code.m();
        let h = code.parity_check_matrix();

        // Pre-compute check node neighbors (cached for hot path optimization)
        let check_neighbors: Vec<Vec<usize>> =
            (0..m).map(|check| h.row_iter(check).collect()).collect();

        // Pre-compute variable node neighbors (cached for hot path optimization)
        let var_neighbors: Vec<Vec<usize>> = (0..n).map(|var| h.col_iter(var).collect()).collect();

        // Find maximum check node degree for temp buffer sizing
        let max_check_degree = check_neighbors
            .iter()
            .map(|neighbors| neighbors.len())
            .max()
            .unwrap_or(0);

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
            check_neighbors,
            var_neighbors,
            temp_inputs: Vec::with_capacity(max_check_degree),
            last_iterations: 0,
            config,
            systematic_cols: None,
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
        Self::decode_batch_with_config(code, llr_blocks, max_iterations, DecoderConfig::default())
    }

    /// Decodes multiple LLR blocks in batch with a custom configuration.
    ///
    /// # Arguments
    ///
    /// * `code` - The LDPC code
    /// * `llr_blocks` - Slice of LLR vectors, one per frame
    /// * `max_iterations` - Maximum number of BP iterations per frame
    /// * `config` - Decoder configuration (algorithm, early termination)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, DecoderAlgorithm, DecoderConfig};
    /// use gf2_coding::llr::Llr;
    ///
    /// let edges = vec![(0, 0), (0, 1), (0, 2)];
    /// let code = LdpcCode::from_edges(1, 3, &edges);
    /// let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
    ///
    /// let llr_blocks: Vec<Vec<Llr>> = (0..10)
    ///     .map(|_| vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)])
    ///     .collect();
    ///
    /// let results = LdpcDecoder::decode_batch_with_config(&code, &llr_blocks, 10, config);
    /// assert_eq!(results.len(), 10);
    /// ```
    pub fn decode_batch_with_config(
        code: &LdpcCode,
        llr_blocks: &[Vec<Llr>],
        max_iterations: usize,
        config: DecoderConfig,
    ) -> Vec<DecoderResult> {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            (0..llr_blocks.len())
                .into_par_iter()
                .map(|i| {
                    let mut decoder = Self::with_config(code.clone(), config);
                    decoder.decode_iterative(&llr_blocks[i], max_iterations)
                })
                .collect()
        }

        #[cfg(not(feature = "parallel"))]
        {
            llr_blocks
                .iter()
                .map(|llrs| {
                    let mut decoder = Self::with_config(code.clone(), config);
                    decoder.decode_iterative(llrs, max_iterations)
                })
                .collect()
        }
    }

    /// Performs check node update (sum-product algorithm).
    ///
    /// Computes check-to-variable messages using the exact box-plus operation.
    fn check_node_update_spa(&mut self, _channel_llrs: &[Llr]) {
        for (check, neighbors) in self.check_neighbors.iter().enumerate() {
            for (pos, &_var) in neighbors.iter().enumerate() {
                // Reuse pre-allocated buffer
                self.temp_inputs.clear();

                for (other_pos, &other_var) in neighbors.iter().enumerate() {
                    if other_pos != pos {
                        // Get variable-to-check message
                        let var_check_pos = self.find_check_position(other_var, check);
                        self.temp_inputs
                            .push(self.var_to_check[other_var][var_check_pos]);
                    }
                }

                // Compute check-to-variable message using box-plus
                let message = if self.temp_inputs.is_empty() {
                    Llr::zero()
                } else {
                    Llr::boxplus_n(&self.temp_inputs)
                };

                self.check_to_var[check][pos] = message;
            }
        }
    }

    /// Performs check node update (min-sum approximation).
    fn check_node_update_minsum(&mut self, _channel_llrs: &[Llr]) {
        for (check, neighbors) in self.check_neighbors.iter().enumerate() {
            for (pos, &_var) in neighbors.iter().enumerate() {
                // Reuse pre-allocated buffer
                self.temp_inputs.clear();

                for (other_pos, &other_var) in neighbors.iter().enumerate() {
                    if other_pos != pos {
                        let var_check_pos = self.find_check_position(other_var, check);
                        self.temp_inputs
                            .push(self.var_to_check[other_var][var_check_pos]);
                    }
                }

                let message = if self.temp_inputs.is_empty() {
                    Llr::zero()
                } else {
                    // boxplus_minsum_n handles SIMD dispatch internally
                    Llr::boxplus_minsum_n(&self.temp_inputs)
                };

                self.check_to_var[check][pos] = message;
            }
        }
    }

    /// Performs check node update (normalized min-sum approximation).
    ///
    /// Scales the standard min-sum output by `alpha` to compensate for overestimation.
    fn check_node_update_normalized_minsum(&mut self, _channel_llrs: &[Llr], alpha: f32) {
        for (check, neighbors) in self.check_neighbors.iter().enumerate() {
            for (pos, &_var) in neighbors.iter().enumerate() {
                self.temp_inputs.clear();

                for (other_pos, &other_var) in neighbors.iter().enumerate() {
                    if other_pos != pos {
                        let var_check_pos = self.find_check_position(other_var, check);
                        self.temp_inputs
                            .push(self.var_to_check[other_var][var_check_pos]);
                    }
                }

                let message = if self.temp_inputs.is_empty() {
                    Llr::zero()
                } else {
                    Llr::boxplus_normalized_minsum_n(&self.temp_inputs, alpha)
                };

                self.check_to_var[check][pos] = message;
            }
        }
    }

    /// Performs check node update (offset min-sum approximation).
    ///
    /// Subtracts `beta` from the minimum magnitude to compensate for overestimation.
    fn check_node_update_offset_minsum(&mut self, _channel_llrs: &[Llr], beta: f32) {
        for (check, neighbors) in self.check_neighbors.iter().enumerate() {
            for (pos, &_var) in neighbors.iter().enumerate() {
                self.temp_inputs.clear();

                for (other_pos, &other_var) in neighbors.iter().enumerate() {
                    if other_pos != pos {
                        let var_check_pos = self.find_check_position(other_var, check);
                        self.temp_inputs
                            .push(self.var_to_check[other_var][var_check_pos]);
                    }
                }

                let message = if self.temp_inputs.is_empty() {
                    Llr::zero()
                } else {
                    Llr::boxplus_offset_minsum_n(&self.temp_inputs, beta)
                };

                self.check_to_var[check][pos] = message;
            }
        }
    }

    /// Dispatches the check node update to the configured algorithm.
    fn check_node_update(&mut self, channel_llrs: &[Llr]) {
        match self.config.algorithm {
            DecoderAlgorithm::MinSum => self.check_node_update_minsum(channel_llrs),
            DecoderAlgorithm::NormalizedMinSum(alpha) => {
                self.check_node_update_normalized_minsum(channel_llrs, alpha)
            }
            DecoderAlgorithm::OffsetMinSum(beta) => {
                self.check_node_update_offset_minsum(channel_llrs, beta)
            }
            DecoderAlgorithm::SumProduct => self.check_node_update_spa(channel_llrs),
        }
    }

    /// Performs variable node update.
    ///
    /// Updates beliefs and variable-to-check messages.
    fn variable_node_update(&mut self, channel_llrs: &[Llr]) {
        for (var, &channel_llr) in channel_llrs.iter().enumerate().take(self.code.n()) {
            let neighbors = &self.var_neighbors[var];

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
        self.var_neighbors[var]
            .iter()
            .position(|&check| check == target_check)
            .expect("Check not found in variable's neighbors")
    }

    /// Helper: Get check-to-variable message.
    fn check_to_var_message(&self, var: usize, var_check_pos: usize) -> Llr {
        let check = self.var_neighbors[var][var_check_pos];
        let check_var_pos = self.check_neighbors[check]
            .iter()
            .position(|&v| v == var)
            .unwrap();
        self.check_to_var[check][check_var_pos]
    }

    /// Runs iterative BP and returns the full decoded codeword (all n bits).
    ///
    /// Unlike [`IterativeSoftDecoder::decode_iterative`] which extracts k
    /// message bits from systematic positions, this returns the hard-decided
    /// codeword at all n positions. Useful when the caller knows the
    /// systematic column mapping (e.g., 5G NR rate-matched codes that use
    /// natural column ordering per `SYSTEMATIC_ENCODING_CONVENTION.md`).
    ///
    /// # Arguments
    ///
    /// * `llrs` - Channel LLRs, one per codeword position (length n)
    /// * `max_iterations` - Maximum BP iterations
    ///
    /// # Returns
    ///
    /// A [`DecoderResult`] where `decoded_bits` contains the full n-bit
    /// codeword (not just the k message bits).
    ///
    /// # Complexity
    ///
    /// Same as [`IterativeSoftDecoder::decode_iterative`].
    pub fn decode_to_codeword(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
        assert_eq!(llrs.len(), self.code.n(), "LLR length must equal n");

        // Reset all messages
        for check_msgs in &mut self.check_to_var {
            for msg in check_msgs {
                *msg = Llr::zero();
            }
        }
        for (var, &llr) in llrs.iter().enumerate().take(self.code.n()) {
            for pos in 0..self.var_to_check[var].len() {
                self.var_to_check[var][pos] = llr;
            }
        }

        let mut iterations = 0;
        let mut converged = false;

        for iter in 0..max_iterations {
            iterations = iter + 1;
            self.check_node_update(llrs);
            self.variable_node_update(llrs);

            if self.config.early_termination {
                let decoded = self.hard_decode();
                if self.code.is_valid_codeword(&decoded) {
                    converged = true;
                    break;
                }
            }
        }

        self.last_iterations = iterations;
        let decoded_codeword = self.hard_decode();
        let syndrome_passed = self.code.is_valid_codeword(&decoded_codeword);

        if !self.config.early_termination {
            converged = syndrome_passed;
        }

        DecoderResult::new(decoded_codeword, iterations, converged, syndrome_passed)
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
        let early_termination = self.config.early_termination;

        for iter in 0..max_iterations {
            iterations = iter + 1;

            // Check node update (dispatches to configured algorithm)
            self.check_node_update(llrs);

            // Variable node update
            self.variable_node_update(llrs);

            // Early termination: check syndrome before max iterations
            if early_termination {
                let decoded = self.hard_decode();
                if self.code.is_valid_codeword(&decoded) {
                    converged = true;
                    break;
                }
            }
        }

        self.last_iterations = iterations;
        let decoded_codeword = self.hard_decode();
        let syndrome_passed = self.code.is_valid_codeword(&decoded_codeword);

        // If early termination was disabled, set converged based on final syndrome
        if !early_termination {
            converged = syndrome_passed;
        }

        // Extract message bits from the decoded codeword at the systematic
        // column positions determined by RREF. Message bit i is located at
        // codeword position systematic_cols[i], which may differ from i when
        // RREF assigns pivots to columns in the natural systematic range
        // (e.g., for QC-LDPC codes like 5G NR BG2).
        //
        // Computed lazily on first decode to avoid expensive RREF at
        // construction time for large codes (e.g., DVB-T2 normal frames).
        let sys_cols = self
            .systematic_cols
            .get_or_insert_with(|| self.code.systematic_cols());
        let k = self.code.k();
        let mut message = BitVec::with_capacity(k);
        for &col in &sys_cols[..k] {
            message.push_bit(decoded_codeword.get(col));
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
    /// Encodes multiple messages in batch using ComputeBackend.
    ///
    /// Uses the default CpuBackend which automatically selects SIMD kernels
    /// and parallel processing when the `parallel` feature is enabled.
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
    /// let encoder = LdpcEncoder::new(code.clone());
    ///
    /// // Message length must match code.k() = n - m = 3 - 1 = 2 bits
    /// let mut msg1 = BitVec::new();
    /// msg1.push_bit(false);
    /// msg1.push_bit(false);
    ///
    /// let mut msg2 = BitVec::new();
    /// msg2.push_bit(true);
    /// msg2.push_bit(true);
    ///
    /// let messages = vec![msg1, msg2];
    /// let codewords = encoder.encode_batch(&messages);
    /// assert_eq!(codewords.len(), 2);
    /// assert_eq!(codewords[0].len(), code.n());
    /// ```
    pub fn encode_batch(&self, messages: &[BitVec]) -> Vec<BitVec> {
        // Use default CpuBackend for batch operations
        let backend = gf2_core::compute::CpuBackend::new();
        self.encoding_matrices.encode_batch(messages, &backend)
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

    /// Test that cached neighbors produce identical results to dynamic iteration
    #[test]
    fn test_cached_neighbors_correctness() {
        let code = LdpcCode::dvb_t2_normal(crate::CodeRate::Rate3_5);
        let h = code.parity_check_matrix();

        // Pre-compute neighbors (what we'll cache)
        let cached: Vec<Vec<usize>> = (0..code.m())
            .map(|check| h.row_iter(check).collect())
            .collect();

        // Verify against dynamic iteration
        for (check, cached_neighbors) in cached.iter().enumerate() {
            let dynamic: Vec<usize> = h.row_iter(check).collect();
            assert_eq!(
                cached_neighbors, &dynamic,
                "Cached neighbors must match dynamic iteration for check {}",
                check
            );
        }
    }

    /// Test that decoder with cached neighbors produces same output as original
    #[test]
    fn test_decoder_equivalence_with_caching() {
        let code = LdpcCode::dvb_t2_normal(crate::CodeRate::Rate3_5);

        // Create test LLRs (high SNR, should decode in 1-2 iterations)
        let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();

        // Decode twice (after optimization, both paths will use cached neighbors)
        let mut decoder1 = LdpcDecoder::new(code.clone());
        let result1 = decoder1.decode_iterative(&llrs, 50);

        let mut decoder2 = LdpcDecoder::new(code.clone());
        let result2 = decoder2.decode_iterative(&llrs, 50);

        // Results must be identical
        assert_eq!(result1.converged, result2.converged);
        assert_eq!(result1.iterations, result2.iterations);
        assert_eq!(result1.decoded_bits, result2.decoded_bits);
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

#[cfg(test)]
mod algorithm_tests {
    use super::*;
    use crate::traits::IterativeSoftDecoder;

    /// Helper: build a simple [3,2] single parity check code
    fn simple_parity_code() -> LdpcCode {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        LdpcCode::from_edges(1, 3, &edges)
    }

    #[test]
    fn test_decoder_config_default() {
        let config = DecoderConfig::default();
        assert_eq!(config.algorithm(), DecoderAlgorithm::MinSum);
        assert!(config.early_termination());
    }

    #[test]
    fn test_decoder_config_new_valid_normalized() {
        let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
        assert_eq!(
            config.algorithm(),
            DecoderAlgorithm::NormalizedMinSum(0.875)
        );
    }

    #[test]
    fn test_decoder_config_new_valid_offset() {
        let config = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(0.5), false);
        assert_eq!(config.algorithm(), DecoderAlgorithm::OffsetMinSum(0.5));
        assert!(!config.early_termination());
    }

    #[test]
    fn test_decoder_config_new_normalized_alpha_one() {
        // alpha = 1.0 is valid (upper bound inclusive)
        let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(1.0), true);
        assert_eq!(config.algorithm(), DecoderAlgorithm::NormalizedMinSum(1.0));
    }

    #[test]
    fn test_decoder_config_new_offset_beta_zero() {
        // beta = 0.0 is valid (lower bound inclusive)
        let config = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(0.0), true);
        assert_eq!(config.algorithm(), DecoderAlgorithm::OffsetMinSum(0.0));
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_zero() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.0), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_negative() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(-0.5), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_greater_than_one() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(1.1), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_nan() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(f32::NAN), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_inf() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(f32::INFINITY), true);
    }

    #[test]
    #[should_panic(expected = "OffsetMinSum beta must be finite and >= 0.0")]
    fn test_decoder_config_rejects_beta_negative() {
        DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(-0.1), true);
    }

    #[test]
    #[should_panic(expected = "OffsetMinSum beta must be finite and >= 0.0")]
    fn test_decoder_config_rejects_beta_nan() {
        DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(f32::NAN), true);
    }

    #[test]
    #[should_panic(expected = "OffsetMinSum beta must be finite and >= 0.0")]
    fn test_decoder_config_rejects_beta_inf() {
        DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(f32::INFINITY), true);
    }

    #[test]
    fn test_normalized_minsum_single_error_correction() {
        let code = simple_parity_code();
        let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
        let mut decoder = LdpcDecoder::with_config(code, config);

        // Strong all-zero codeword
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result = decoder.decode_iterative(&llrs, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_offset_minsum_single_error_correction() {
        let code = simple_parity_code();
        let config = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(0.5), true);
        let mut decoder = LdpcDecoder::with_config(code, config);

        // Strong all-zero codeword
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result = decoder.decode_iterative(&llrs, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_sum_product_single_error_correction() {
        let code = simple_parity_code();
        let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);
        let mut decoder = LdpcDecoder::with_config(code, config);

        // Strong all-zero codeword
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result = decoder.decode_iterative(&llrs, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_early_termination_reduces_iterations() {
        let code = simple_parity_code();

        // With early termination (default)
        let config_early = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
        let mut decoder_early = LdpcDecoder::with_config(code.clone(), config_early);

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result_early = decoder_early.decode_iterative(&llrs, 50);
        assert!(result_early.converged);
        assert!(
            result_early.iterations < 50,
            "Should converge before max iterations"
        );

        // Without early termination
        let config_no_early = DecoderConfig::new(DecoderAlgorithm::MinSum, false);
        let mut decoder_no_early = LdpcDecoder::with_config(code, config_no_early);
        let result_no_early = decoder_no_early.decode_iterative(&llrs, 50);

        // Without early termination, always runs all max_iterations
        assert_eq!(result_no_early.iterations, 50);
        assert!(result_no_early.syndrome_check_passed);
        assert!(result_no_early.converged); // converged is set based on final syndrome when early_termination=false
    }

    #[test]
    fn test_all_algorithms_agree_on_high_snr() {
        let code = simple_parity_code();
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let max_iter = 20;

        let algorithms = [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::NormalizedMinSum(0.875),
            DecoderAlgorithm::OffsetMinSum(0.5),
            DecoderAlgorithm::SumProduct,
        ];

        for &algo in &algorithms {
            let config = DecoderConfig::new(algo, true);
            let mut decoder = LdpcDecoder::with_config(code.clone(), config);
            let result = decoder.decode_iterative(&llrs, max_iter);

            assert!(
                result.converged,
                "Algorithm {:?} should converge at high SNR",
                algo
            );
            assert!(
                result.syndrome_check_passed,
                "Algorithm {:?} should pass syndrome check",
                algo
            );
            assert_eq!(
                result.decoded_bits.count_ones(),
                0,
                "Algorithm {:?} should decode all-zero codeword",
                algo
            );
        }
    }

    /// Diagnostic test: sum-product vs NMS on the (256,121) 5G NR rate-matched code.
    ///
    /// With noiseless LLRs, NMS converges but SP produces NaN/Inf due to
    /// numerical overflow in boxplus_n (tanh product -> 1.0 -> atanh(1.0) = Inf).
    #[test]
    fn test_sum_product_nr5g_256_121_noiseless() {
        use crate::ldpc::nr_5g::Nr5gRateMatchedDecoder;
        use crate::ldpc::QuasiCyclicLdpc;
        use crate::traits::{BlockEncoder, IterativeSoftDecoder};

        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let target_k = rm_code.params().target_k;
        let target_n = rm_code.params().target_n;

        // Encode the zero message
        let message = BitVec::zeros(target_k);
        let codeword = rm_code.encode(&message);
        assert_eq!(codeword.len(), target_n);

        // Create noiseless LLRs: +10.0 for bit=0, -10.0 for bit=1
        let channel_llrs: Vec<Llr> = (0..target_n)
            .map(|i| {
                if codeword.get(i) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        // NMS (alpha=0.75) should converge
        let mut nms_decoder = Nr5gRateMatchedDecoder::new(rm_code.clone());
        let nms_result = nms_decoder.decode_iterative(&channel_llrs, 50);
        assert!(
            nms_result.converged,
            "NMS should converge on noiseless input"
        );
        assert!(
            nms_result.syndrome_check_passed,
            "NMS should pass syndrome check"
        );
        let nms_errors = (0..target_k)
            .filter(|&i| nms_result.decoded_bits.get(i) != message.get(i))
            .count();
        assert_eq!(nms_errors, 0, "NMS should have zero bit errors");

        // SumProduct should also converge
        let mut sp_decoder =
            Nr5gRateMatchedDecoder::with_algorithm(rm_code.clone(), DecoderAlgorithm::SumProduct);
        let sp_result = sp_decoder.decode_iterative(&channel_llrs, 50);

        let sp_errors = (0..target_k)
            .filter(|&i| sp_result.decoded_bits.get(i) != message.get(i))
            .count();

        assert!(
            sp_result.converged,
            "SP should converge on noiseless input (converged={}, syndrome={}, iters={}, errors={})",
            sp_result.converged,
            sp_result.syndrome_check_passed,
            sp_result.iterations,
            sp_errors
        );
        assert!(
            sp_result.syndrome_check_passed,
            "SP should pass syndrome check"
        );
        assert_eq!(sp_errors, 0, "SP should have zero bit errors");
    }

    /// Diagnostic: check that boxplus_n handles extreme LLR values without NaN/Inf.
    ///
    /// When var-to-check messages grow large (>~9 in f32), tanh(x/2) saturates
    /// to exactly 1.0, making the product 1.0, and atanh(1.0) = Inf.
    /// This propagates through the graph: Inf - Inf = NaN at variable nodes.
    #[test]
    fn test_boxplus_n_numerical_stability() {
        // Simulate what happens in a check node with large messages.
        // In the 5G rate-matched code, filler LLR=15.0 and after a few
        // iterations, var-to-check messages can reach 20+.
        let large_msgs: Vec<Llr> = vec![
            Llr::new(15.0),
            Llr::new(12.0),
            Llr::new(10.0),
            Llr::new(8.0),
            Llr::new(15.0),
        ];
        let result = Llr::boxplus_n(&large_msgs);
        assert!(
            result.is_finite(),
            "boxplus_n should not produce Inf for large positive LLRs, got {}",
            result.value()
        );

        // Mix of large positive and zero (punctured positions)
        let mixed_msgs: Vec<Llr> = vec![
            Llr::new(15.0),
            Llr::new(0.0),
            Llr::new(10.0),
            Llr::new(15.0),
        ];
        let result = Llr::boxplus_n(&mixed_msgs);
        assert!(
            result.is_finite(),
            "boxplus_n with a zero input should be finite, got {}",
            result.value()
        );
        // tanh(0/2) = 0, so product = 0, atanh(0) = 0
        assert_eq!(
            result.value(),
            0.0,
            "boxplus_n with a zero input should return 0"
        );

        // Very large messages (simulating after many iterations)
        let huge_msgs: Vec<Llr> = vec![Llr::new(50.0), Llr::new(30.0), Llr::new(40.0)];
        let result = Llr::boxplus_n(&huge_msgs);
        assert!(
            result.is_finite(),
            "boxplus_n should not produce Inf for very large LLRs, got {}",
            result.value()
        );

        // Also check the first case passes after fix
        assert!(
            Llr::boxplus_n(&large_msgs).value().abs() < 20.0,
            "boxplus_n of moderate LLRs should produce a bounded result"
        );
    }

    #[test]
    fn test_all_algorithms_converge_on_11_codeword() {
        let code = simple_parity_code();
        // [1,1,0] is a valid codeword (even parity)
        let llrs = vec![Llr::new(-10.0), Llr::new(-10.0), Llr::new(10.0)];
        let max_iter = 20;

        let algorithms = [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::NormalizedMinSum(0.875),
            DecoderAlgorithm::OffsetMinSum(0.5),
            DecoderAlgorithm::SumProduct,
        ];

        for &algo in &algorithms {
            let config = DecoderConfig::new(algo, true);
            let mut decoder = LdpcDecoder::with_config(code.clone(), config);
            let result = decoder.decode_iterative(&llrs, max_iter);

            assert!(
                result.converged,
                "Algorithm {:?} should converge for [1,1,0]",
                algo
            );
            assert_eq!(
                result.decoded_bits.count_ones(),
                2,
                "Algorithm {:?} should decode to [1,1,0]",
                algo
            );
        }
    }
}

#[cfg(test)]
mod profiling_helpers {
    use super::*;

    #[test]
    #[ignore]
    fn measure_check_node_degrees() {
        let code = LdpcCode::dvb_t2_normal(crate::CodeRate::Rate3_5);
        let h = code.parity_check_matrix();

        let mut degrees = Vec::new();
        for check in 0..code.m() {
            let degree = h.row_iter(check).count();
            degrees.push(degree);
        }

        degrees.sort();

        println!("\nDVB-T2 NORMAL Rate 3/5 check node degrees:");
        println!("  Total checks: {}", code.m());
        println!("  Min:    {}", degrees.iter().min().unwrap());
        println!("  Max:    {}", degrees.iter().max().unwrap());
        println!("  Median: {}", degrees[degrees.len() / 2]);
        println!(
            "  Mean:   {:.2}",
            degrees.iter().sum::<usize>() as f64 / degrees.len() as f64
        );

        // Histogram
        let mut histogram = std::collections::HashMap::new();
        for &deg in &degrees {
            *histogram.entry(deg).or_insert(0) += 1;
        }

        println!("\nHistogram:");
        let mut bins: Vec<_> = histogram.iter().collect();
        bins.sort_by_key(|(k, _)| *k);
        for (deg, count) in bins {
            println!(
                "  Degree {:2}: {:5} checks ({:.1}%)",
                deg,
                count,
                100.0 * *count as f64 / code.m() as f64
            );
        }
    }
}

#[cfg(test)]
mod decoder_proptests {
    use super::*;
    use crate::traits::IterativeSoftDecoder;
    use proptest::prelude::*;

    proptest! {
        /// At high SNR, all min-sum variants (MinSum, NormalizedMinSum, OffsetMinSum)
        /// should produce the same hard decisions for the all-zero codeword.
        #[test]
        fn test_algorithm_variants_agree_at_high_snr(
            alpha in 0.5f32..=1.0f32,
            beta in 0.0f32..=1.0f32,
            snr_mag in 5.0f32..20.0f32,
        ) {
            // Single parity check code [3,2]
            let edges = vec![(0, 0), (0, 1), (0, 2)];
            let code = LdpcCode::from_edges(1, 3, &edges);

            // High SNR all-zero codeword
            let llrs = vec![Llr::new(snr_mag), Llr::new(snr_mag), Llr::new(snr_mag)];

            // Decode with MinSum
            let config_ms = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
            let mut decoder_ms = LdpcDecoder::with_config(code.clone(), config_ms);
            let result_ms = decoder_ms.decode_iterative(&llrs, 20);

            // Decode with NormalizedMinSum
            let config_nms = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(alpha), true);
            let mut decoder_nms = LdpcDecoder::with_config(code.clone(), config_nms);
            let result_nms = decoder_nms.decode_iterative(&llrs, 20);

            // Decode with OffsetMinSum
            let config_oms = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(beta), true);
            let mut decoder_oms = LdpcDecoder::with_config(code, config_oms);
            let result_oms = decoder_oms.decode_iterative(&llrs, 20);

            // All should converge at high SNR
            prop_assert!(result_ms.converged, "MinSum should converge");
            prop_assert!(result_nms.converged, "NormalizedMinSum should converge");
            prop_assert!(result_oms.converged, "OffsetMinSum should converge");

            // All should produce the same hard decisions (all-zero)
            prop_assert_eq!(result_ms.decoded_bits.count_ones(), 0);
            prop_assert_eq!(result_nms.decoded_bits.count_ones(), 0);
            prop_assert_eq!(result_oms.decoded_bits.count_ones(), 0);
        }
    }
}
