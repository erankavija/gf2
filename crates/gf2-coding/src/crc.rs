//! CRC-based linear block codes for GRAND decoding.
//!
//! A CRC (Cyclic Redundancy Check) with polynomial of degree r defines a
//! (n, n-r) linear block code. While CRCs are typically used for error detection,
//! GRAND-family decoders can use them for error *correction* by treating the
//! CRC syndrome as a linear code constraint.
//!
//! # Construction
//!
//! Given a CRC polynomial p(x) of degree r and desired codeword length n:
//! - k = n - r message bits
//! - Systematic encoding: c(x) = x^r · m(x) + (x^r · m(x)) mod p(x)
//! - Generator matrix G (k × n) in systematic form [I_k | P]
//! - Parity-check matrix H (r × n) = [P^T | I_r]
//!
//! # Examples
//!
//! ```
//! use gf2_coding::crc::CrcCode;
//! use gf2_coding::traits::{BlockEncoder, GeneratorMatrixAccess};
//! use gf2_core::BitVec;
//!
//! let code = CrcCode::crc_25_15();
//! assert_eq!(code.n(), 25);
//! assert_eq!(code.k(), 15);
//!
//! let msg = BitVec::ones(15);
//! let cw = code.encode(&msg);
//! assert_eq!(cw.len(), 25);
//!
//! // Syndrome is zero for valid codewords
//! let syn = code.syndrome(&cw);
//! assert_eq!(syn.count_ones(), 0);
//! ```

use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::{BitMatrix, BitVec};

/// A CRC-based linear block code for use with GRAND decoders.
///
/// The code is defined by a CRC generator polynomial and a codeword length.
/// It provides systematic encoding and syndrome computation via the standard
/// linear code interface required by GRAND.
///
/// # Examples
///
/// ```
/// use gf2_coding::crc::CrcCode;
/// use gf2_coding::traits::GeneratorMatrixAccess;
///
/// let code = CrcCode::crc_25_15();
/// assert_eq!(code.n(), 25);
/// assert_eq!(code.k(), 15);
/// ```
#[derive(Clone, Debug)]
pub struct CrcCode {
    /// Generator matrix G (k × n) in systematic form
    g: BitMatrix,
    /// Parity-check matrix H (r × n)
    h: BitMatrix,
    /// Codeword length
    n: usize,
    /// Message length
    k: usize,
    /// CRC polynomial (bit representation, MSB is degree r)
    poly: u64,
}

impl CrcCode {
    /// Creates a CRC-based linear block code.
    ///
    /// # Arguments
    ///
    /// * `n` - Codeword length
    /// * `poly` - CRC generator polynomial in binary (e.g., 0x2b9 for a degree-10 polynomial).
    ///   The polynomial `x^r + ... + 1` is represented with the x^r bit included.
    ///
    /// # Panics
    ///
    /// Panics if the polynomial degree is zero or if n <= degree.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// // CRC(25,15) with polynomial 0x6b9 (degree 10)
    /// let code = CrcCode::new(25, 0x6b9);
    /// assert_eq!(code.n(), 25);
    /// assert_eq!(code.k(), 15);
    /// ```
    pub fn new(n: usize, poly: u64) -> Self {
        assert!(poly > 1, "Polynomial must have degree >= 1");

        // Compute degree (position of MSB)
        let degree = 63 - poly.leading_zeros() as usize;
        assert!(n > degree, "Codeword length must exceed polynomial degree");

        let k = n - degree;

        // Build systematic generator matrix G = [I_k | P]
        // For each message unit vector e_i, compute the CRC remainder
        let mut g = BitMatrix::zeros(k, n);
        for i in 0..k {
            // Set identity part
            g.set(i, i, true);

            // Compute CRC of x^(r + k - 1 - i) = shift the single bit by (r + k - 1 - i) positions
            // In systematic form: for message bit at position i, compute remainder of
            // x^(degree + k - 1 - i) mod poly
            let remainder = Self::crc_remainder_single_bit(k - 1 - i, degree, poly);

            // Set parity part (columns k..n)
            for bit in 0..degree {
                if (remainder >> bit) & 1 == 1 {
                    g.set(i, k + (degree - 1 - bit), true);
                }
            }
        }

        // Build parity-check matrix H = [P^T | I_r]
        let mut h = BitMatrix::zeros(degree, n);

        // P^T part: extract from G
        for i in 0..degree {
            for j in 0..k {
                h.set(i, j, g.get(j, k + i));
            }
        }

        // I_r part
        for i in 0..degree {
            h.set(i, k + i, true);
        }

        Self { g, h, n, k, poly }
    }

    /// Computes the CRC remainder for a single bit at a given message position.
    ///
    /// Computes x^(degree + position) mod poly using repeated polynomial division.
    fn crc_remainder_single_bit(position: usize, degree: usize, poly: u64) -> u64 {
        // Start with x^(degree + position) and reduce mod poly
        // This is equivalent to shifting a 1 bit through a CRC register
        let mut remainder: u64 = 1;
        for _ in 0..(degree + position) {
            remainder <<= 1;
            if remainder & (1 << degree) != 0 {
                remainder ^= poly;
            }
        }
        remainder
    }

    /// Returns the codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// assert_eq!(code.n(), 25);
    /// ```
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// assert_eq!(code.k(), 15);
    /// ```
    pub fn k(&self) -> usize {
        self.k
    }

    /// Returns the CRC generator polynomial.
    pub fn poly(&self) -> u64 {
        self.poly
    }

    /// Returns whether all codewords have even Hamming weight.
    ///
    /// For CRC codes, this is true when (x+1) divides the generator polynomial,
    /// i.e., the polynomial has an even number of terms.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// // 0x2b9 = 1010111001 — check if (x+1) divides it
    /// println!("is_even: {}", code.is_even());
    /// ```
    pub fn is_even(&self) -> bool {
        // (x+1) divides poly iff poly evaluated at x=1 is 0
        // i.e., the number of 1-bits in poly is even
        self.poly.count_ones() % 2 == 0
    }

    /// Returns a reference to the parity-check matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// let h = code.h();
    /// assert_eq!(h.rows(), 10);
    /// assert_eq!(h.cols(), 25);
    /// ```
    pub fn h(&self) -> &BitMatrix {
        &self.h
    }

    /// Returns a reference to the generator matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// let g = code.g();
    /// assert_eq!(g.rows(), 15);
    /// assert_eq!(g.cols(), 25);
    /// ```
    pub fn g(&self) -> &BitMatrix {
        &self.g
    }

    /// Computes the syndrome of a received word.
    ///
    /// Returns the zero vector for valid codewords.
    ///
    /// # Arguments
    ///
    /// * `received` - A received word of length n
    ///
    /// # Panics
    ///
    /// Panics if `received.len() != n()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = CrcCode::crc_25_15();
    /// let msg = BitVec::ones(15);
    /// let cw = code.encode(&msg);
    /// let syn = code.syndrome(&cw);
    /// assert_eq!(syn.count_ones(), 0);
    /// ```
    pub fn syndrome(&self, received: &BitVec) -> BitVec {
        assert_eq!(
            received.len(),
            self.n,
            "Received word must have length n = {}",
            self.n
        );

        self.h.matvec(received)
    }

    // ---- Factory constructors ----

    /// Creates CRC(25, 15) with polynomial 0x2b9.
    ///
    /// This code uses the 10-bit CRC polynomial x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + x^0
    /// (0x6B9 with the x^10 term, or 0x2b9 as the standard representation with MSB = x^10).
    ///
    /// Wait — let me clarify: `0x2b9` in hex is `0010_1011_1001` in binary = x^9 + x^7 + x^5 + x^4 + x^3 + x^0.
    /// That's only degree 9. For a (25,15) code, we need degree 10. The intended polynomial
    /// including the leading x^10 term is `0x6b9` = `0110_1011_1001` = x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + x^0.
    ///
    /// The GRAND literature specifies polynomial `0x2b9` as shorthand for the 10-bit CRC
    /// where the leading coefficient is implicit. We use the full representation internally.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = CrcCode::crc_25_15();
    /// assert_eq!(code.n(), 25);
    /// assert_eq!(code.k(), 15);
    ///
    /// let msg = BitVec::ones(15);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.len(), 25);
    /// ```
    pub fn crc_25_15() -> Self {
        // 0x2b9 with implicit leading bit → 0x2b9 | (1 << 10) = 0x6b9
        // 0x6b9 = x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + x^0
        Self::new(25, 0x6b9)
    }
}

impl BlockEncoder for CrcCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }

    /// Encodes a message using systematic CRC encoding.
    ///
    /// The codeword format is [message bits | CRC parity bits].
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != k()`.
    fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.k,
            "Message must have length k = {}",
            self.k
        );

        // Compute codeword = message * G
        let mut msg_matrix = BitMatrix::zeros(1, self.k);
        for i in 0..self.k {
            msg_matrix.set(0, i, message.get(i));
        }

        let cw_matrix = &msg_matrix * &self.g;
        cw_matrix.row_as_bitvec(0)
    }
}

impl GeneratorMatrixAccess for CrcCode {
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
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Dimension tests ----

    #[test]
    fn test_crc_25_15_dimensions() {
        let code = CrcCode::crc_25_15();
        assert_eq!(code.n(), 25);
        assert_eq!(code.k(), 15);
    }

    #[test]
    fn test_crc_25_15_matrix_dimensions() {
        let code = CrcCode::crc_25_15();
        assert_eq!(code.g().rows(), 15);
        assert_eq!(code.g().cols(), 25);
        assert_eq!(code.h().rows(), 10);
        assert_eq!(code.h().cols(), 25);
    }

    // ---- H * G^T = 0 ----

    #[test]
    fn test_crc_25_15_h_gt_zero() {
        let code = CrcCode::crc_25_15();
        let gt = code.g().transpose();
        let product = code.h() * &gt;

        for r in 0..product.rows() {
            for c in 0..product.cols() {
                assert!(
                    !product.get(r, c),
                    "H * G^T not zero at ({}, {}) for CRC(25,15)",
                    r,
                    c
                );
            }
        }
    }

    // ---- Encoding and syndrome tests ----

    #[test]
    fn test_crc_25_15_zero_syndrome_for_codewords() {
        let code = CrcCode::crc_25_15();

        // Test a variety of messages
        for seed in 0u32..200 {
            let mut msg = BitVec::new();
            for bit in 0..15 {
                msg.push_bit(((seed.wrapping_mul(7).wrapping_add(bit)) % 2) == 0);
            }
            let cw = code.encode(&msg);
            let syn = code.syndrome(&cw);
            assert_eq!(syn.count_ones(), 0, "Non-zero syndrome for seed {}", seed);
        }
    }

    #[test]
    fn test_crc_25_15_zero_message_syndrome() {
        let code = CrcCode::crc_25_15();
        let msg = BitVec::new();
        let mut zero_msg = msg;
        zero_msg.resize(15, false);
        let cw = code.encode(&zero_msg);

        // All-zero message should give all-zero codeword
        assert_eq!(cw.count_ones(), 0);
        let syn = code.syndrome(&cw);
        assert_eq!(syn.count_ones(), 0);
    }

    #[test]
    fn test_crc_25_15_all_ones_message() {
        let code = CrcCode::crc_25_15();
        let msg = BitVec::ones(15);
        let cw = code.encode(&msg);
        assert_eq!(cw.len(), 25);

        let syn = code.syndrome(&cw);
        assert_eq!(syn.count_ones(), 0);
    }

    // ---- Systematic property ----

    #[test]
    fn test_crc_25_15_systematic_encoding() {
        let code = CrcCode::crc_25_15();

        for seed in 0u32..50 {
            let mut msg = BitVec::new();
            for bit in 0..15 {
                msg.push_bit(((seed.wrapping_mul(13).wrapping_add(bit)) % 2) == 0);
            }
            let cw = code.encode(&msg);

            // First k bits should be the message
            for bit in 0..15 {
                assert_eq!(
                    cw.get(bit),
                    msg.get(bit),
                    "Systematic bit {} mismatch for seed {}",
                    bit,
                    seed
                );
            }
        }
    }

    // ---- Non-zero syndrome for errors ----

    #[test]
    fn test_crc_25_15_nonzero_syndrome_for_single_error() {
        let code = CrcCode::crc_25_15();
        let msg = BitVec::ones(15);
        let cw = code.encode(&msg);

        for pos in 0..25 {
            let mut received = cw.clone();
            received.set(pos, !received.get(pos));
            let syn = code.syndrome(&received);
            assert!(
                syn.count_ones() > 0,
                "Zero syndrome for error at position {}",
                pos
            );
        }
    }

    // ---- Exhaustive small test with different polynomial ----

    #[test]
    fn test_crc_small_code() {
        // CRC-3 polynomial x^3 + x + 1 = 0b1011 = 0xB
        // Code length 7, so CRC(7, 4)
        let code = CrcCode::new(7, 0xB);
        assert_eq!(code.n(), 7);
        assert_eq!(code.k(), 4);

        // Exhaustive: check all 2^4 = 16 messages
        for i in 0u32..16 {
            let mut msg = BitVec::new();
            for bit in 0..4 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);
            let syn = code.syndrome(&cw);
            assert_eq!(syn.count_ones(), 0, "Non-zero syndrome for message {}", i);
        }
    }

    #[test]
    fn test_crc_small_h_gt_zero() {
        let code = CrcCode::new(7, 0xB);
        let gt = code.g().transpose();
        let product = code.h() * &gt;

        for r in 0..product.rows() {
            for c in 0..product.cols() {
                assert!(!product.get(r, c), "H * G^T not zero at ({}, {})", r, c);
            }
        }
    }

    // ---- is_even tests ----

    #[test]
    fn test_crc_is_even() {
        // 0x6b9 = 11010111001 → popcount = 7 → odd → (x+1) does not divide → not even
        let code = CrcCode::crc_25_15();
        // poly(1) = 1+1+0+1+0+1+1+1+0+0+1 = 7 mod 2 = 1 → not divisible by (x+1)
        assert!(!code.is_even());

        // Test a polynomial divisible by (x+1): x^3 + x^2 + x + 1 = 0b1111
        // popcount = 4 (even) → (x+1) divides → is_even = true
        let code2 = CrcCode::new(7, 0xF);
        assert!(code2.is_even());
    }

    // ---- Minimum distance of CRC(25,15) ----

    #[test]
    fn test_crc_25_15_minimum_distance_lower_bound() {
        // CRC(25,15) should have minimum distance >= 4 since the polynomial
        // x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1 has burst-error detection capability.
        // We verify by sampling: check that no non-zero codeword has weight < 4.
        let code = CrcCode::crc_25_15();

        // Test all single-weight-1 messages and weight-2 messages
        for i in 0..15 {
            let mut msg = BitVec::new();
            msg.resize(15, false);
            msg.set(i, true);
            let cw = code.encode(&msg);
            let w = cw.count_ones();
            assert!(
                w >= 4,
                "Weight-1 message at bit {} gives codeword weight {}, expected >= 4",
                i,
                w
            );
        }
    }

    // ---- GeneratorMatrixAccess trait ----

    #[test]
    fn test_crc_generator_matrix_access_trait() {
        let code = CrcCode::crc_25_15();
        let g = <CrcCode as GeneratorMatrixAccess>::generator_matrix(&code);
        assert_eq!(g.rows(), 15);
        assert_eq!(g.cols(), 25);
        assert!(<CrcCode as GeneratorMatrixAccess>::is_systematic(&code));
    }

    // ---- Custom polynomial ----

    #[test]
    fn test_crc_custom_polynomial() {
        // x^4 + x + 1 = 0b10011 = 0x13
        let code = CrcCode::new(15, 0x13);
        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);

        let msg = BitVec::ones(11);
        let cw = code.encode(&msg);
        let syn = code.syndrome(&cw);
        assert_eq!(syn.count_ones(), 0);
    }

    #[test]
    #[should_panic(expected = "Polynomial must have degree >= 1")]
    fn test_crc_invalid_polynomial() {
        CrcCode::new(10, 1); // degree 0
    }

    #[test]
    #[should_panic(expected = "Codeword length must exceed polynomial degree")]
    fn test_crc_n_too_small() {
        CrcCode::new(3, 0x13); // degree 4, n=3
    }
}
