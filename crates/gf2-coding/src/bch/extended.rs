//! Extended BCH codes.
//!
//! An extended BCH code eBCH(n+1, k) is formed from a standard BCH(n, k)
//! code by appending one overall parity-check bit to each codeword, ensuring
//! all codewords have even weight. This increases the minimum distance by 1
//! for odd-distance base codes.
//!
//! # Construction
//!
//! Given a BCH(n, k) code with generator matrix G\_bch (k x n) and
//! parity-check matrix H\_bch ((n-k) x n):
//!
//! - The extended generator matrix is G\_ext (k x (n+1)):
//!   row i of G\_ext = \[row i of G\_bch | parity bit\], where the parity bit
//!   makes the total weight even.
//!
//! - The extended parity-check matrix is H\_ext ((n-k+1) x (n+1)):
//!   H\_ext = \[H\_bch 0; 1 1 ... 1 1\], i.e., the original H with a zero
//!   column appended plus an all-ones row.
//!
//! # Supported codes
//!
//! | Code | Base | t | d\_min |
//! |------|------|---|-------|
//! | eBCH(16,11) | BCH(15,11) | 1 | 4 |
//! | eBCH(16,7) | BCH(15,7) | 2 | 6 |
//! | eBCH(32,26) | BCH(31,26) | 1 | 4 |
//! | eBCH(64,57) | BCH(63,57) | 1 | 4 |
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::extended::ExtendedBchCode;
//! use gf2_coding::traits::BlockEncoder;
//! use gf2_core::BitVec;
//!
//! let code = ExtendedBchCode::ebch_16_11();
//! assert_eq!(code.n(), 16);
//! assert_eq!(code.k(), 11);
//!
//! let msg = BitVec::ones(11);
//! let cw = code.encode(&msg);
//! assert_eq!(cw.len(), 16);
//! assert_eq!(cw.count_ones() % 2, 0); // even weight
//! ```

use crate::linear::LinearBlockCode;
use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};

use super::BchCode;

/// An extended BCH code formed by appending an overall parity-check bit.
///
/// All codewords of an extended BCH code have even Hamming weight. The
/// extension increases the minimum distance by 1 when the base code has
/// odd minimum distance.
///
/// The struct wraps a [`LinearBlockCode`] that stores the extended generator
/// and parity-check matrices.
///
/// # Examples
///
/// ```
/// use gf2_coding::bch::extended::ExtendedBchCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_core::BitVec;
///
/// let code = ExtendedBchCode::ebch_16_11();
/// let msg = BitVec::zeros(11);
/// let cw = code.encode(&msg);
/// assert_eq!(cw.count_ones() % 2, 0);
/// ```
#[derive(Debug, Clone)]
pub struct ExtendedBchCode {
    inner: LinearBlockCode,
    base_t: usize,
}

impl ExtendedBchCode {
    /// Constructs an extended BCH code from a base BCH code.
    ///
    /// Computes the generator and parity-check matrices for the extended
    /// code by appending an overall parity-check bit.
    ///
    /// # Arguments
    ///
    /// * `base` - The base BCH(n, k) code to extend
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::bch::BchCode;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let base = BchCode::new(15, 11, 1, field);
    /// let ext = ExtendedBchCode::from_bch(&base);
    /// assert_eq!(ext.n(), 16);
    /// assert_eq!(ext.k(), 11);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(k * n) where k and n are the BCH code parameters.
    pub fn from_bch(base: &BchCode) -> Self {
        let bch_g = base.generator_matrix(); // k x n
        let k = base.k();
        let n_bch = base.n();
        let n_ext = n_bch + 1;
        let r_bch = n_bch - k;

        // Build extended generator matrix: k x (n+1)
        // Each row gets the original bits plus a parity bit that makes weight even.
        let mut g_ext = BitMatrix::zeros(k, n_ext);
        for i in 0..k {
            let mut weight = 0usize;
            for j in 0..n_bch {
                let bit = bch_g.get(i, j);
                g_ext.set(i, j, bit);
                if bit {
                    weight += 1;
                }
            }
            // Set parity bit (last column) so total weight is even
            if weight % 2 != 0 {
                g_ext.set(i, n_bch, true);
            }
        }

        // Build extended parity-check matrix: (r+1) x (n+1)
        // Top part: [H_bch | 0]
        // Bottom row: [1 1 ... 1 1] (all-ones)
        let r_ext = r_bch + 1;
        let mut h_ext = BitMatrix::zeros(r_ext, n_ext);

        // We need the base H matrix. Compute it from G using RREF.
        // For systematic BCH, H = [-P^T | I_{r}] but we compute directly.
        // Actually, we build H from the base code's generator matrix.
        // For a systematic code with G = [I_k | P], H = [P^T | I_r].
        // The BCH encoder produces [message | parity], so G is systematic with
        // identity in the first k columns.

        // Extract P from G = [I_k | P]
        let mut p = BitMatrix::zeros(k, r_bch);
        for i in 0..k {
            for j in 0..r_bch {
                p.set(i, j, bch_g.get(i, k + j));
            }
        }
        // H_bch = [P^T | I_r]
        let p_t = p.transpose(); // r x k
        for i in 0..r_bch {
            for j in 0..k {
                h_ext.set(i, j, p_t.get(i, j));
            }
            h_ext.set(i, k + i, true); // identity part
                                       // Column n_bch (the extension column) is 0 for these rows
        }

        // Bottom row: all ones
        for j in 0..n_ext {
            h_ext.set(r_ext - 1, j, true);
        }

        let inner = LinearBlockCode::new_systematic(g_ext, Some(h_ext));

        Self {
            inner,
            base_t: base.t(),
        }
    }

    /// Returns the codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// assert_eq!(ExtendedBchCode::ebch_16_11().n(), 16);
    /// ```
    pub fn n(&self) -> usize {
        self.inner.n()
    }

    /// Returns the message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// assert_eq!(ExtendedBchCode::ebch_16_11().k(), 11);
    /// ```
    pub fn k(&self) -> usize {
        self.inner.k()
    }

    /// Returns the inner [`LinearBlockCode`].
    ///
    /// This provides access to the generator and parity-check matrices as
    /// well as syndrome computation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// let inner = code.inner();
    /// assert_eq!(inner.n(), 16);
    /// assert_eq!(inner.k(), 11);
    /// ```
    pub fn inner(&self) -> &LinearBlockCode {
        &self.inner
    }

    /// Returns `true` because all eBCH codewords have even weight.
    ///
    /// This flag is used by GRAND decoders to skip odd-weight candidates.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert!(code.is_even());
    /// ```
    pub fn is_even(&self) -> bool {
        true
    }

    /// Returns the error-correction capability of the base BCH code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_7();
    /// assert_eq!(code.base_t(), 2);
    /// ```
    pub fn base_t(&self) -> usize {
        self.base_t
    }

    /// Creates an eBCH(16,11) code from BCH(15,11,1).
    ///
    /// Parameters: n=16, k=11, base t=1, d\_min=4.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert_eq!(code.n(), 16);
    /// assert_eq!(code.k(), 11);
    ///
    /// let msg = BitVec::ones(11);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_16_11() -> Self {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let base = BchCode::new(15, 11, 1, field);
        Self::from_bch(&base)
    }

    /// Creates an eBCH(16,7) code from BCH(15,7,2).
    ///
    /// Parameters: n=16, k=7, base t=2, d\_min=6.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = ExtendedBchCode::ebch_16_7();
    /// assert_eq!(code.n(), 16);
    /// assert_eq!(code.k(), 7);
    ///
    /// let msg = BitVec::ones(7);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_16_7() -> Self {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let base = BchCode::new(15, 7, 2, field);
        Self::from_bch(&base)
    }

    /// Creates an eBCH(32,26) code from BCH(31,26,1).
    ///
    /// Parameters: n=32, k=26, base t=1, d\_min=4.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = ExtendedBchCode::ebch_32_26();
    /// assert_eq!(code.n(), 32);
    /// assert_eq!(code.k(), 26);
    ///
    /// let msg = BitVec::ones(26);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_32_26() -> Self {
        let field = Gf2mField::new(5, 0b100101).with_tables();
        let base = BchCode::new(31, 26, 1, field);
        Self::from_bch(&base)
    }

    /// Creates an eBCH(64,57) code from BCH(63,57,1).
    ///
    /// Parameters: n=64, k=57, base t=1, d\_min=4.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = ExtendedBchCode::ebch_64_57();
    /// assert_eq!(code.n(), 64);
    /// assert_eq!(code.k(), 57);
    ///
    /// let msg = BitVec::ones(57);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_64_57() -> Self {
        let field = Gf2mField::new(6, 0b1000011).with_tables();
        let base = BchCode::new(63, 57, 1, field);
        Self::from_bch(&base)
    }

    /// Returns the parity-check matrix H of the extended code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// let h = code.parity_check();
    /// assert_eq!(h.rows(), 16 - 11);
    /// assert_eq!(h.cols(), 16);
    /// ```
    pub fn parity_check(&self) -> &BitMatrix {
        self.inner
            .parity_check()
            .expect("extended BCH always has H matrix")
    }
}

impl BlockEncoder for ExtendedBchCode {
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

impl GeneratorMatrixAccess for ExtendedBchCode {
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
    fn test_ebch_16_11_parameters() {
        let code = ExtendedBchCode::ebch_16_11();
        assert_eq!(code.n(), 16);
        assert_eq!(code.k(), 11);
        assert_eq!(code.base_t(), 1);
        assert!(code.is_even());
    }

    #[test]
    fn test_ebch_16_7_parameters() {
        let code = ExtendedBchCode::ebch_16_7();
        assert_eq!(code.n(), 16);
        assert_eq!(code.k(), 7);
        assert_eq!(code.base_t(), 2);
        assert!(code.is_even());
    }

    #[test]
    fn test_ebch_32_26_parameters() {
        let code = ExtendedBchCode::ebch_32_26();
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 26);
        assert_eq!(code.base_t(), 1);
    }

    #[test]
    fn test_ebch_64_57_parameters() {
        let code = ExtendedBchCode::ebch_64_57();
        assert_eq!(code.n(), 64);
        assert_eq!(code.k(), 57);
        assert_eq!(code.base_t(), 1);
    }

    #[test]
    fn test_ebch_16_11_orthogonality() {
        let code = ExtendedBchCode::ebch_16_11();
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
    fn test_ebch_16_7_orthogonality() {
        let code = ExtendedBchCode::ebch_16_7();
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
    fn test_ebch_32_26_orthogonality() {
        let code = ExtendedBchCode::ebch_32_26();
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
    fn test_ebch_64_57_orthogonality() {
        let code = ExtendedBchCode::ebch_64_57();
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
    fn test_ebch_16_11_even_weight() {
        let code = ExtendedBchCode::ebch_16_11();
        // Test all single-bit messages and a few multi-bit
        for i in 0..code.k() {
            let mut msg = BitVec::zeros(code.k());
            msg.set(i, true);
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for basis vector {} must have even weight",
                i
            );
        }
        // All zeros
        let cw = code.encode(&BitVec::zeros(code.k()));
        assert_eq!(cw.count_ones() % 2, 0);
        // All ones
        let cw = code.encode(&BitVec::ones(code.k()));
        assert_eq!(cw.count_ones() % 2, 0);
    }

    #[test]
    fn test_ebch_16_7_even_weight() {
        let code = ExtendedBchCode::ebch_16_7();
        for i in 0..code.k() {
            let mut msg = BitVec::zeros(code.k());
            msg.set(i, true);
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for basis vector {} must have even weight",
                i
            );
        }
    }

    #[test]
    fn test_ebch_32_26_even_weight() {
        let code = ExtendedBchCode::ebch_32_26();
        for i in 0..code.k() {
            let mut msg = BitVec::zeros(code.k());
            msg.set(i, true);
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for basis vector {} must have even weight",
                i
            );
        }
    }

    #[test]
    fn test_ebch_64_57_even_weight() {
        let code = ExtendedBchCode::ebch_64_57();
        for i in 0..code.k() {
            let mut msg = BitVec::zeros(code.k());
            msg.set(i, true);
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for basis vector {} must have even weight",
                i
            );
        }
    }

    #[test]
    fn test_ebch_16_11_syndrome_zero_for_codewords() {
        let code = ExtendedBchCode::ebch_16_11();
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
    }

    /// Verify d_min = 4 for eBCH(16,11) by checking that no nonzero
    /// codeword has weight < 4.
    #[test]
    fn test_ebch_16_11_minimum_distance() {
        let code = ExtendedBchCode::ebch_16_11();
        let n = code.n();

        // Check that no weight-1 or weight-2 or weight-3 pattern is a codeword
        // (i.e., H * e^T != 0 for all such patterns).

        // Weight 1
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

        // Weight 2
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

        // Weight 3
        for i in 0..n {
            for j in (i + 1)..n {
                for l in (j + 1)..n {
                    let mut e = BitVec::zeros(n);
                    e.set(i, true);
                    e.set(j, true);
                    e.set(l, true);
                    let syn = code.inner().syndrome(&e).unwrap();
                    assert!(
                        syn.count_ones() > 0,
                        "weight-3 at ({},{},{}) has zero syndrome",
                        i,
                        j,
                        l
                    );
                }
            }
        }

        // Verify there exists a weight-4 codeword (d_min is exactly 4)
        let mut found_weight_4 = false;
        'outer: for i in 0..n {
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
                            found_weight_4 = true;
                            break 'outer;
                        }
                    }
                }
            }
        }
        assert!(found_weight_4, "d_min should be exactly 4");
    }

    /// Verify d_min = 4 for eBCH(32,26) by exhaustive check on weight 1..3.
    #[test]
    fn test_ebch_32_26_minimum_distance() {
        let code = ExtendedBchCode::ebch_32_26();
        let n = code.n();

        // Weight 1
        for i in 0..n {
            let mut e = BitVec::zeros(n);
            e.set(i, true);
            let syn = code.inner().syndrome(&e).unwrap();
            assert!(syn.count_ones() > 0);
        }

        // Weight 2
        for i in 0..n {
            for j in (i + 1)..n {
                let mut e = BitVec::zeros(n);
                e.set(i, true);
                e.set(j, true);
                let syn = code.inner().syndrome(&e).unwrap();
                assert!(syn.count_ones() > 0);
            }
        }

        // Weight 3
        for i in 0..n {
            for j in (i + 1)..n {
                for l in (j + 1)..n {
                    let mut e = BitVec::zeros(n);
                    e.set(i, true);
                    e.set(j, true);
                    e.set(l, true);
                    let syn = code.inner().syndrome(&e).unwrap();
                    assert!(syn.count_ones() > 0);
                }
            }
        }
    }

    /// Verify d_min = 4 for eBCH(64,57) by exhaustive check on weight 1..3.
    #[test]
    fn test_ebch_64_57_minimum_distance() {
        let code = ExtendedBchCode::ebch_64_57();
        let n = code.n();

        // Weight 1
        for i in 0..n {
            let mut e = BitVec::zeros(n);
            e.set(i, true);
            let syn = code.inner().syndrome(&e).unwrap();
            assert!(syn.count_ones() > 0);
        }

        // Weight 2
        for i in 0..n {
            for j in (i + 1)..n {
                let mut e = BitVec::zeros(n);
                e.set(i, true);
                e.set(j, true);
                let syn = code.inner().syndrome(&e).unwrap();
                assert!(syn.count_ones() > 0);
            }
        }

        // Weight 3
        for i in 0..n {
            for j in (i + 1)..n {
                for l in (j + 1)..n {
                    let mut e = BitVec::zeros(n);
                    e.set(i, true);
                    e.set(j, true);
                    e.set(l, true);
                    let syn = code.inner().syndrome(&e).unwrap();
                    assert!(syn.count_ones() > 0);
                }
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
        /// For any random message, the encoded codeword must have zero syndrome
        /// and even Hamming weight.
        #[test]
        fn prop_ebch_16_11_syndrome_zero_and_even(
            msg_bits in prop::collection::vec(any::<bool>(), 11)
        ) {
            let code = ExtendedBchCode::ebch_16_11();
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = code.encode(&msg);
            let syn = code.inner().syndrome(&cw).unwrap();
            prop_assert_eq!(syn.count_ones(), 0, "syndrome must be zero");
            prop_assert_eq!(cw.count_ones() % 2, 0, "codeword must have even weight");
        }

        #[test]
        fn prop_ebch_16_7_syndrome_zero_and_even(
            msg_bits in prop::collection::vec(any::<bool>(), 7)
        ) {
            let code = ExtendedBchCode::ebch_16_7();
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = code.encode(&msg);
            let syn = code.inner().syndrome(&cw).unwrap();
            prop_assert_eq!(syn.count_ones(), 0, "syndrome must be zero");
            prop_assert_eq!(cw.count_ones() % 2, 0, "codeword must have even weight");
        }

        #[test]
        fn prop_ebch_32_26_syndrome_zero_and_even(
            msg_bits in prop::collection::vec(any::<bool>(), 26)
        ) {
            let code = ExtendedBchCode::ebch_32_26();
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = code.encode(&msg);
            let syn = code.inner().syndrome(&cw).unwrap();
            prop_assert_eq!(syn.count_ones(), 0, "syndrome must be zero");
            prop_assert_eq!(cw.count_ones() % 2, 0, "codeword must have even weight");
        }

        #[test]
        fn prop_ebch_64_57_syndrome_zero_and_even(
            msg_bits in prop::collection::vec(any::<bool>(), 57)
        ) {
            let code = ExtendedBchCode::ebch_64_57();
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = code.encode(&msg);
            let syn = code.inner().syndrome(&cw).unwrap();
            prop_assert_eq!(syn.count_ones(), 0, "syndrome must be zero");
            prop_assert_eq!(cw.count_ones() % 2, 0, "codeword must have even weight");
        }
    }
}
