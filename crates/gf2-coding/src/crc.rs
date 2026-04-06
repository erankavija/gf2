//! CRC (Cyclic Redundancy Check) codes used as linear block codes.
//!
//! A CRC polynomial of degree r defines an (n, n-r) linear block code.
//! When used for error correction (rather than just detection), the code is
//! treated as a standard linear block code with a systematic generator
//! matrix derived from the CRC polynomial.
//!
//! # CRC polynomial conventions
//!
//! A CRC generator polynomial of degree r has the form:
//!
//! g(x) = x^r + c\_{r-1} x^{r-1} + ... + c\_1 x + c\_0
//!
//! There are two common hexadecimal representations:
//!
//! - **Full representation** (degree-r bit set): includes the leading x^r term.
//!   For a degree-10 polynomial, this is an 11-bit value.
//! - **Truncated representation** (degree-r bit omitted): the leading coefficient
//!   is always 1 and is implied.
//!
//! This module uses the **full representation** in constructors. For example,
//! the CRC-10 polynomial x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + x^0 is
//! `0x6b9` in full form (bit 10 set) or `0x2b9` in truncated form.
//!
//! # Example: CRC(25,15)
//!
//! The CRC(25,15) code uses the degree-10 polynomial `0x6b9`:
//!
//! ```text
//! g(x) = x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1
//!      = 0b110_1011_1001 = 0x6b9 (full, with x^10 bit)
//!      = 0b010_1011_1001 = 0x2b9 (truncated, without x^10 bit)
//! ```
//!
//! # Examples
//!
//! ```
//! use gf2_coding::crc::CrcCode;
//! use gf2_coding::traits::BlockEncoder;
//! use gf2_core::BitVec;
//!
//! let code = CrcCode::crc_25_15();
//! assert_eq!(code.n(), 25);
//! assert_eq!(code.k(), 15);
//!
//! let msg = BitVec::ones(15);
//! let cw = code.encode(&msg);
//! assert_eq!(cw.len(), 25);
//! ```

use crate::linear::LinearBlockCode;
use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::{BitMatrix, BitVec};

/// A CRC code treated as a linear block code.
///
/// The generator matrix is constructed from the CRC polynomial using
/// systematic encoding: for each basis message, the parity bits are the
/// remainder of dividing x^r * m(x) by g(x).
///
/// # Examples
///
/// ```
/// use gf2_coding::crc::CrcCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_core::BitVec;
///
/// let code = CrcCode::crc_25_15();
/// let msg = BitVec::zeros(15);
/// let cw = code.encode(&msg);
/// assert_eq!(cw.len(), 25);
/// ```
#[derive(Debug, Clone)]
pub struct CrcCode {
    inner: LinearBlockCode,
    /// The full polynomial value (with the leading degree-r bit set).
    poly: u64,
}

impl CrcCode {
    /// Creates a CRC-based linear block code from a generator polynomial.
    ///
    /// # Arguments
    ///
    /// * `n` - Codeword length
    /// * `k` - Message length
    /// * `poly` - CRC generator polynomial in **full representation** (the
    ///   leading x^r coefficient bit is set). The polynomial must have degree
    ///   exactly `n - k`.
    ///
    /// # Panics
    ///
    /// Panics if `n <= k` or if `poly` does not have degree `n - k`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// // CRC(25,15) with polynomial 0x6b9 (degree 10)
    /// let code = CrcCode::new(25, 15, 0x6b9);
    /// assert_eq!(code.n(), 25);
    /// assert_eq!(code.k(), 15);
    /// ```
    pub fn new(n: usize, k: usize, poly: u64) -> Self {
        assert!(n > k, "n must be greater than k");
        let r = n - k;

        // Verify polynomial degree
        let degree = 63 - poly.leading_zeros() as usize;
        assert_eq!(
            degree, r,
            "Polynomial degree ({}) must equal n - k ({})",
            degree, r
        );

        // Build generator matrix by systematic encoding each basis vector.
        // For message m_i = e_i (i-th basis vector), the codeword is
        // [m_i | remainder of x^r * m_i(x) / g(x)].
        let mut g = BitMatrix::zeros(k, n);

        for i in 0..k {
            // Identity part
            g.set(i, i, true);

            // Compute remainder: the message polynomial for basis vector i
            // has a single 1 at position i. In the polynomial representation,
            // message bit 0 is the highest-degree coefficient (x^{k-1}), so
            // basis vector i corresponds to x^{k-1-i}.
            let remainder = Self::crc_remainder(1u64 << (k - 1 - i), k, r, poly);

            // Set parity bits (columns k..n)
            for j in 0..r {
                if (remainder >> (r - 1 - j)) & 1 == 1 {
                    g.set(i, k + j, true);
                }
            }
        }

        // Build H matrix: H = [P^T | I_r]
        let mut h = BitMatrix::zeros(r, n);
        for i in 0..r {
            // P^T part: column i of P^T = row i across all k message parity contributions
            for j in 0..k {
                h.set(i, j, g.get(j, k + i));
            }
            // Identity part
            h.set(i, k + i, true);
        }

        let inner = LinearBlockCode::new_systematic(g, Some(h));

        Self { inner, poly }
    }

    /// Computes the CRC remainder of `msg_val` (a polynomial of degree < `k`)
    /// divided by `poly` (of degree `r`). Returns an `r`-bit remainder.
    fn crc_remainder(msg_val: u64, k: usize, r: usize, poly: u64) -> u64 {
        // Shift message by r positions (multiply by x^r)
        let mut dividend = msg_val << r;
        let deg = k + r; // maximum possible degree + 1

        // Long division from highest bit
        for i in (0..deg).rev() {
            if (dividend >> i) & 1 == 1 {
                // Only subtract if this would reduce degree
                if i >= r {
                    dividend ^= poly << (i - r);
                }
            }
        }

        // Remainder is the low r bits
        dividend & ((1u64 << r) - 1)
    }

    /// Returns the codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// assert_eq!(CrcCode::crc_25_15().n(), 25);
    /// ```
    pub fn n(&self) -> usize {
        self.inner.n()
    }

    /// Returns the message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// assert_eq!(CrcCode::crc_25_15().k(), 15);
    /// ```
    pub fn k(&self) -> usize {
        self.inner.k()
    }

    /// Returns the CRC generator polynomial in full representation.
    ///
    /// The returned value has the leading x^r bit set. For example, the
    /// degree-10 polynomial `x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1`
    /// is returned as `0x6b9`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// assert_eq!(code.poly(), 0x6b9);
    /// ```
    pub fn poly(&self) -> u64 {
        self.poly
    }

    /// Creates the CRC(25,15) code used in GRAND product code constructions.
    ///
    /// Generator polynomial: `g(x) = x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1`.
    ///
    /// - Full representation (with x^10 bit): `0x6b9`
    /// - Truncated representation (without x^10 bit): `0x2b9`
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
    /// assert_eq!(code.poly(), 0x6b9);
    ///
    /// let msg = BitVec::ones(15);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.len(), 25);
    /// ```
    pub fn crc_25_15() -> Self {
        // x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1
        // = 0b110_1011_1001 = 0x6b9
        Self::new(25, 15, 0x6b9)
    }

    /// Returns the parity-check matrix H.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// let h = code.parity_check();
    /// assert_eq!(h.rows(), 10);
    /// assert_eq!(h.cols(), 25);
    /// ```
    pub fn parity_check(&self) -> &BitMatrix {
        self.inner
            .parity_check()
            .expect("CRC code always has H matrix")
    }

    /// Returns the inner [`LinearBlockCode`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::crc::CrcCode;
    ///
    /// let code = CrcCode::crc_25_15();
    /// let inner = code.inner();
    /// assert_eq!(inner.n(), 25);
    /// ```
    pub fn inner(&self) -> &LinearBlockCode {
        &self.inner
    }
}

impl BlockEncoder for CrcCode {
    fn k(&self) -> usize {
        self.inner.k()
    }

    fn n(&self) -> usize {
        self.inner.n()
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        self.inner.encode(message)
    }
}

impl GeneratorMatrixAccess for CrcCode {
    fn k(&self) -> usize {
        self.inner.k()
    }

    fn n(&self) -> usize {
        self.inner.n()
    }

    fn generator_matrix(&self) -> BitMatrix {
        self.inner.generator().clone()
    }

    fn is_systematic(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::BlockEncoder;

    #[test]
    fn test_crc_25_15_parameters() {
        let code = CrcCode::crc_25_15();
        assert_eq!(code.n(), 25);
        assert_eq!(code.k(), 15);
        assert_eq!(code.poly(), 0x6b9);
    }

    #[test]
    fn test_crc_25_15_orthogonality() {
        let code = CrcCode::crc_25_15();
        let g = code.generator_matrix();
        let h = code.parity_check();
        let h_t = h.transpose();
        let product = &g * &h_t;
        for i in 0..product.rows() {
            for j in 0..product.cols() {
                assert!(!product.get(i, j), "G*H^T must be zero at ({}, {})", i, j);
            }
        }
    }

    #[test]
    fn test_crc_25_15_syndrome_zero() {
        let code = CrcCode::crc_25_15();
        for i in 0..code.k() {
            let mut msg = BitVec::zeros(code.k());
            msg.set(i, true);
            let cw = code.encode(&msg);
            let syn = code.inner().syndrome(&cw).unwrap();
            assert_eq!(
                syn.count_ones(),
                0,
                "Syndrome must be zero for codeword from basis vector {}",
                i
            );
        }
        // All zeros
        let cw = code.encode(&BitVec::zeros(code.k()));
        let syn = code.inner().syndrome(&cw).unwrap();
        assert_eq!(syn.count_ones(), 0);
        // All ones
        let cw = code.encode(&BitVec::ones(code.k()));
        let syn = code.inner().syndrome(&cw).unwrap();
        assert_eq!(syn.count_ones(), 0);
    }

    /// Verify minimum distance by checking that no weight-1 or weight-2
    /// pattern has zero syndrome, and find the actual d_min.
    #[test]
    fn test_crc_25_15_minimum_distance_lower_bound() {
        let code = CrcCode::crc_25_15();
        let n = code.n();

        // Weight 1: all must have nonzero syndrome
        for i in 0..n {
            let mut e = BitVec::zeros(n);
            e.set(i, true);
            let syn = code.inner().syndrome(&e).unwrap();
            assert!(
                syn.count_ones() > 0,
                "weight-1 at pos {} has zero syndrome",
                i
            );
        }

        // Weight 2: all must have nonzero syndrome
        for i in 0..n {
            for j in (i + 1)..n {
                let mut e = BitVec::zeros(n);
                e.set(i, true);
                e.set(j, true);
                let syn = code.inner().syndrome(&e).unwrap();
                assert!(
                    syn.count_ones() > 0,
                    "weight-2 at ({},{}) has zero syndrome",
                    i,
                    j
                );
            }
        }
    }

    /// Compute the exact minimum distance by searching weight-3 patterns.
    /// This verifies d_min >= 3 (checked above) and determines if d_min > 3.
    #[test]
    fn test_crc_25_15_minimum_distance_exact() {
        let code = CrcCode::crc_25_15();
        let n = code.n();

        // Search weight-3 for zero syndrome
        let mut min_weight_found = n + 1;
        for i in 0..n {
            for j in (i + 1)..n {
                for l in (j + 1)..n {
                    let mut e = BitVec::zeros(n);
                    e.set(i, true);
                    e.set(j, true);
                    e.set(l, true);
                    let syn = code.inner().syndrome(&e).unwrap();
                    if syn.count_ones() == 0 && 3 < min_weight_found {
                        min_weight_found = 3;
                    }
                }
            }
        }

        if min_weight_found > 3 {
            // Check weight-4
            'w4: for i in 0..n {
                for j in (i + 1)..n {
                    for l in (j + 1)..n {
                        for m in (l + 1)..n {
                            let mut e = BitVec::zeros(n);
                            e.set(i, true);
                            e.set(j, true);
                            e.set(l, true);
                            e.set(m, true);
                            let syn = code.inner().syndrome(&e).unwrap();
                            if syn.count_ones() == 0 {
                                min_weight_found = 4;
                                break 'w4;
                            }
                        }
                    }
                }
            }
        }

        // CRC(25,15) should have d_min >= 3 (it's a CRC with a degree-10 polynomial)
        assert!(
            min_weight_found >= 3,
            "d_min must be at least 3, found {}",
            min_weight_found
        );
        // Record the actual d_min for documentation
        assert!(
            min_weight_found <= 6,
            "d_min should be reasonable, found {}",
            min_weight_found
        );
    }

    #[test]
    fn test_crc_polynomial_full_representation() {
        // Verify that 0x6b9 encodes the right polynomial
        let poly: u64 = 0x6b9;
        // x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1
        // bit10=1, bit9=1, bit8=0, bit7=1, bit6=0, bit5=1, bit4=1, bit3=1, bit2=0, bit1=0, bit0=1
        assert_eq!(poly, 0b110_1011_1001);
        assert_eq!((poly >> 10) & 1, 1); // x^10
        assert_eq!((poly >> 9) & 1, 1); // x^9
        assert_eq!((poly >> 8) & 1, 0); // no x^8
        assert_eq!((poly >> 7) & 1, 1); // x^7
        assert_eq!((poly >> 6) & 1, 0); // no x^6
        assert_eq!((poly >> 5) & 1, 1); // x^5
        assert_eq!((poly >> 4) & 1, 1); // x^4
        assert_eq!((poly >> 3) & 1, 1); // x^3
        assert_eq!((poly >> 2) & 1, 0); // no x^2
        assert_eq!((poly >> 1) & 1, 0); // no x^1
        assert_eq!(poly & 1, 1); // x^0
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::traits::BlockEncoder;
    use proptest::prelude::*;

    proptest! {
        /// For any random message, the encoded codeword must have zero syndrome.
        #[test]
        fn prop_crc_25_15_syndrome_zero(
            msg_bits in prop::collection::vec(any::<bool>(), 15)
        ) {
            let code = CrcCode::crc_25_15();
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = code.encode(&msg);
            let syn = code.inner().syndrome(&cw).unwrap();
            prop_assert_eq!(syn.count_ones(), 0, "syndrome must be zero for valid codeword");
        }

        /// The sum of two codewords must also be a codeword (linearity).
        #[test]
        fn prop_crc_25_15_linearity(
            msg1_bits in prop::collection::vec(any::<bool>(), 15),
            msg2_bits in prop::collection::vec(any::<bool>(), 15)
        ) {
            let code = CrcCode::crc_25_15();

            let mut msg1 = BitVec::new();
            for bit in msg1_bits { msg1.push_bit(bit); }
            let mut msg2 = BitVec::new();
            for bit in msg2_bits { msg2.push_bit(bit); }

            let cw1 = code.encode(&msg1);
            let cw2 = code.encode(&msg2);
            let mut cw_sum = cw1.clone();
            cw_sum.bit_xor_into(&cw2);

            let syn = code.inner().syndrome(&cw_sum).unwrap();
            prop_assert_eq!(syn.count_ones(), 0, "sum of codewords must be a codeword");
        }
    }
}
