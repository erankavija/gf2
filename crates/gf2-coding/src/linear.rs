//! Binary linear block codes with syndrome-table decoding.

use crate::error::{CodeCapability, CodeError};
use crate::traits::{block, HardDecisionDecoder};
use gf2_core::gfp::Fp;
use gf2_core::BitMatrix;
use gf2_core::BitVec;
use std::collections::HashMap;

/// A binary linear [n, k] block code with generator matrix G (k × n) and an
/// optional parity-check matrix H.
#[derive(Debug, Clone)]
pub struct LinearBlockCode {
    /// Generator matrix G (k × n)
    g: BitMatrix,
    /// Parity-check matrix H (r × n), where r = n - k
    h: Option<BitMatrix>,
    k: usize,
    n: usize,
    systematic_positions: Vec<usize>,
}

impl LinearBlockCode {
    /// Creates a code whose message bits occupy codeword positions `0..k`.
    ///
    /// # Panics
    ///
    /// Panics if `h` is provided and is not `(n - k) × n`.
    pub fn new_systematic(g: BitMatrix, h: Option<BitMatrix>) -> Self {
        let k = g.rows();
        let n = g.cols();

        if let Some(ref h_matrix) = h {
            let r = h_matrix.rows();
            assert_eq!(
                h_matrix.cols(),
                n,
                "Parity-check matrix must have n columns"
            );
            assert_eq!(r, n - k, "Parity-check matrix must have r = n - k rows");
        }

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

    /// The generator matrix G (k × n).
    pub fn generator(&self) -> &BitMatrix {
        &self.g
    }

    /// The parity-check matrix H ((n - k) × n), if the code has one.
    pub fn parity_check(&self) -> Option<&BitMatrix> {
        self.h.as_ref()
    }

    /// Extracts the message bits from a codeword at systematic positions.
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

    /// Computes the syndrome s = H * c^T, or `None` when the code has no
    /// parity-check matrix.
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
            let mut c_t = BitMatrix::zeros(self.n, 1);
            for i in 0..self.n {
                c_t.set(i, 0, codeword.get(i));
            }

            let s_matrix = h * &c_t;

            s_matrix.col_as_bitvec(0)
        })
    }

    /// Creates the Hamming code with n = 2^r - 1, k = n - r and d_min = 3.
    ///
    /// Column `c` of H is the binary representation of `c + 1`; message bits
    /// occupy the columns whose value is not a power of two.
    ///
    /// # Panics
    ///
    /// Panics if r < 2
    pub fn hamming(r: usize) -> Self {
        assert!(r >= 2, "Hamming code parameter r must be >= 2");

        let n = (1 << r) - 1;
        let k = n - r;

        let mut h = BitMatrix::zeros(r, n);

        for col in 0..n {
            let value = col + 1;
            for row in 0..r {
                if (value & (1 << row)) != 0 {
                    h.set(row, col, true);
                }
            }
        }

        let mut g = BitMatrix::zeros(k, n);

        let mut systematic_cols = Vec::new();
        let mut parity_cols = Vec::new();

        for col in 0..n {
            let value = col + 1;
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

        for (msg_idx, &data_col) in systematic_cols.iter().enumerate() {
            g.set(msg_idx, data_col, true);

            for (parity_idx, &parity_col) in parity_cols.iter().enumerate() {
                if h.get(parity_idx, data_col) {
                    g.set(msg_idx, parity_col, true);
                }
            }
        }

        Self {
            g,
            h: Some(h),
            k,
            n,
            systematic_positions: systematic_cols,
        }
    }
}

impl block::BlockCode for LinearBlockCode {
    type Symbol = Fp<2>;
    type Symbols = BitVec;

    fn symbol_zero(&self) -> Self::Symbol {
        Fp::<2>::new(0)
    }

    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }
}

impl block::BlockEncoder for LinearBlockCode {
    /// Writes `codeword = message · G`, overwriting every position of
    /// `codeword`, without heap allocation.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] when `message` does not
    /// hold `k` symbols or `codeword` does not hold `n` symbols.
    ///
    /// # Complexity
    ///
    /// O(k · n / 64) word operations and `n` bit writes.
    fn encode_into(
        &self,
        message: &Self::Symbols,
        codeword: &mut Self::Symbols,
    ) -> Result<(), CodeError> {
        if message.len() != self.k {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.k,
                actual: message.len(),
            });
        }
        if codeword.len() != self.n {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.n,
                actual: codeword.len(),
            });
        }

        for word in 0..self.n.div_ceil(64) {
            let mut accumulator = 0u64;
            for row in 0..self.k {
                if message.get(row) {
                    accumulator ^= self.g.row_words(row)[word];
                }
            }

            let base = word * 64;
            for offset in 0..(self.n - base).min(64) {
                codeword.set(base + offset, (accumulator >> offset) & 1 == 1);
            }
        }
        Ok(())
    }
}

impl block::GeneratorMatrixAccess for LinearBlockCode {
    type GeneratorMatrix = BitMatrix;

    /// Copies the stored generator into `out`.
    ///
    /// # Complexity
    ///
    /// O(k · n / 64) word copies.
    fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
        if out.rows() != self.k || out.cols() != self.n {
            return Err(CodeError::ShapeMismatch {
                expected_rows: self.k,
                expected_cols: self.n,
                actual_rows: out.rows(),
                actual_cols: out.cols(),
            });
        }
        for row in 0..self.k {
            out.row_words_mut(row)
                .copy_from_slice(self.g.row_words(row));
        }
        Ok(())
    }

    /// Tests the generator restricted to the code's message coordinates.
    ///
    /// The message coordinates are the positions recorded in
    /// `systematic_positions`, in message order, so the answer stays `true`
    /// for a code whose identity block sits away from columns `0..k`.
    ///
    /// # Complexity
    ///
    /// O(k²) bit reads.
    fn is_systematic(&self) -> Result<bool, CodeError> {
        if self.g.rows() != self.k
            || self.g.cols() != self.n
            || self.systematic_positions.len() != self.k
        {
            return Ok(false);
        }
        for row in 0..self.k {
            for (message_index, &position) in self.systematic_positions.iter().enumerate() {
                if position >= self.n || self.g.get(row, position) != (row == message_index) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Reports whether the recorded message coordinates are `0..k()`.
    ///
    /// [`Self::new_systematic`] records that order, while [`Self::hamming`]
    /// records the columns of `H` that are not powers of two, so a Hamming
    /// code answers `false` here and `true` from
    /// [`is_systematic`](block::GeneratorMatrixAccess::is_systematic).
    ///
    /// # Complexity
    ///
    /// O(k) position reads.
    fn has_canonical_message_order(&self) -> Result<bool, CodeError> {
        Ok(self.systematic_positions.iter().copied().eq(0..self.k))
    }
}

impl block::ParityCheckMatrixAccess for LinearBlockCode {
    type ParityCheckMatrix = BitMatrix;

    /// Copies the stored parity-check matrix into `out`.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::CapabilityUnavailable`] for a code constructed
    /// without a parity-check matrix, and [`CodeError::ShapeMismatch`] when
    /// `out` is not `parity_check_rows() × n()`.
    ///
    /// # Complexity
    ///
    /// O(r · n / 64) word copies, one packed word slice per row.
    fn parity_check_matrix_into(&self, out: &mut Self::ParityCheckMatrix) -> Result<(), CodeError> {
        let h = self.h.as_ref().ok_or(CodeError::CapabilityUnavailable {
            capability: CodeCapability::ParityCheckMatrix,
        })?;
        if out.rows() != self.parity_check_rows() || out.cols() != self.n {
            return Err(CodeError::ShapeMismatch {
                expected_rows: self.parity_check_rows(),
                expected_cols: self.n,
                actual_rows: out.rows(),
                actual_cols: out.cols(),
            });
        }
        for row in 0..h.rows() {
            out.row_words_mut(row).copy_from_slice(h.row_words(row));
        }
        Ok(())
    }
}

/// Hard-decision decoder that corrects single-bit errors through a
/// syndrome-to-error-pattern table; a received word with any other syndrome
/// is left uncorrected.
#[derive(Debug, Clone)]
#[allow(clippy::mutable_key_type)] // BitVec interior mutability doesn't affect Hash/Eq
pub struct SyndromeTableDecoder {
    code: LinearBlockCode,
    syndrome_table: HashMap<BitVec, BitVec>,
}

impl SyndromeTableDecoder {
    /// Builds the table of the zero and all single-bit error patterns.
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

        let mut zero_error = BitVec::new();
        zero_error.resize(code.n, false);

        let zero_syndrome = code.syndrome(&zero_error).expect("Code must have H matrix");
        syndrome_table.insert(zero_syndrome, zero_error);

        for err_pos in 0..code.n {
            let mut error_pattern = BitVec::new();
            error_pattern.resize(code.n, false);
            error_pattern.set(err_pos, true);

            if let Some(syndrome) = code.syndrome(&error_pattern) {
                syndrome_table.insert(syndrome, error_pattern);
            }
        }

        Self {
            code,
            syndrome_table,
        }
    }

    /// The code whose zero and single-error syndromes fill the table.
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

        let syndrome = self
            .code
            .syndrome(received)
            .expect("Code must have H matrix");

        let error_pattern = self
            .syndrome_table
            .get(&syndrome)
            .cloned()
            .unwrap_or_else(|| {
                let mut zero_error = BitVec::new();
                zero_error.resize(self.code.n, false);
                zero_error
            });

        let mut corrected = received.clone();
        corrected.bit_xor_into(&error_pattern);

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

        let mut msg = BitVec::new();
        for bit in [true, false, true, false] {
            msg.push_bit(bit);
        }
        let mut codeword = code.encode(&msg);

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
        let code = LinearBlockCode::hamming(4);
        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);

        let mut msg = BitVec::new();
        for i in 0..11 {
            msg.push_bit(i % 2 == 0);
        }

        let codeword = code.encode(&msg);
        assert_eq!(codeword.len(), 15);

        let syndrome = code.syndrome(&codeword).unwrap();
        assert_eq!(syndrome.count_ones(), 0);
    }

    #[test]
    fn test_hamming_general_31_26() {
        let code = LinearBlockCode::hamming(5);
        assert_eq!(code.n(), 31);
        assert_eq!(code.k(), 26);

        let mut msg = BitVec::new();
        for i in 0..26 {
            msg.push_bit(i % 3 == 0);
        }

        let codeword = code.encode(&msg);
        assert_eq!(codeword.len(), 31);

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
        // r = 2 is the smallest Hamming code.
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

        assert_eq!(code.systematic_positions.len(), code.k());

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
        let code = LinearBlockCode::hamming(7);
        assert_eq!(code.k(), 120);
        assert_eq!(code.n(), 127);

        let mut msg = BitVec::new();
        msg.resize(120, false);
        msg.set(63, true);
        msg.set(64, true);

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
        msg.resize(5, false);

        code.encode(&msg);
    }

    #[test]
    #[should_panic(expected = "Codeword length must be n")]
    fn test_syndrome_wrong_codeword_length() {
        let code = LinearBlockCode::hamming(3);
        let mut codeword = BitVec::new();
        codeword.resize(10, false);

        code.syndrome(&codeword);
    }

    #[test]
    #[should_panic(expected = "Received vector length must be n")]
    fn test_decode_wrong_codeword_length() {
        let code = LinearBlockCode::hamming(3);
        let decoder = SyndromeTableDecoder::new(code);

        let mut received = BitVec::new();
        received.resize(5, false);

        decoder.decode(&received);
    }

    #[test]
    fn test_multiple_hamming_sizes() {
        for r in 2..=6 {
            let code = LinearBlockCode::hamming(r);
            let n = (1 << r) - 1;
            let k = n - r;

            assert_eq!(code.n(), n);
            assert_eq!(code.k(), k);

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

            let mut c_sum = c1.clone();
            c_sum.bit_xor_into(&c2);

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
            let code = LinearBlockCode::hamming(3);
            let decoder = SyndromeTableDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let mut received = decoder.code().encode(&msg);

            if error_pos1 == error_pos2 {
                let decoded = decoder.decode(&received);
                prop_assert_eq!(decoded, msg);
            } else {
                received.set(error_pos1, !received.get(error_pos1));
                received.set(error_pos2, !received.get(error_pos2));

                // Two errors exceed the correction radius; only absence of a panic is checked.
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
            let code = LinearBlockCode::hamming(r);

            if let Some(h) = code.parity_check() {
                let g = code.generator();
                let h_t = h.transpose();
                let product = g * &h_t;

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

        let h_t = h.transpose();
        let product = &g * &h_t;

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

#[cfg(test)]
mod canonical_traits_tests {
    use super::*;
    use crate::traits::block;
    use crate::traits::block::conformance;
    use crate::traits::{BlockEncoder as V1BlockEncoder, GeneratorMatrixAccess as V1Generator};

    /// Encodes through the textbook `message · G` matrix product, which is
    /// independent of the packed word accumulation inside `encode_into`.
    fn reference_encode(code: &LinearBlockCode, message: &BitVec) -> BitVec {
        let mut message_matrix = BitMatrix::zeros(1, code.k());
        for index in 0..code.k() {
            message_matrix.set(0, index, message.get(index));
        }
        (&message_matrix * code.generator()).row_as_bitvec(0)
    }

    /// Messages covering the empty, full, single-position, word-boundary, and
    /// alternating patterns for a `k`-bit message.
    fn probe_messages(k: usize) -> Vec<BitVec> {
        let mut messages = vec![BitVec::zeros(k)];

        let mut all_ones = BitVec::zeros(k);
        let mut alternating = BitVec::zeros(k);
        for index in 0..k {
            all_ones.set(index, true);
            alternating.set(index, index % 2 == 0);
        }
        messages.push(all_ones);
        messages.push(alternating);

        for index in [0, 63, 64, 65, k / 2, k - 1] {
            if index < k {
                let mut single = BitVec::zeros(k);
                single.set(index, true);
                messages.push(single);
            }
        }
        messages
    }

    #[test]
    fn linear_code_encode_into_matches_matrix_product_reference() {
        for r in 2..=7 {
            let code = LinearBlockCode::hamming(r);
            for message in probe_messages(code.k()) {
                let encoded = block::BlockEncoder::encode(&code, &message).unwrap();
                assert_eq!(encoded, reference_encode(&code, &message), "r = {r}");
            }
        }
    }

    #[test]
    fn linear_code_encode_into_overwrites_the_caller_buffer() {
        let code = LinearBlockCode::hamming(7);
        for message in probe_messages(code.k()) {
            let expected = block::BlockEncoder::encode(&code, &message).unwrap();

            let mut buffer = BitVec::zeros(code.n());
            for index in 0..code.n() {
                buffer.set(index, true);
            }
            block::BlockEncoder::encode_into(&code, &message, &mut buffer).unwrap();
            assert_eq!(buffer, expected, "a dirty buffer is overwritten, not mixed");
        }
    }

    #[test]
    fn linear_code_satisfies_block_encoder_contract() {
        let code = LinearBlockCode::hamming(3);
        let mut message = BitVec::zeros(code.k());
        for index in [0, 2, 3] {
            message.set(index, true);
        }
        conformance::block_encoder_contract(&code, &message);
    }

    #[test]
    fn linear_code_generator_rows_encode_message_basis() {
        let code = LinearBlockCode::hamming(3);
        conformance::generator_rows_encode_basis(&code, &Fp::<2>::new(1));
        conformance::generator_matrix_contract(&code);
        conformance::generator_parity_orthogonality(&code);
    }

    #[test]
    fn linear_code_parity_check_matrix_matches_stored_h() {
        let code = LinearBlockCode::hamming(3);
        let expected = code.parity_check().unwrap();
        let actual = block::ParityCheckMatrixAccess::parity_check_matrix(&code).unwrap();
        assert_eq!(&actual, expected);
        conformance::parity_check_matrix_contract(&code);
    }

    #[test]
    fn linear_code_without_parity_reports_capability_unavailable() {
        let code = LinearBlockCode::new_systematic(BitMatrix::zeros(1, 3), None);
        let expected = CodeError::CapabilityUnavailable {
            capability: CodeCapability::ParityCheckMatrix,
        };
        assert_eq!(
            block::ParityCheckMatrixAccess::parity_check_matrix(&code),
            Err(expected.clone())
        );

        let mut output = BitMatrix::zeros(code.n() - code.k(), code.n());
        assert_eq!(
            block::ParityCheckMatrixAccess::parity_check_matrix_into(&code, &mut output),
            Err(CodeError::CapabilityUnavailable {
                capability: CodeCapability::ParityCheckMatrix,
            })
        );
    }

    #[test]
    fn linear_code_binary_v1_adapter_preserves_version_1_results() {
        let code = LinearBlockCode::hamming(3);
        let mut message = BitVec::zeros(code.k());
        message.set(0, true);
        let canonical = block::BlockEncoder::encode(&code, &message).unwrap();
        let v1 = V1BlockEncoder::encode(&code, &message);
        assert_eq!(v1, canonical);
        assert_eq!(V1BlockEncoder::k(&code), code.k());
        assert_eq!(V1BlockEncoder::n(&code), code.n());

        let canonical_generator = block::GeneratorMatrixAccess::generator_matrix(&code).unwrap();
        let v1_generator = V1Generator::generator_matrix(&code);
        assert_eq!(v1_generator, canonical_generator);
        assert!(V1Generator::is_systematic(&code));
    }

    #[test]
    fn linear_code_packed_word_boundary_encoding() {
        let code = LinearBlockCode::hamming(7);
        let mut message = BitVec::zeros(code.k());
        for index in [0, 63, 64, 65, 119] {
            message.set(index, true);
        }

        let canonical = block::BlockEncoder::encode(&code, &message).unwrap();
        let v1 = V1BlockEncoder::encode(&code, &message);
        assert_eq!(canonical, v1);
        assert_eq!(canonical.len(), 127);
        for (message_index, &codeword_index) in code.systematic_positions.iter().enumerate() {
            assert_eq!(message.get(message_index), canonical.get(codeword_index));
        }
    }
}
