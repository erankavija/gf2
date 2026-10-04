//! CRC codes used as systematic linear block codes: a generator polynomial of
//! degree r defines an (n, n-r) code. Polynomials are in the full
//! representation, with the leading x^r bit set: the CRC(25,15) polynomial
//! x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1 is `0x6b9`, written `0x2b9` in the
//! truncated notation of `@/citation/Yuan2025`.

use crate::linear::LinearBlockCode;
use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::{BitMatrix, BitVec};

/// A CRC code as a systematic linear block code: the parity bits of a message
/// m(x) are the remainder of x^r * m(x) divided by g(x).
#[derive(Debug, Clone)]
pub struct CrcCode {
    inner: LinearBlockCode,
    /// The full polynomial value (with the leading degree-r bit set).
    poly: u64,
}

impl CrcCode {
    /// Creates the (n, k) code whose generator polynomial `poly`, in the full
    /// representation, has degree `n - k`.
    ///
    /// # Panics
    ///
    /// Panics if `n <= k` or if `poly` does not have degree `n - k`.
    ///
    /// # Complexity
    ///
    /// O(k * n) for constructing the systematic generator matrix.
    pub fn new(n: usize, k: usize, poly: u64) -> Self {
        assert!(n > k, "n must be greater than k");
        let r = n - k;

        let degree = 63 - poly.leading_zeros() as usize;
        assert_eq!(
            degree, r,
            "Polynomial degree ({}) must equal n - k ({})",
            degree, r
        );

        let mut g = BitMatrix::zeros(k, n);

        for i in 0..k {
            g.set(i, i, true);

            // Message bit 0 is the highest-degree coefficient, so basis vector i is x^{k-1-i}.
            let remainder = Self::crc_remainder(1u64 << (k - 1 - i), k, r, poly);

            for j in 0..r {
                if (remainder >> (r - 1 - j)) & 1 == 1 {
                    g.set(i, k + j, true);
                }
            }
        }

        let mut h = BitMatrix::zeros(r, n);
        for i in 0..r {
            for j in 0..k {
                h.set(i, j, g.get(j, k + i));
            }
            h.set(i, k + i, true);
        }

        let inner = LinearBlockCode::new_systematic(g, Some(h));

        Self { inner, poly }
    }

    /// Computes the CRC remainder of `msg_val` (a polynomial of degree < `k`)
    /// divided by `poly` (of degree `r`). Returns an `r`-bit remainder.
    fn crc_remainder(msg_val: u64, k: usize, r: usize, poly: u64) -> u64 {
        let mut dividend = msg_val << r;
        let deg = k + r; // maximum possible degree + 1

        for i in (0..deg).rev() {
            if (dividend >> i) & 1 == 1 {
                // Only subtract if this would reduce degree
                if i >= r {
                    dividend ^= poly << (i - r);
                }
            }
        }

        dividend & ((1u64 << r) - 1)
    }

    /// Returns the codeword length.
    pub fn n(&self) -> usize {
        self.inner.n()
    }

    /// Returns the message length.
    pub fn k(&self) -> usize {
        self.inner.k()
    }

    /// The generator polynomial in full representation (leading x^r bit set).
    pub fn poly(&self) -> u64 {
        self.poly
    }

    /// The CRC(25,15) code of `@/citation/Yuan2025`, with
    /// `g(x) = x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1` (`0x6b9`).
    pub fn crc_25_15() -> Self {
        Self::new(25, 15, 0x6b9)
    }

    /// The parity-check matrix H = [P^T | I_r].
    pub fn parity_check(&self) -> &BitMatrix {
        self.inner
            .parity_check()
            .expect("CRC code always has H matrix")
    }

    /// Returns whether all codewords have even Hamming weight.
    ///
    /// O(k * n): tests the weight of every generator row.
    pub fn is_even(&self) -> bool {
        // A linear code is even iff every generator row has even weight.
        let g = self.inner.generator_matrix();
        for i in 0..self.inner.k() {
            let mut weight = 0;
            for j in 0..self.inner.n() {
                if g.get(i, j) {
                    weight += 1;
                }
            }
            if weight % 2 != 0 {
                return false;
            }
        }
        true
    }

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
        let cw = code.encode(&BitVec::zeros(code.k()));
        let syn = code.inner().syndrome(&cw).unwrap();
        assert_eq!(syn.count_ones(), 0);
        let cw = code.encode(&BitVec::ones(code.k()));
        let syn = code.inner().syndrome(&cw).unwrap();
        assert_eq!(syn.count_ones(), 0);
    }

    #[test]
    fn test_crc_25_15_minimum_distance_lower_bound() {
        let code = CrcCode::crc_25_15();
        let n = code.n();

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

    #[test]
    fn test_crc_25_15_minimum_distance_exact() {
        let code = CrcCode::crc_25_15();
        let n = code.n();

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

        assert_eq!(
            min_weight_found, 4,
            "CRC(25,15) d_min should be exactly 4, found {}",
            min_weight_found
        );
    }

    #[test]
    fn test_crc_polynomial_full_representation() {
        let poly: u64 = 0x6b9;
        // x^10 + x^9 + x^7 + x^5 + x^4 + x^3 + 1
        assert_eq!(poly, 0b110_1011_1001);
        assert_eq!((poly >> 10) & 1, 1);
        assert_eq!((poly >> 9) & 1, 1);
        assert_eq!((poly >> 8) & 1, 0);
        assert_eq!((poly >> 7) & 1, 1);
        assert_eq!((poly >> 6) & 1, 0);
        assert_eq!((poly >> 5) & 1, 1);
        assert_eq!((poly >> 4) & 1, 1);
        assert_eq!((poly >> 3) & 1, 1);
        assert_eq!((poly >> 2) & 1, 0);
        assert_eq!((poly >> 1) & 1, 0);
        assert_eq!(poly & 1, 1);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::traits::BlockEncoder;
    use proptest::prelude::*;

    proptest! {
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
