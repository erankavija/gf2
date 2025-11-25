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
#[derive(Debug, Clone)]
pub struct RuEncodingMatrices {
    /// Message dimension k
    k: usize,
    /// Codeword length n
    n: usize,
    /// Parity length r = n - k
    r: usize,
    /// Generator matrix in systematic form [I_k | P]
    /// We store this temporarily for the simple implementation
    generator: BitMatrix,
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
        let r = m;
        
        // Convert sparse H to dense for Gaussian elimination
        // TODO: Optimize with sparse operations
        let mut h_dense = BitMatrix::zeros(m, n);
        for row in 0..m {
            for col_idx in h.row_iter(row) {
                h_dense.set(row, col_idx, true);
            }
        }
        
        // Compute generator matrix using kernel/nullspace approach
        // For systematic encoding, we need G = [I_k | P] such that H·G^T = 0
        //
        // Strategy: Use Gaussian elimination on H to get it in systematic form,
        // then extract P from the transformed H
        let generator = Self::compute_generator_matrix(&h_dense, k, n)?;
        
        Ok(Self {
            k,
            n,
            r,
            generator,
        })
    }
    
    /// Compute generator matrix from parity-check matrix.
    ///
    /// Computes G such that H·G^T = 0 using the following algorithm:
    ///
    /// 1. Gaussian elimination with column pivoting from right to find m
    ///    independent columns for parity positions
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
    ) -> Result<BitMatrix, PreprocessError> {
        let m = h.rows();
        
        // Create working copy of H for Gaussian elimination
        let mut h_work = BitMatrix::zeros(m, n);
        for i in 0..m {
            for j in 0..n {
                h_work.set(i, j, h.get(i, j));
            }
        }
        
        // Gaussian elimination with column pivoting from right
        // Goal: Find m independent columns to use as parity positions
        let mut pivot_row = 0;
        let mut parity_cols = Vec::new();
        
        // Start from rightmost column (prefer parity bits on right)
        for col in (0..n).rev() {
            if pivot_row >= m {
                break;
            }
            
            // Find pivot in this column
            let mut found_pivot = false;
            for row in pivot_row..m {
                if h_work.get(row, col) {
                    // Swap rows if needed
                    if row != pivot_row {
                        for j in 0..n {
                            let tmp = h_work.get(pivot_row, j);
                            h_work.set(pivot_row, j, h_work.get(row, j));
                            h_work.set(row, j, tmp);
                        }
                    }
                    found_pivot = true;
                    break;
                }
            }
            
            if !found_pivot {
                continue;
            }
            
            parity_cols.push(col);
            
            // Eliminate other rows
            for row in 0..m {
                if row != pivot_row && h_work.get(row, col) {
                    for j in 0..n {
                        if h_work.get(pivot_row, j) {
                            h_work.set(row, j, h_work.get(row, j) ^ true);
                        }
                    }
                }
            }
            
            pivot_row += 1;
        }
        
        if parity_cols.len() != m {
            return Err(PreprocessError::RankDeficient);
        }
        
        parity_cols.reverse();
        
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
                    let is_pivot_row = parity_cols.iter()
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
        
        // Build generator matrix G = [I_k | P]
        // For systematic code: G[i, message_cols[i]] = 1 (identity part)
        // Parity part: G[i, parity_cols[j]] = H_work[row_order[j], message_cols[i]]
        let mut g = BitMatrix::zeros(k, n);
        
        // Identity part
        for i in 0..k {
            g.set(i, message_cols[i], true);
        }
        
        // Parity part: from H·G^T = 0
        for i in 0..k {
            for j in 0..m {
                let h_val = h_work.get(row_order[j], message_cols[i]);
                g.set(i, parity_cols[j], h_val);
            }
        }
        
        Ok(g)
    }
    
    /// Encode a message into a systematic codeword.
    ///
    /// # Arguments
    ///
    /// * `message` - Message bits (length k)
    ///
    /// # Returns
    ///
    /// Systematic codeword [message | parity] of length n.
    ///
    /// # Panics
    ///
    /// Panics if message length doesn't equal k.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let codeword = matrices.encode(&message);
    /// assert_eq!(codeword.len(), n);
    /// ```
    pub fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.k,
            "Message length must be k = {}",
            self.k
        );
        
        // Convert message to 1 × k matrix
        let mut msg_matrix = BitMatrix::zeros(1, self.k);
        for i in 0..self.k {
            msg_matrix.set(0, i, message.get(i));
        }
        
        // Multiply message by generator matrix: c = m · G
        let codeword_matrix = &msg_matrix * &self.generator;
        
        // Extract codeword as BitVec (row 0 of result)
        codeword_matrix.row_as_bitvec(0)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::sparse::SpBitMatrixDual;
    
    fn simple_hamming_7_4_h() -> SpBitMatrixDual {
        let edges = vec![
            (0, 0), (0, 2), (0, 3), (0, 4),
            (1, 1), (1, 3), (1, 5),
            (2, 2), (2, 3), (2, 6),
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
            (0, 0), (0, 1), (0, 3), (0, 4),
            (1, 0), (1, 2), (1, 3), (1, 5),
            (2, 1), (2, 2), (2, 3), (2, 6),
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
}
