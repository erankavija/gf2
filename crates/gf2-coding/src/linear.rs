//! Linear block codes for error correction.
//!
//! This module provides implementations of linear block codes, including systematic codes
//! and syndrome-based decoding.

use crate::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitMatrix;
use gf2_core::BitVec;
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
/// use gf2_coding::LinearBlockCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_core::BitVec;
///
/// // Create a Hamming(7,4) code
/// let code = LinearBlockCode::hamming(3);
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
    /// use gf2_coding::LinearBlockCode;
    /// use gf2_core::bitmatrix;
    ///
    /// let g = gf2_core::bitmatrix![
    ///     1, 0, 0, 0, 1, 1, 0;
    ///     0, 1, 0, 0, 1, 0, 1;
    ///     0, 0, 1, 0, 0, 1, 1;
    ///     0, 0, 0, 1, 1, 1, 1;
    /// ];
    /// let h = gf2_core::bitmatrix![
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
            assert_eq!(r, n - k, "Parity-check matrix must have r = n - k rows");
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
            let mut c_t = BitMatrix::zeros(self.n, 1);
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
    /// use gf2_coding::LinearBlockCode;
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
        let mut h = BitMatrix::zeros(r, n);

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

        let mut g = BitMatrix::zeros(k, n);

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

        assert_eq!(
            systematic_cols.len(),
            k,
            "Should have k systematic positions"
        );
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

impl crate::traits::GeneratorMatrixAccess for LinearBlockCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }

    fn generator_matrix(&self) -> BitMatrix {
        self.g.clone()
    }

    fn is_systematic(&self) -> bool {
        true // Hamming codes are always systematic
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
        let mut msg_matrix = BitMatrix::zeros(1, self.k);
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
/// use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
/// use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
/// use gf2_core::BitVec;
///
/// let code = LinearBlockCode::hamming(3);
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
#[allow(clippy::mutable_key_type)] // BitVec interior mutability doesn't affect Hash/Eq
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
    #[allow(clippy::mutable_key_type)] // BitVec interior mutability doesn't affect Hash/Eq
    pub fn new(code: LinearBlockCode) -> Self {
        assert!(
            code.h.is_some(),
            "Syndrome decoder requires a parity-check matrix"
        );

        let mut syndrome_table = HashMap::new();

        // Zero syndrome corresponds to no error (all zeros error pattern)
        let mut zero_error = BitVec::new();
        zero_error.resize(code.n, false);

        // Compute the actual zero syndrome (should be r bits all zero)
        let zero_syndrome = code.syndrome(&zero_error).expect("Code must have H matrix");
        syndrome_table.insert(zero_syndrome, zero_error);

        // For each possible single-bit error position
        for err_pos in 0..code.n {
            let mut error_pattern = BitVec::new();
            error_pattern.resize(code.n, false);
            error_pattern.set(err_pos, true);

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
        let syndrome = self
            .code
            .syndrome(received)
            .expect("Code must have H matrix");

        // Look up error pattern
        let error_pattern = self
            .syndrome_table
            .get(&syndrome)
            .cloned()
            .unwrap_or_else(|| {
                // Unknown syndrome - return zero error pattern (no correction)
                let mut zero_error = BitVec::new();
                zero_error.resize(self.code.n, false);
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
    use crate::traits::BlockEncoder;

    #[test]
    fn test_hamming_7_4_parameters() {
        let code = LinearBlockCode::hamming(3);
        assert_eq!(code.k(), 4);
        assert_eq!(code.n(), 7);
    }

    #[test]
    fn test_hamming_7_4_encode() {
        let code = LinearBlockCode::hamming(3);

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
        let code = LinearBlockCode::hamming(3);

        // Valid codeword should have zero syndrome
        let mut msg = BitVec::new();
        for bit in [true, false, true, false] {
            msg.push_bit(bit);
        }
        let codeword = code.encode(&msg);

        let syndrome = code.syndrome(&codeword).unwrap();
        assert_eq!(
            syndrome.count_ones(),
            0,
            "Valid codeword should have zero syndrome"
        );
    }

    #[test]
    fn test_hamming_7_4_syndrome_nonzero() {
        let code = LinearBlockCode::hamming(3);

        // Create a codeword and introduce an error
        let mut msg = BitVec::new();
        for bit in [true, false, true, false] {
            msg.push_bit(bit);
        }
        let mut codeword = code.encode(&msg);

        // Flip bit 2
        codeword.set(2, !codeword.get(2));

        let syndrome = code.syndrome(&codeword).unwrap();
        assert!(
            syndrome.count_ones() > 0,
            "Corrupted codeword should have non-zero syndrome"
        );
    }

    #[test]
    fn test_syndrome_decoder_no_error() {
        let code = LinearBlockCode::hamming(3);
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
        let code = LinearBlockCode::hamming(3);
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
            assert_eq!(
                decoded, msg,
                "Failed to correct error at position {}",
                err_pos
            );
        }
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
            assert_eq!(
                decoded, msg,
                "Failed to correct error at position {}",
                err_pos
            );
        }
    }

    #[test]
    fn test_project_message() {
        let code = LinearBlockCode::hamming(3);

        let mut msg = BitVec::new();
        for bit in [true, true, false, true] {
            msg.push_bit(bit);
        }

        let codeword = code.encode(&msg);
        let extracted = code.project_message(&codeword);

        assert_eq!(extracted, msg);
    }

    #[test]
    fn test_empty_message() {
        // Edge case: if we had a code with k=0, encoding should produce consistent output
        // For now, test that Hamming codes handle minimum sizes properly
        let code = LinearBlockCode::hamming(2);
        assert_eq!(code.k(), 1);
        assert_eq!(code.n(), 3);

        let mut msg = BitVec::new();
        msg.push_bit(false);
        let codeword = code.encode(&msg);
        assert_eq!(codeword.len(), 3);
    }

    #[test]
    fn test_all_zeros_message() {
        let code = LinearBlockCode::hamming(3);
        let mut msg = BitVec::new();
        msg.resize(code.k(), false);

        let codeword = code.encode(&msg);
        let syndrome = code.syndrome(&codeword).unwrap();

        assert_eq!(
            syndrome.count_ones(),
            0,
            "All-zero message should encode to all-zero codeword with zero syndrome"
        );
        assert_eq!(
            codeword.count_ones(),
            0,
            "All-zero message should produce all-zero codeword"
        );
    }

    #[test]
    fn test_all_ones_message() {
        let code = LinearBlockCode::hamming(3);
        let mut msg = BitVec::new();
        msg.resize(code.k(), true);

        let codeword = code.encode(&msg);
        let syndrome = code.syndrome(&codeword).unwrap();

        assert_eq!(
            syndrome.count_ones(),
            0,
            "Valid codeword should have zero syndrome"
        );
    }

    #[test]
    fn test_systematic_positions_hamming() {
        let code = LinearBlockCode::hamming(3);

        // For Hamming codes, systematic positions should be non-power-of-2 positions
        assert_eq!(code.systematic_positions.len(), code.k());

        // Verify that encoding preserves message bits at systematic positions
        let mut msg = BitVec::new();
        for bit in [true, false, true, true] {
            msg.push_bit(bit);
        }

        let codeword = code.encode(&msg);
        for (msg_bit_idx, &codeword_pos) in code.systematic_positions.iter().enumerate() {
            assert_eq!(
                msg.get(msg_bit_idx),
                codeword.get(codeword_pos),
                "Message bit {} should appear at systematic position {}",
                msg_bit_idx,
                codeword_pos
            );
        }
    }

    #[test]
    fn test_word_boundary_message_sizes() {
        // Test messages at word boundaries (63, 64, 65 bits)
        // Hamming(127, 120) has r=7, k=120
        let code = LinearBlockCode::hamming(7);
        assert_eq!(code.k(), 120);
        assert_eq!(code.n(), 127);

        let mut msg = BitVec::new();
        msg.resize(120, false);
        msg.set(63, true); // Set bit at word boundary
        msg.set(64, true); // Set bit just after word boundary

        let codeword = code.encode(&msg);
        assert_eq!(codeword.len(), 127);

        let syndrome = code.syndrome(&codeword).unwrap();
        assert_eq!(
            syndrome.count_ones(),
            0,
            "Valid codeword should have zero syndrome"
        );
    }

    #[test]
    #[should_panic(expected = "Message length must be k")]
    fn test_encode_wrong_message_length() {
        let code = LinearBlockCode::hamming(3);
        let mut msg = BitVec::new();
        msg.resize(5, false); // Wrong length (should be 4)

        code.encode(&msg);
    }

    #[test]
    #[should_panic(expected = "Codeword length must be n")]
    fn test_syndrome_wrong_codeword_length() {
        let code = LinearBlockCode::hamming(3);
        let mut codeword = BitVec::new();
        codeword.resize(10, false); // Wrong length (should be 7)

        code.syndrome(&codeword);
    }

    #[test]
    #[should_panic(expected = "Received vector length must be n")]
    fn test_decode_wrong_codeword_length() {
        let code = LinearBlockCode::hamming(3);
        let decoder = SyndromeTableDecoder::new(code);

        let mut received = BitVec::new();
        received.resize(5, false); // Wrong length (should be 7)

        decoder.decode(&received);
    }

    #[test]
    fn test_multiple_hamming_sizes() {
        // Test various Hamming code sizes
        for r in 2..=6 {
            let code = LinearBlockCode::hamming(r);
            let n = (1 << r) - 1;
            let k = n - r;

            assert_eq!(code.n(), n);
            assert_eq!(code.k(), k);

            // Verify dimensions of matrices
            assert_eq!(code.generator().rows(), k);
            assert_eq!(code.generator().cols(), n);

            if let Some(h) = code.parity_check() {
                assert_eq!(h.rows(), r);
                assert_eq!(h.cols(), n);
            }
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::traits::BlockEncoder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_encode_decode_roundtrip_hamming_7_4(msg_bits in prop::collection::vec(any::<bool>(), 4)) {
            let code = LinearBlockCode::hamming(3);
            let decoder = SyndromeTableDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = decoder.code().encode(&msg);
            let decoded = decoder.decode(&codeword);

            prop_assert_eq!(decoded, msg);
        }

        #[test]
        fn prop_valid_codeword_has_zero_syndrome(msg_bits in prop::collection::vec(any::<bool>(), 4)) {
            let code = LinearBlockCode::hamming(3);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = code.encode(&msg);
            let syndrome = code.syndrome(&codeword).unwrap();

            prop_assert_eq!(syndrome.count_ones(), 0, "Valid codeword must have zero syndrome");
        }

        #[test]
        fn prop_single_bit_error_correction_hamming_7_4(
            msg_bits in prop::collection::vec(any::<bool>(), 4),
            error_pos in 0usize..7
        ) {
            let code = LinearBlockCode::hamming(3);
            let decoder = SyndromeTableDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let mut received = decoder.code().encode(&msg);
            received.set(error_pos, !received.get(error_pos));

            let decoded = decoder.decode(&received);
            prop_assert_eq!(decoded, msg, "Failed to correct error at position {}", error_pos);
        }

        #[test]
        fn prop_syndrome_linearity(
            msg1_bits in prop::collection::vec(any::<bool>(), 4),
            msg2_bits in prop::collection::vec(any::<bool>(), 4)
        ) {
            let code = LinearBlockCode::hamming(3);

            let mut msg1 = BitVec::new();
            for bit in msg1_bits {
                msg1.push_bit(bit);
            }

            let mut msg2 = BitVec::new();
            for bit in msg2_bits {
                msg2.push_bit(bit);
            }

            let c1 = code.encode(&msg1);
            let c2 = code.encode(&msg2);

            // XOR the codewords (addition in GF(2))
            let mut c_sum = c1.clone();
            c_sum.bit_xor_into(&c2);

            // Syndrome should be zero since c_sum is also a valid codeword
            let syndrome = code.syndrome(&c_sum).unwrap();
            prop_assert_eq!(syndrome.count_ones(), 0, "Sum of valid codewords should be a valid codeword");
        }

        #[test]
        fn prop_project_message_preserves_data(msg_bits in prop::collection::vec(any::<bool>(), 4)) {
            let code = LinearBlockCode::hamming(3);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = code.encode(&msg);
            let extracted = code.project_message(&codeword);

            prop_assert_eq!(extracted, msg);
        }

        #[test]
        fn prop_encode_decode_roundtrip_hamming_15_11(msg_bits in prop::collection::vec(any::<bool>(), 11)) {
            let code = LinearBlockCode::hamming(4);
            let decoder = SyndromeTableDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = decoder.code().encode(&msg);
            let decoded = decoder.decode(&codeword);

            prop_assert_eq!(decoded, msg);
        }

        #[test]
        fn prop_single_bit_error_correction_hamming_15_11(
            msg_bits in prop::collection::vec(any::<bool>(), 11),
            error_pos in 0usize..15
        ) {
            let code = LinearBlockCode::hamming(4);
            let decoder = SyndromeTableDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let mut received = decoder.code().encode(&msg);
            received.set(error_pos, !received.get(error_pos));

            let decoded = decoder.decode(&received);
            prop_assert_eq!(decoded, msg, "Failed to correct error at position {}", error_pos);
        }

        #[test]
        fn prop_encode_decode_roundtrip_hamming_31_26(msg_bits in prop::collection::vec(any::<bool>(), 26)) {
            let code = LinearBlockCode::hamming(5);
            let decoder = SyndromeTableDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = decoder.code().encode(&msg);
            let decoded = decoder.decode(&codeword);

            prop_assert_eq!(decoded, msg);
        }

        #[test]
        fn prop_hamming_distance_property(
            msg_bits in prop::collection::vec(any::<bool>(), 4),
            error_pos1 in 0usize..7,
            error_pos2 in 0usize..7
        ) {
            // Hamming codes have minimum distance 3, so they can correct 1 error
            // but not necessarily 2 errors (unless they're at same position)
            let code = LinearBlockCode::hamming(3);
            let decoder = SyndromeTableDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let mut received = decoder.code().encode(&msg);

            if error_pos1 == error_pos2 {
                // Two errors at same position cancel out
                let decoded = decoder.decode(&received);
                prop_assert_eq!(decoded, msg);
            } else {
                // Two errors at different positions
                received.set(error_pos1, !received.get(error_pos1));
                received.set(error_pos2, !received.get(error_pos2));

                // Decoder may or may not correct correctly (distance 3 code)
                // We just verify it doesn't panic
                let _decoded = decoder.decode(&received);
            }
        }

        #[test]
        fn prop_systematic_encoding_preserves_message_bits(msg_bits in prop::collection::vec(any::<bool>(), 4)) {
            let code = LinearBlockCode::hamming(3);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = code.encode(&msg);

            // Check that message bits appear at systematic positions
            for (msg_idx, &sys_pos) in code.systematic_positions.iter().enumerate() {
                prop_assert_eq!(
                    msg.get(msg_idx),
                    codeword.get(sys_pos),
                    "Message bit {} must appear at systematic position {}",
                    msg_idx,
                    sys_pos
                );
            }
        }

        #[test]
        fn prop_generator_parity_orthogonality(r in 2usize..7) {
            // For any Hamming code, G * H^T should be zero
            let code = LinearBlockCode::hamming(r);

            if let Some(h) = code.parity_check() {
                let g = code.generator();
                let h_t = h.transpose();
                let product = g * &h_t;

                // Product should be all zeros (k × r zero matrix)
                for row in 0..product.rows() {
                    for col in 0..product.cols() {
                        prop_assert!(!product.get(row, col), "G * H^T must be zero at ({}, {})", row, col);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod generator_matrix_access_tests {
    use super::*;
    use crate::traits::GeneratorMatrixAccess;

    #[test]
    fn test_linear_code_generator_matrix_dimensions() {
        let code = LinearBlockCode::hamming(3);
        let g = code.generator_matrix();
        assert_eq!(g.rows(), code.k());
        assert_eq!(g.cols(), code.n());
    }

    #[test]
    fn test_linear_code_generator_equals_stored() {
        let code = LinearBlockCode::hamming(3);
        let g1 = code.generator();
        let g2 = code.generator_matrix();
        assert_eq!(g1, &g2);
    }

    #[test]
    fn test_linear_code_is_systematic() {
        let code = LinearBlockCode::hamming(3);
        assert!(code.is_systematic());
    }

    #[test]
    fn test_linear_code_generator_parity_orthogonality() {
        let code = LinearBlockCode::hamming(3);
        let g = code.generator_matrix();
        let h = code.parity_check().unwrap();

        // G·H^T = 0
        let h_t = h.transpose();
        let product = &g * &h_t;

        // Verify all entries are zero
        for i in 0..product.rows() {
            for j in 0..product.cols() {
                assert!(!product.get(i, j), "G·H^T must be zero at ({}, {})", i, j);
            }
        }
    }

    #[test]
    fn test_linear_code_multiple_sizes() {
        for r in 2..=5 {
            let code = LinearBlockCode::hamming(r);
            let g = code.generator_matrix();

            assert_eq!(g.rows(), code.k());
            assert_eq!(g.cols(), code.n());
            assert!(code.is_systematic());
        }
    }
}
