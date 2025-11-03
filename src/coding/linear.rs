//! Linear block codes for error correction.
//!
//! This module provides implementations of linear block codes, including systematic codes
//! and syndrome-based decoding.

use crate::matrix::BitMatrix;
use crate::BitVec;
use crate::coding::traits::{BlockEncoder, HardDecisionDecoder};
use std::collections::HashMap;

/// A linear block code defined by generator and parity-check matrices.
///
/// A linear [n, k] block code encodes k message bits into n codeword bits using
/// a generator matrix G (k × n). For systematic codes, the first k bits of the
/// codeword are the message bits.
///
/// # Examples
///
/// ```
/// use gf2::coding::LinearBlockCode;
/// use gf2::coding::traits::BlockEncoder;
/// use gf2::BitVec;
///
/// // Create a Hamming(7,4) code
/// let code = LinearBlockCode::hamming_7_4();
/// assert_eq!(code.k(), 4);
/// assert_eq!(code.n(), 7);
///
/// // Encode a message
/// let mut msg = BitVec::new();
/// msg.push_bit(true);
/// msg.push_bit(false);
/// msg.push_bit(true);
/// msg.push_bit(false);
/// let codeword = code.encode(&msg);
/// assert_eq!(codeword.len(), 7);
/// ```
#[derive(Debug, Clone)]
pub struct LinearBlockCode {
    /// Generator matrix G (k × n)
    g: BitMatrix,
    /// Parity-check matrix H (r × n), where r = n - k
    h: Option<BitMatrix>,
    /// Number of message bits
    k: usize,
    /// Number of codeword bits
    n: usize,
    /// Positions of systematic (message) bits in the codeword
    systematic_positions: Vec<usize>,
}

impl LinearBlockCode {
    /// Creates a new systematic linear block code.
    ///
    /// # Arguments
    ///
    /// * `g` - Generator matrix (k × n)
    /// * `h` - Optional parity-check matrix (r × n)
    ///
    /// # Panics
    ///
    /// Panics if dimensions are inconsistent or if `h` is provided but has wrong dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::coding::LinearBlockCode;
    /// use gf2::bitmatrix;
    ///
    /// let g = bitmatrix![
    ///     1, 0, 0, 0, 1, 1, 0;
    ///     0, 1, 0, 0, 1, 0, 1;
    ///     0, 0, 1, 0, 0, 1, 1;
    ///     0, 0, 0, 1, 1, 1, 1;
    /// ];
    /// let h = bitmatrix![
    ///     1, 1, 0, 1, 1, 0, 0;
    ///     1, 0, 1, 1, 0, 1, 0;
    ///     0, 1, 1, 1, 0, 0, 1;
    /// ];
    /// let code = LinearBlockCode::new_systematic(g, Some(h));
    /// assert_eq!(code.k(), 4);
    /// assert_eq!(code.n(), 7);
    /// ```
    pub fn new_systematic(g: BitMatrix, h: Option<BitMatrix>) -> Self {
        let k = g.rows();
        let n = g.cols();

        // Validate dimensions
        if let Some(ref h_matrix) = h {
            let r = h_matrix.rows();
            assert_eq!(
                h_matrix.cols(),
                n,
                "Parity-check matrix must have n columns"
            );
            assert_eq!(
                r,
                n - k,
                "Parity-check matrix must have r = n - k rows"
            );
        }

        // For systematic codes, assume message bits are in positions 0..k
        let systematic_positions = (0..k).collect();

        Self {
            g,
            h,
            k,
            n,
            systematic_positions,
        }
    }

    /// Returns the number of message bits (dimension).
    pub fn k(&self) -> usize {
        self.k
    }

    /// Returns the number of codeword bits (length).
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns a reference to the generator matrix.
    pub fn generator(&self) -> &BitMatrix {
        &self.g
    }

    /// Returns a reference to the parity-check matrix, if present.
    pub fn parity_check(&self) -> Option<&BitMatrix> {
        self.h.as_ref()
    }

    /// Extracts the message bits from a codeword at systematic positions.
    ///
    /// # Arguments
    ///
    /// * `codeword` - A codeword bit vector
    ///
    /// # Returns
    ///
    /// A bit vector containing the message bits
    ///
    /// # Panics
    ///
    /// Panics if `codeword.len() != n()`
    pub fn project_message(&self, codeword: &BitVec) -> BitVec {
        assert_eq!(
            codeword.len(),
            self.n,
            "Codeword length must be n = {}",
            self.n
        );

        let mut message = BitVec::new();
        for &pos in &self.systematic_positions {
            message.push_bit(codeword.get(pos));
        }
        message
    }

    /// Computes the syndrome of a received codeword.
    ///
    /// The syndrome is s = H * c^T where H is the parity-check matrix and c is the codeword.
    /// A zero syndrome indicates no detectable errors.
    ///
    /// # Arguments
    ///
    /// * `codeword` - The received codeword
    ///
    /// # Returns
    ///
    /// `Some(syndrome)` if parity-check matrix is available, `None` otherwise
    ///
    /// # Panics
    ///
    /// Panics if `codeword.len() != n()`
    pub fn syndrome(&self, codeword: &BitVec) -> Option<BitVec> {
        assert_eq!(
            codeword.len(),
            self.n,
            "Codeword length must be n = {}",
            self.n
        );

        self.h.as_ref().map(|h| {
            // Convert codeword to column vector (n × 1 matrix)
            let mut c_t = BitMatrix::new_zero(self.n, 1);
            for i in 0..self.n {
                c_t.set(i, 0, codeword.get(i));
            }

            // Compute s = H * c^T
            let s_matrix = h * &c_t;

            // Extract syndrome as BitVec
            let r = h.rows();
            let mut syndrome = BitVec::new();
            for i in 0..r {
                syndrome.push_bit(s_matrix.get(i, 0));
            }
            syndrome
        })
    }

    /// Creates a standard Hamming(7,4) code.
    ///
    /// This is the most common Hamming code, encoding 4 data bits into 7 bits
    /// with the ability to correct single-bit errors.
    ///
    /// Generator matrix G (4×7):
    /// ```text
    ///   ┌             ┐
    ///   │ 1 0 0 0 1 1 0 │
    ///   │ 0 1 0 0 1 0 1 │
    ///   │ 0 0 1 0 0 1 1 │
    ///   │ 0 0 0 1 1 1 1 │
    ///   └             ┘
    /// ```
    ///
    /// Parity-check matrix H (3×7):
    /// ```text
    ///   ┌             ┐
    ///   │ 1 1 0 1 1 0 0 │
    ///   │ 1 0 1 1 0 1 0 │
    ///   │ 0 1 1 1 0 0 1 │
    ///   └             ┘
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::coding::LinearBlockCode;
    ///
    /// let code = LinearBlockCode::hamming_7_4();
    /// assert_eq!(code.k(), 4);
    /// assert_eq!(code.n(), 7);
    /// ```
    pub fn hamming_7_4() -> Self {
        let g = crate::bitmatrix![
            1, 0, 0, 0, 1, 1, 0;
            0, 1, 0, 0, 1, 0, 1;
            0, 0, 1, 0, 0, 1, 1;
            0, 0, 0, 1, 1, 1, 1;
        ];

        let h = crate::bitmatrix![
            1, 1, 0, 1, 1, 0, 0;
            1, 0, 1, 1, 0, 1, 0;
            0, 1, 1, 1, 0, 0, 1;
        ];

        Self::new_systematic(g, Some(h))
    }

    /// Creates a general Hamming code with parameter r.
    ///
    /// A Hamming code with parameter r has:
    /// - n = 2^r - 1 (codeword length)
    /// - k = 2^r - r - 1 (message length)
    /// - r = n - k (parity bits, minimum distance d_min = 3)
    ///
    /// The code can correct 1 error and detect 2 errors.
    ///
    /// # Arguments
    ///
    /// * `r` - The parameter r, must be >= 2
    ///
    /// # Panics
    ///
    /// Panics if r < 2
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::coding::LinearBlockCode;
    ///
    /// // Hamming(7,4) with r=3
    /// let code = LinearBlockCode::hamming(3);
    /// assert_eq!(code.k(), 4);
    /// assert_eq!(code.n(), 7);
    ///
    /// // Hamming(15,11) with r=4
    /// let code2 = LinearBlockCode::hamming(4);
    /// assert_eq!(code2.k(), 11);
    /// assert_eq!(code2.n(), 15);
    /// ```
    pub fn hamming(r: usize) -> Self {
        assert!(r >= 2, "Hamming code parameter r must be >= 2");

        let n = (1 << r) - 1; // 2^r - 1
        let k = n - r;

        // Build parity-check matrix H (r × n)
        // H contains all non-zero binary r-tuples as columns
        let mut h = BitMatrix::new_zero(r, n);
        
        // Fill columns with binary representations of 1 to n
        for col in 0..n {
            let value = col + 1; // Use 1-indexed values
            for row in 0..r {
                if (value & (1 << row)) != 0 {
                    h.set(row, col, true);
                }
            }
        }

        // Build generator matrix G (k × n) in systematic form [I_k | P]
        // For systematic form, we need G such that G * H^T = 0
        // We construct G = [I_k | P] where the first k columns form identity
        
        let mut g = BitMatrix::new_zero(k, n);
        
        // Identify which columns of H correspond to data bits (systematic positions)
        // We want columns that are not powers of 2 (parity bit positions are at indices 2^i - 1)
        let mut systematic_cols = Vec::new();
        let mut parity_cols = Vec::new();
        
        for col in 0..n {
            let value = col + 1;
            // Check if value is a power of 2
            if value & (value - 1) == 0 {
                parity_cols.push(col);
            } else {
                systematic_cols.push(col);
            }
        }
        
        assert_eq!(systematic_cols.len(), k, "Should have k systematic positions");
        assert_eq!(parity_cols.len(), r, "Should have r parity positions");

        // Build G in systematic form
        // For each message bit position, we set the corresponding identity bit
        // and compute the parity bits
        for (msg_idx, &data_col) in systematic_cols.iter().enumerate() {
            // Set identity part: this message bit affects this data position
            g.set(msg_idx, data_col, true);
            
            // Set parity part: for each parity position, check if this data position
            // contributes to it by checking the corresponding H entry
            for (parity_idx, &parity_col) in parity_cols.iter().enumerate() {
                // If H[parity_idx, data_col] = 1, then this message bit contributes to this parity
                if h.get(parity_idx, data_col) {
                    g.set(msg_idx, parity_col, true);
                }
            }
        }

        // Create with explicit systematic positions
        Self {
            g,
            h: Some(h),
            k,
            n,
            systematic_positions: systematic_cols,
        }
    }
}

impl BlockEncoder for LinearBlockCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.k,
            "Message length must be k = {}",
            self.k
        );

        // Convert message to 1 × k matrix
        let mut msg_matrix = BitMatrix::new_zero(1, self.k);
        for i in 0..self.k {
            msg_matrix.set(0, i, message.get(i));
        }

        // Compute codeword = message * G
        let codeword_matrix = &msg_matrix * &self.g;

        // Extract codeword as BitVec
        let mut codeword = BitVec::new();
        for i in 0..self.n {
            codeword.push_bit(codeword_matrix.get(0, i));
        }
        codeword
    }
}

/// Syndrome-table decoder for linear block codes.
///
/// This decoder uses a precomputed lookup table mapping syndromes to error patterns.
/// It's efficient for codes with small syndrome spaces (small r = n - k).
///
/// # Examples
///
/// ```
/// use gf2::coding::{LinearBlockCode, SyndromeTableDecoder};
/// use gf2::coding::traits::{BlockEncoder, HardDecisionDecoder};
/// use gf2::BitVec;
///
/// let code = LinearBlockCode::hamming_7_4();
/// let decoder = SyndromeTableDecoder::new(code);
///
/// // Encode a message
/// let mut msg = BitVec::new();
/// for bit in [true, false, true, false] {
///     msg.push_bit(bit);
/// }
/// let codeword = decoder.code().encode(&msg);
///
/// // Introduce an error
/// let mut received = codeword.clone();
/// received.set(2, !received.get(2));
///
/// // Decode and correct
/// let decoded = decoder.decode(&received);
/// assert_eq!(decoded, msg);
/// ```
#[derive(Debug, Clone)]
pub struct SyndromeTableDecoder {
    code: LinearBlockCode,
    syndrome_table: HashMap<BitVec, BitVec>,
}

impl SyndromeTableDecoder {
    /// Creates a new syndrome-table decoder for the given code.
    ///
    /// This builds a lookup table for single-error patterns. For each possible
    /// single-bit error position, it computes the corresponding syndrome.
    ///
    /// # Arguments
    ///
    /// * `code` - The linear block code to decode
    ///
    /// # Panics
    ///
    /// Panics if the code does not have a parity-check matrix
    pub fn new(code: LinearBlockCode) -> Self {
        assert!(
            code.h.is_some(),
            "Syndrome decoder requires a parity-check matrix"
        );

        let mut syndrome_table = HashMap::new();

        // Zero syndrome corresponds to no error
        let zero_syndrome = BitVec::new();
        let mut zero_error = BitVec::new();
        for _ in 0..code.n {
            zero_error.push_bit(false);
        }
        syndrome_table.insert(zero_syndrome, zero_error);

        // For each possible single-bit error position
        for err_pos in 0..code.n {
            let mut error_pattern = BitVec::new();
            for i in 0..code.n {
                error_pattern.push_bit(i == err_pos);
            }

            // Compute syndrome for this error pattern
            if let Some(syndrome) = code.syndrome(&error_pattern) {
                syndrome_table.insert(syndrome, error_pattern);
            }
        }

        Self {
            code,
            syndrome_table,
        }
    }

    /// Returns a reference to the underlying code.
    pub fn code(&self) -> &LinearBlockCode {
        &self.code
    }
}

impl HardDecisionDecoder for SyndromeTableDecoder {
    fn decode(&self, received: &BitVec) -> BitVec {
        assert_eq!(
            received.len(),
            self.code.n,
            "Received vector length must be n = {}",
            self.code.n
        );

        // Compute syndrome
        let syndrome = self.code.syndrome(received).expect("Code must have H matrix");

        // Look up error pattern
        let error_pattern = self
            .syndrome_table
            .get(&syndrome)
            .cloned()
            .unwrap_or_else(|| {
                // Unknown syndrome - return zero error pattern (no correction)
                let mut zero_error = BitVec::new();
                for _ in 0..self.code.n {
                    zero_error.push_bit(false);
                }
                zero_error
            });

        // Correct the received vector: corrected = received XOR error_pattern
        let mut corrected = received.clone();
        corrected.bit_xor_into(&error_pattern);

        // Extract message bits
        self.code.project_message(&corrected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::traits::BlockEncoder;

    #[test]
    fn test_hamming_7_4_parameters() {
        let code = LinearBlockCode::hamming_7_4();
        assert_eq!(code.k(), 4);
        assert_eq!(code.n(), 7);
    }

    #[test]
    fn test_hamming_7_4_encode() {
        let code = LinearBlockCode::hamming_7_4();

        // Test encoding [1,0,1,0]
        let mut msg = BitVec::new();
        msg.push_bit(true);
        msg.push_bit(false);
        msg.push_bit(true);
        msg.push_bit(false);

        let codeword = code.encode(&msg);
        assert_eq!(codeword.len(), 7);
    }

    #[test]
    fn test_hamming_7_4_syndrome_zero() {
        let code = LinearBlockCode::hamming_7_4();

        // Valid codeword should have zero syndrome
        let mut msg = BitVec::new();
        for bit in [true, false, true, false] {
            msg.push_bit(bit);
        }
        let codeword = code.encode(&msg);

        let syndrome = code.syndrome(&codeword).unwrap();
        assert_eq!(syndrome.count_ones(), 0, "Valid codeword should have zero syndrome");
    }

    #[test]
    fn test_hamming_7_4_syndrome_nonzero() {
        let code = LinearBlockCode::hamming_7_4();

        // Create a codeword and introduce an error
        let mut msg = BitVec::new();
        for bit in [true, false, true, false] {
            msg.push_bit(bit);
        }
        let mut codeword = code.encode(&msg);

        // Flip bit 2
        codeword.set(2, !codeword.get(2));

        let syndrome = code.syndrome(&codeword).unwrap();
        assert!(syndrome.count_ones() > 0, "Corrupted codeword should have non-zero syndrome");
    }

    #[test]
    fn test_syndrome_decoder_no_error() {
        let code = LinearBlockCode::hamming_7_4();
        let decoder = SyndromeTableDecoder::new(code);

        let mut msg = BitVec::new();
        for bit in [true, false, true, false] {
            msg.push_bit(bit);
        }

        let codeword = decoder.code().encode(&msg);
        let decoded = decoder.decode(&codeword);

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_syndrome_decoder_single_error() {
        let code = LinearBlockCode::hamming_7_4();
        let decoder = SyndromeTableDecoder::new(code);

        let mut msg = BitVec::new();
        for bit in [true, false, true, false] {
            msg.push_bit(bit);
        }

        let received = decoder.code().encode(&msg);
        
        // Test error correction at each position
        for err_pos in 0..7 {
            let mut corrupted = received.clone();
            corrupted.set(err_pos, !corrupted.get(err_pos));
            
            let decoded = decoder.decode(&corrupted);
            assert_eq!(decoded, msg, "Failed to correct error at position {}", err_pos);
        }
    }

    #[test]
    fn test_hamming_general_7_4() {
        // Verify that hamming(3) produces a (7,4) code
        let code = LinearBlockCode::hamming(3);
        assert_eq!(code.n(), 7);
        assert_eq!(code.k(), 4);
    }

    #[test]
    fn test_hamming_general_15_11() {
        // Test Hamming(15,11) with r=4
        let code = LinearBlockCode::hamming(4);
        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);

        // Test encoding
        let mut msg = BitVec::new();
        for i in 0..11 {
            msg.push_bit(i % 2 == 0);
        }
        
        let codeword = code.encode(&msg);
        assert_eq!(codeword.len(), 15);

        // Valid codeword should have zero syndrome
        let syndrome = code.syndrome(&codeword).unwrap();
        assert_eq!(syndrome.count_ones(), 0);
    }

    #[test]
    fn test_hamming_general_31_26() {
        // Test Hamming(31,26) with r=5
        let code = LinearBlockCode::hamming(5);
        assert_eq!(code.n(), 31);
        assert_eq!(code.k(), 26);

        // Test encoding
        let mut msg = BitVec::new();
        for i in 0..26 {
            msg.push_bit(i % 3 == 0);
        }
        
        let codeword = code.encode(&msg);
        assert_eq!(codeword.len(), 31);

        // Valid codeword should have zero syndrome
        let syndrome = code.syndrome(&codeword).unwrap();
        assert_eq!(syndrome.count_ones(), 0);
    }

    #[test]
    fn test_hamming_general_decoder_15_11() {
        let code = LinearBlockCode::hamming(4);
        let decoder = SyndromeTableDecoder::new(code);

        let mut msg = BitVec::new();
        for i in 0..11 {
            msg.push_bit(i % 2 == 1);
        }

        let codeword = decoder.code().encode(&msg);
        
        // Test correction at a few positions
        for err_pos in [0, 5, 10, 14] {
            let mut corrupted = codeword.clone();
            corrupted.set(err_pos, !corrupted.get(err_pos));
            
            let decoded = decoder.decode(&corrupted);
            assert_eq!(decoded, msg, "Failed to correct error at position {}", err_pos);
        }
    }

    #[test]
    fn test_project_message() {
        let code = LinearBlockCode::hamming_7_4();
        
        let mut msg = BitVec::new();
        for bit in [true, true, false, true] {
            msg.push_bit(bit);
        }

        let codeword = code.encode(&msg);
        let extracted = code.project_message(&codeword);
        
        assert_eq!(extracted, msg);
    }
}
