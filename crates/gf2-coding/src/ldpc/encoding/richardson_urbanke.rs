//! Systematic LDPC encoding from a dense parity matrix.
//!
//! [`RuEncodingMatrices::preprocess`] row-reduces H (m × n) to select m parity
//! columns and stores the parity part P of G = [I_k | P]; encoding computes
//! `parity = P^T × message`.

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

/// Preprocessed systematic-encoding matrices of one LDPC code.
///
/// For G = [I_k | P] only the parity part P is stored, as a dense
/// [`BitMatrix`]; `generator()` reconstructs G by adjoining the identity.
#[derive(Debug, Clone)]
pub struct RuEncodingMatrices {
    /// Message dimension k
    k: usize,
    /// Codeword length n
    n: usize,
    /// Parity length r = n - k
    r: usize,
    /// Parity matrix P (k × r): parity = P^T × message
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
    /// Computes the parity part P of G = [I_k | P] from the parity-check
    /// matrix `h` (m × n), which is densified for the row reduction.
    ///
    /// # Errors
    ///
    /// [`PreprocessError::InvalidDimensions`] if `m == 0`, `n == 0` or
    /// `m >= n`; [`PreprocessError::RankDeficient`] if the rank of `h` is not `m`.
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

        let mut h_dense = BitMatrix::zeros(m, n);
        for row in 0..m {
            for col_idx in h.row_iter(row) {
                h_dense.set(row, col_idx, true);
            }
        }

        eprintln!("  Running Gaussian elimination...");

        Self::compute_generator_matrix(&h_dense, k, n)
    }

    /// Computes G with H·G^T = 0: RREF with right-to-left pivoting selects m
    /// independent parity columns, and G = [I_k | P] with
    /// P[i,j] = H_work[row_order[j], message_cols[i]].
    fn compute_generator_matrix(
        h: &BitMatrix,
        k: usize,
        n: usize,
    ) -> Result<Self, PreprocessError> {
        let m = h.rows();

        // pivot_from_right=true to prefer parity bits on right
        eprintln!("  Running RREF (word-level + SIMD)...");
        let rref_result = rref(h, true);

        if rref_result.rank != m {
            return Err(PreprocessError::RankDeficient);
        }

        let parity_cols = rref_result.pivot_cols;
        let h_work = rref_result.reduced;
        eprintln!("  RREF complete (rank = {})", rref_result.rank);

        let mut message_cols = Vec::new();
        for col in 0..n {
            if !parity_cols.contains(&col) {
                message_cols.push(col);
            }
        }

        if message_cols.len() != k {
            return Err(PreprocessError::GaussianEliminationFailed);
        }

        // rref(h, true) reorders rows so that row i has its pivot at
        // parity_cols[i], so the identity permutation is the row order.
        let row_order: Vec<usize> = (0..m).collect();

        #[cfg(debug_assertions)]
        {
            for (i, &pcol) in parity_cols.iter().enumerate() {
                debug_assert!(
                    h_work.get(i, pcol),
                    "RREF invariant violated: row {} has no 1 at pivot col {}",
                    i,
                    pcol
                );
            }
        }

        // From H·G^T = 0: P[i, j] = H_work[row_order[j], message_cols[i]]
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

    /// Encodes `message` (length k) into the length-n codeword that carries the
    /// message at the systematic positions and P^T × message at the parity
    /// positions.
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

        let parity = self.parity_matrix.matvec_transpose(message);

        let mut codeword = BitVec::zeros(self.n);

        for (i, &col) in self.systematic_cols.iter().enumerate() {
            codeword.set(col, message.get(i));
        }

        for (j, &col) in self.parity_cols.iter().enumerate() {
            codeword.set(col, parity.get(j));
        }

        codeword
    }

    /// Encodes each message as [`Self::encode`] does, computing the parities
    /// through `backend.batch_matvec_transpose`.
    ///
    /// # Panics
    ///
    /// Panics if any message length doesn't equal k.
    pub fn encode_batch(
        &self,
        messages: &[BitVec],
        backend: &dyn gf2_core::compute::ComputeBackend,
    ) -> Vec<BitVec> {
        for msg in messages {
            assert_eq!(msg.len(), self.k, "Message length must be k = {}", self.k);
        }

        if messages.is_empty() {
            return vec![];
        }

        let parities = backend.batch_matvec_transpose(&self.parity_matrix, messages);

        messages
            .iter()
            .zip(parities.iter())
            .map(|(message, parity)| {
                let mut codeword = BitVec::zeros(self.n);

                for (i, &col) in self.systematic_cols.iter().enumerate() {
                    codeword.set(col, message.get(i));
                }

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

    /// Returns the parity part P (k × r) of G = [I_k | P].
    pub fn parity_part(&self) -> &BitMatrix {
        &self.parity_matrix
    }

    /// Returns the full generator matrix G = [I_k | P], constructed and
    /// allocated on each call.
    pub fn generator(&self) -> SpBitMatrixDual {
        if !self.is_systematic {
            // Both constructors produce systematic codes.
            panic!("Non-systematic codes not yet implemented");
        }

        let mut edges = Vec::new();

        for i in 0..self.k {
            edges.push((i, self.systematic_cols[i]));
        }

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

    /// Returns the number of non-zero entries in the parity part P.
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

    /// Create encoding matrices from pre-computed components, as when loading
    /// from a cache.
    ///
    /// # Panics
    ///
    /// Panics if `n < k`, if `parity_matrix` is not k × (n − k), or if
    /// `systematic_cols` and `parity_cols` do not have lengths k and n − k.
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

        for msg_val in 0u8..16 {
            let mut message = BitVec::new();
            for i in 0..4 {
                message.push_bit((msg_val >> i) & 1 == 1);
            }

            let codeword = matrices.encode(&message);
            assert_eq!(codeword.len(), 7);

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

    #[test]
    fn test_generator_is_sparse() {
        let h = simple_hamming_7_4_h();
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        let nnz = matrices.parity_nnz();
        assert!(nnz <= 20, "Generator should be sparse, got {} edges", nnz);

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
        let h = simple_hamming_7_4_h();
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        let zero_msg = BitVec::zeros(4);
        let zero_codeword = matrices.encode(&zero_msg);
        assert_eq!(
            zero_codeword.count_ones(),
            0,
            "Zero message should produce zero codeword"
        );

        for bit_pos in 0..4 {
            let mut message = BitVec::zeros(4);
            message.set(bit_pos, true);

            let codeword = matrices.encode(&message);

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
        let h = simple_hamming_7_4_h();
        let matrices = RuEncodingMatrices::preprocess(&h).unwrap();

        let mut message = BitVec::new();
        for _ in 0..4 {
            message.push_bit(true);
        }

        let _codeword = matrices.encode(&message);
    }
}
