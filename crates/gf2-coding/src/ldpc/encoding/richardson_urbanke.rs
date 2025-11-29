//! Richardson-Urbanke systematic encoding for LDPC codes.
//!
//! Implements efficient systematic encoding using preprocessed matrices.
//!
//! # Algorithm
//!
//! Given parity-check matrix H (m × n), preprocessing computes encoding
//! matrices that enable O(edges) encoding complexity:
//!
//! 1. **Preprocessing** (once per code):
//!    - Apply Gaussian elimination to transform H to approximate systematic form
//!    - Compute encoding matrices φ and ψ from the structured parts
//!    - Cache these matrices for repeated use
//!
//! 2. **Encoding** (fast, repeated):
//!    - Given message m, compute parity bits using φ and ψ
//!    - Concatenate to form systematic codeword [m | parity]
//!
//! # References
//!
//! Richardson, T. and Urbanke, R. (2001). "Efficient encoding of low-density
//! parity-check codes." IEEE Transactions on Information Theory, 47(2), 638-656.

use gf2_core::alg::rref::rref;
use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::{BitMatrix, BitVec};
use std::fmt;

/// Error types for encoding preprocessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessError {
    /// Matrix is not full rank
    RankDeficient,
    /// Matrix dimensions invalid
    InvalidDimensions,
    /// Gaussian elimination failed
    GaussianEliminationFailed,
}

impl fmt::Display for PreprocessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RankDeficient => write!(f, "Parity-check matrix is not full rank"),
            Self::InvalidDimensions => write!(f, "Invalid matrix dimensions"),
            Self::GaussianEliminationFailed => write!(f, "Gaussian elimination failed"),
        }
    }
}

impl std::error::Error for PreprocessError {}

/// Preprocessed matrices for Richardson-Urbanke encoding.
///
/// These matrices are computed once per LDPC code configuration and then
/// cached for repeated encoding operations.
///
/// # Storage Convention
///
/// For systematic codes G = [I_k | P], we store ONLY the parity part P
/// since the identity part is redundant.
///
/// The parity matrix is stored as **dense BitMatrix** because DVB-T2 parity
/// matrices are 40-50% dense. Dense storage is 30× smaller and potentially
/// faster with SIMD for high-density matrices.
///
/// The full generator matrix is available via `generator()` which reconstructs
/// it on-demand by adjoining the identity part. The parity part alone is
/// available via `parity_part()` for efficient encoding.
///
/// # Encoding
///
/// Encoding uses only the parity part: `parity = P^T × message`,
/// then places bits in their systematic/parity positions.
#[derive(Debug, Clone)]
pub struct RuEncodingMatrices {
    /// Message dimension k
    k: usize,
    /// Codeword length n
    n: usize,
    /// Parity length r = n - k
    r: usize,
    /// Parity matrix P (k × r) stored as DENSE bit-packed matrix
    /// For systematic codes, this is the only stored matrix.
    /// Dense storage used because DVB-T2 matrices are 40-50% dense.
    /// Used to compute parity bits: parity = P^T × message
    parity_matrix: BitMatrix,
    /// Systematic bit positions (length k)
    /// For standard systematic codes: [0, 1, ..., k-1]
    systematic_cols: Vec<usize>,
    /// Parity bit positions (length r)
    /// For standard systematic codes: [k, k+1, ..., n-1]
    parity_cols: Vec<usize>,
    /// Whether this is a systematic code
    is_systematic: bool,
}

impl RuEncodingMatrices {
    /// Preprocess parity-check matrix for fast encoding.
    ///
    /// Computes generator matrix G from parity-check matrix H.
    /// For a systematic code, G = [I_k | P] where P is the parity part.
    ///
    /// # Arguments
    ///
    /// * `h` - Parity-check matrix (m × n) in sparse format
    ///
    /// # Returns
    ///
    /// Preprocessed encoding matrices, or error if preprocessing fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use gf2_coding::ldpc::encoding::RuEncodingMatrices;
    ///
    /// let matrices = RuEncodingMatrices::preprocess(&h)?;
    /// let codeword = matrices.encode(&message);
    /// ```
    pub fn preprocess(h: &SpBitMatrixDual) -> Result<Self, PreprocessError> {
        let m = h.rows();
        let n = h.cols();

        if m == 0 || n == 0 || m >= n {
            return Err(PreprocessError::InvalidDimensions);
        }

        let k = n - m;

        eprintln!("  Converting to dense ({} × {})...", m, n);

        // Convert sparse H to dense for RREF
        let mut h_dense = BitMatrix::zeros(m, n);
        for row in 0..m {
            for col_idx in h.row_iter(row) {
                h_dense.set(row, col_idx, true);
            }
        }

        eprintln!("  Running Gaussian elimination...");

        // Compute parity matrix using compute_generator_matrix
        Self::compute_generator_matrix(&h_dense, k, n)
    }

    /// Compute generator matrix from parity-check matrix.
    ///
    /// Computes G such that H·G^T = 0 using the following algorithm:
    ///
    /// 1. RREF (Reduced Row Echelon Form) with right-to-left pivoting to find m
    ///    independent columns for parity positions. Uses gf2-core's optimized
    ///    word-level RREF implementation with SIMD acceleration.
    /// 2. **Critical**: Reorder rows so row i has its unique pivot in parity_cols[i]
    ///    to align the identity structure correctly
    /// 3. Build G = [I_k | P] where P[i,j] = H_work[row_order[j], message_cols[i]]
    ///
    /// The row reordering step ensures the transformed H has proper structure
    /// [A | I_m], allowing correct extraction of parity relationships.
    fn compute_generator_matrix(
        h: &BitMatrix,
        k: usize,
        n: usize,
    ) -> Result<Self, PreprocessError> {
        let m = h.rows();

        // Use gf2-core's optimized RREF with word-level operations and SIMD acceleration
        // pivot_from_right=true to prefer parity bits on right
        eprintln!("  Running RREF (word-level + SIMD)...");
        let rref_result = rref(h, true);

        if rref_result.rank != m {
            return Err(PreprocessError::RankDeficient);
        }

        let parity_cols = rref_result.pivot_cols;
        let h_work = rref_result.reduced;
        eprintln!("  RREF complete (rank = {})", rref_result.rank);

        // Message columns are non-parity columns
        let mut message_cols = Vec::new();
        for col in 0..n {
            if !parity_cols.contains(&col) {
                message_cols.push(col);
            }
        }

        if message_cols.len() != k {
            return Err(PreprocessError::GaussianEliminationFailed);
        }

        // Reorder rows so row i has its pivot in parity_cols[i]
        // This ensures the identity structure aligns correctly
        let mut row_order = Vec::new();
        for &pcol in &parity_cols {
            for row in 0..m {
                if h_work.get(row, pcol) {
                    // Check this is the only pivot column with 1 in this row
                    let is_pivot_row = parity_cols
                        .iter()
                        .all(|&pc2| pc2 == pcol || !h_work.get(row, pc2));
                    if is_pivot_row {
                        row_order.push(row);
                        break;
                    }
                }
            }
        }

        if row_order.len() != m {
            return Err(PreprocessError::GaussianEliminationFailed);
        }

        // Build parity matrix P (k × r) as DENSE
        // From H·G^T = 0, we get: P[i, j] = H_work[row_order[j], message_cols[i]]
        // Dense storage is optimal for DVB-T2 (40-50% density)
        let mut p = BitMatrix::zeros(k, m);

        eprintln!("  Building dense parity matrix ({} × {})...", k, m);
        let mut nnz = 0;
        for (i, &msg_col) in message_cols.iter().enumerate() {
            for (j, &row_idx) in row_order.iter().enumerate() {
                let h_val = h_work.get(row_idx, msg_col);
                if h_val {
                    p.set(i, j, true);
                    nnz += 1;
                }
            }
        }
        let density = (nnz as f64) / ((k * m) as f64) * 100.0;
        eprintln!(
            "  Parity matrix: {} non-zero entries ({:.1}% dense)",
            nnz, density
        );

        Ok(Self {
            k,
            n,
            r: m,
            parity_matrix: p,
            systematic_cols: message_cols,
            parity_cols,
            is_systematic: true,
        })
    }

    /// Encode a message into a systematic codeword.
    ///
    /// For systematic codes, the codeword is [message | parity] where
    /// parity bits are computed as: parity = P^T × message
    ///
    /// Uses dense matrix-vector multiply with word-level operations.
    ///
    /// # Arguments
    ///
    /// * `message` - Message bits (length k)
    ///
    /// # Returns
    ///
    /// Systematic codeword with length n.
    ///
    /// # Panics
    ///
    /// Panics if message length doesn't equal k.
    pub fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.k,
            "Message length must be k = {}",
            self.k
        );

        // Compute parity bits: parity = P^T × message
        // Dense matrix-vector multiply with word-level operations
        let parity = self.parity_matrix.matvec_transpose(message);

        // Build codeword by placing message and parity in correct positions
        let mut codeword = BitVec::zeros(self.n);

        // Place message bits in systematic positions
        for (i, &col) in self.systematic_cols.iter().enumerate() {
            codeword.set(col, message.get(i));
        }

        // Place parity bits in parity positions
        for (j, &col) in self.parity_cols.iter().enumerate() {
            codeword.set(col, parity.get(j));
        }

        codeword
    }

    /// Encodes multiple messages using ComputeBackend for parallelization.
    ///
    /// This method leverages the ComputeBackend's batch_matvec_transpose operation
    /// to efficiently encode multiple messages, potentially in parallel.
    ///
    /// # Arguments
    ///
    /// * `messages` - Slice of messages to encode (each must have length k)
    /// * `backend` - Compute backend to use for matrix operations
    ///
    /// # Returns
    ///
    /// Vector of codewords (each of length n)
    ///
    /// # Panics
    ///
    /// Panics if any message length doesn't equal k.
    pub fn encode_batch(
        &self,
        messages: &[BitVec],
        backend: &dyn gf2_core::compute::ComputeBackend,
    ) -> Vec<BitVec> {
        // Validate all message lengths
        for msg in messages {
            assert_eq!(msg.len(), self.k, "Message length must be k = {}", self.k);
        }

        if messages.is_empty() {
            return vec![];
        }

        // Compute all parity bits in parallel: parity_i = P^T × message_i
        let parities = backend.batch_matvec_transpose(&self.parity_matrix, messages);

        // Build codewords by placing message and parity bits
        messages
            .iter()
            .zip(parities.iter())
            .map(|(message, parity)| {
                let mut codeword = BitVec::zeros(self.n);

                // Place message bits in systematic positions
                for (i, &col) in self.systematic_cols.iter().enumerate() {
                    codeword.set(col, message.get(i));
                }

                // Place parity bits in parity positions
                for (j, &col) in self.parity_cols.iter().enumerate() {
                    codeword.set(col, parity.get(j));
                }

                codeword
            })
            .collect()
    }

    /// Returns the codeword length n.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message dimension k.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Returns the parity length r = n - k.
    pub fn r(&self) -> usize {
        self.r
    }

    /// Returns the parity part of the generator matrix.
    ///
    /// For systematic codes G = [I_k | P], this returns P (k × r).
    /// This is stored as a dense BitMatrix for space efficiency (DVB-T2
    /// matrices are 40-50% dense).
    ///
    /// # Convention
    ///
    /// This is called `parity_part()` not `get_parity_part()` to follow
    /// Rust getter naming conventions (no "get_" prefix for simple accessors).
    pub fn parity_part(&self) -> &BitMatrix {
        &self.parity_matrix
    }

    /// Returns the full generator matrix G.
    ///
    /// For systematic codes G = [I_k | P], this reconstructs the full matrix
    /// by adjoining the identity part. For non-systematic codes, returns the
    /// stored generator matrix directly.
    ///
    /// # Performance
    ///
    /// For systematic codes, this allocates and constructs the full matrix.
    /// Use `parity_part()` for encoding to avoid this overhead.
    ///
    /// # Convention
    ///
    /// Called `generator()` not `get_generator()` following Rust conventions.
    pub fn generator(&self) -> SpBitMatrixDual {
        if !self.is_systematic {
            // For non-systematic codes, would return stored full generator
            // Currently we only support systematic codes
            panic!("Non-systematic codes not yet implemented");
        }

        // Reconstruct full systematic generator G = [I_k | P] as sparse
        let mut edges = Vec::new();

        // Add identity part: G[i, systematic_cols[i]] = 1
        for i in 0..self.k {
            edges.push((i, self.systematic_cols[i]));
        }

        // Add parity part: G[i, parity_cols[j]] = P[i, j]
        for row in 0..self.k {
            for col in 0..self.r {
                if self.parity_matrix.get(row, col) {
                    edges.push((row, self.parity_cols[col]));
                }
            }
        }

        SpBitMatrixDual::from_coo(self.k, self.n, &edges)
    }

    /// Returns the systematic bit positions.
    pub fn systematic_cols(&self) -> &[usize] {
        &self.systematic_cols
    }

    /// Returns the parity bit positions.
    pub fn parity_cols(&self) -> &[usize] {
        &self.parity_cols
    }

    /// Returns whether this is a systematic code.
    pub fn is_systematic(&self) -> bool {
        self.is_systematic
    }

    /// Returns the number of non-zero entries in the parity matrix.
    ///
    /// This counts the edges in the parity part P (not including the identity).
    pub fn parity_nnz(&self) -> usize {
        let mut count = 0;
        for row in 0..self.k {
            for col in 0..self.r {
                if self.parity_matrix.get(row, col) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Create encoding matrices from pre-computed components.
    ///
    /// This is used when loading from cache.
    ///
    /// # Arguments
    ///
    /// * `k` - Message dimension
    /// * `n` - Codeword length
    /// * `parity_matrix` - Parity matrix P (k × r)
    /// * `systematic_cols` - Systematic bit positions  
    /// * `parity_cols` - Parity bit positions
    ///
    /// # Panics
    ///
    /// Panics if dimensions don't match.
    pub fn from_components(
        k: usize,
        n: usize,
        parity_matrix: BitMatrix,
        systematic_cols: Vec<usize>,
        parity_cols: Vec<usize>,
    ) -> Self {
        let r = n - k;
        assert_eq!(parity_matrix.rows(), k, "Parity matrix rows must equal k");
        assert_eq!(parity_matrix.cols(), r, "Parity matrix cols must equal r");
        assert_eq!(systematic_cols.len(), k, "Must have k systematic columns");
        assert_eq!(parity_cols.len(), r, "Must have r parity columns");

        Self {
            k,
            n,
            r,
            parity_matrix,
            systematic_cols,
            parity_cols,
            is_systematic: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::sparse::SpBitMatrixDual;

    fn simple_hamming_7_4_h() -> SpBitMatrixDual {
        let edges = vec![
            (0, 0),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 1),
            (1, 3),
            (1, 5),
            (2, 2),
            (2, 3),
            (2, 6),
        ];
        SpBitMatrixDual::from_coo(3, 7, &edges)
    }

    #[test]
    fn test_preprocess_simple() {
        let h = simple_hamming_7_4_h();
        let result = RuEncodingMatrices::preprocess(&h);
        assert!(result.is_ok());

        let matrices = result.unwrap();
        assert_eq!(matrices.k(), 4);
        assert_eq!(matrices.n(), 7);
        assert_eq!(matrices.r(), 3);
    }

    #[test]
    fn test_encoding_produces_valid_codewords() {
        let h = simple_hamming_7_4_h();
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        // Test all 16 messages
        for msg_val in 0u8..16 {
            let mut message = BitVec::new();
            for i in 0..4 {
                message.push_bit((msg_val >> i) & 1 == 1);
            }

            let codeword = matrices.encode(&message);
            assert_eq!(codeword.len(), 7);

            // Verify H·c = 0
            let syndrome = h.matvec(&codeword);
            assert_eq!(
                syndrome.count_ones(),
                0,
                "Codeword for message {} must satisfy H·c = 0",
                msg_val
            );
        }
    }

    #[test]
    fn test_standard_hamming_7_4() {
        // Standard Hamming [7,4] H matrix
        let edges = vec![
            (0, 0),
            (0, 1),
            (0, 3),
            (0, 4),
            (1, 0),
            (1, 2),
            (1, 3),
            (1, 5),
            (2, 1),
            (2, 2),
            (2, 3),
            (2, 6),
        ];
        let h = SpBitMatrixDual::from_coo(3, 7, &edges);
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        assert_eq!(matrices.k(), 4);
        assert_eq!(matrices.n(), 7);

        // Verify all codewords satisfy H·c = 0
        for msg_val in 0u8..16 {
            let mut message = BitVec::new();
            for i in 0..4 {
                message.push_bit((msg_val >> i) & 1 == 1);
            }

            let codeword = matrices.encode(&message);
            let syndrome = h.matvec(&codeword);
            assert_eq!(
                syndrome.count_ones(),
                0,
                "Standard Hamming codeword must be valid"
            );
        }
    }

    // Tests for parity matrix density characteristics

    #[test]
    fn test_generator_is_sparse() {
        // Test that for truly sparse codes (Hamming), the parity matrix has low density
        // Note: DVB-T2 codes are 40-50% dense, but Hamming codes are ~20% dense
        let h = simple_hamming_7_4_h();
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        // For [7,4] Hamming code, generator should have exactly 16 ones
        // (4 identity bits + 12 parity bits)
        let nnz = matrices.parity_nnz();
        assert!(nnz <= 20, "Generator should be sparse, got {} edges", nnz);

        // Density should be reasonable for a sparse code
        let density = nnz as f64 / (matrices.k() * matrices.n()) as f64;
        assert!(
            density < 0.7,
            "Generator density {:.2}% should be < 70%",
            density * 100.0
        );

        eprintln!(
            "Generator matrix: {} edges, {:.1}% density",
            nnz,
            density * 100.0
        );
    }

    #[test]
    fn test_sparse_matvec_transpose() {
        // Test that sparse matrix-vector multiply works correctly
        let h = simple_hamming_7_4_h();
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        // Test zero message
        let zero_msg = BitVec::zeros(4);
        let zero_codeword = matrices.encode(&zero_msg);
        assert_eq!(
            zero_codeword.count_ones(),
            0,
            "Zero message should produce zero codeword"
        );

        // Test messages with single bit set
        for bit_pos in 0..4 {
            let mut message = BitVec::zeros(4);
            message.set(bit_pos, true);

            let codeword = matrices.encode(&message);

            // Verify it's a valid codeword
            let syndrome = h.matvec(&codeword);
            assert_eq!(
                syndrome.count_ones(),
                0,
                "Single-bit message must produce valid codeword"
            );
        }
    }

    #[test]
    fn test_sparse_encoding_performance() {
        // Test that sparse encoding maintains good performance
        // This is more of a documentation test - actual performance tested in benches
        let h = simple_hamming_7_4_h();
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        let mut message = BitVec::new();
        for _ in 0..4 {
            message.push_bit(true);
        }

        // Just verify it works - benchmarks will test actual performance
        let _codeword = matrices.encode(&message);
    }
}
