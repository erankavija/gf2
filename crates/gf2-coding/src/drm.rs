//! Decreasing Reed-Muller (dRM) codes.
//!
//! A decreasing Reed-Muller code dRM(n, k) is constructed by evaluating
//! monomials over GF(2)^m (where n = 2^m) at all 2^m points, selecting
//! monomials in decreasing monomial order until k rows are obtained.
//!
//! # Construction
//!
//! For m variables over GF(2), the decreasing monomial order sorts
//! monomials first by degree (ascending), then lexicographically within
//! each degree. The generator matrix has k rows, each being the evaluation
//! vector of one monomial across all 2^m points.
//!
//! ## dRM(32, 21) example
//!
//! With m=5 and n=32, the 21 monomials in increasing degree order are:
//!
//! - Degree 0: 1 (constant)
//! - Degree 1: x\_1, x\_2, x\_3, x\_4, x\_5
//! - Degree 2: x\_1 x\_2, x\_1 x\_3, x\_1 x\_4, x\_1 x\_5, x\_2 x\_3,
//!   x\_2 x\_4, x\_2 x\_5, x\_3 x\_4, x\_3 x\_5, x\_4 x\_5
//! - Degree 3 (first 5): x\_1 x\_2 x\_3, x\_1 x\_2 x\_4, x\_1 x\_2 x\_5,
//!   x\_1 x\_3 x\_4, x\_1 x\_3 x\_5
//!
//! This gives exactly 1 + 5 + 10 + 5 = 21 rows.
//!
//! # Systematic form
//!
//! After evaluating the monomials, the constructor applies Gaussian
//! elimination with column permutation to produce a systematic generator
//! matrix G = [I_k | P]. The parity-check matrix is H = [P^T | I_r].
//! As a result, the column ordering of the code differs from the raw
//! evaluation-matrix ordering — this is standard practice for GRAND
//! decoding, which only needs G and H with the orthogonality property.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::drm::DrmCode;
//! use gf2_coding::traits::BlockEncoder;
//! use gf2_core::BitVec;
//!
//! let code = DrmCode::drm_32_21();
//! assert_eq!(code.n(), 32);
//! assert_eq!(code.k(), 21);
//!
//! let msg = BitVec::ones(21);
//! let cw = code.encode(&msg);
//! assert_eq!(cw.len(), 32);
//! ```

use crate::linear::LinearBlockCode;
use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::{BitMatrix, BitVec};

/// A decreasing Reed-Muller code.
///
/// The code is stored internally as a [`LinearBlockCode`] with generator
/// and parity-check matrices computed from monomial evaluations.
///
/// # Examples
///
/// ```
/// use gf2_coding::drm::DrmCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_core::BitVec;
///
/// let code = DrmCode::drm_32_21();
/// let msg = BitVec::zeros(21);
/// let cw = code.encode(&msg);
/// assert_eq!(cw.len(), 32);
/// ```
#[derive(Debug, Clone)]
pub struct DrmCode {
    inner: LinearBlockCode,
}

impl DrmCode {
    /// Constructs a decreasing Reed-Muller code with m variables and k
    /// monomials in decreasing order.
    ///
    /// # Arguments
    ///
    /// * `m` - Number of variables (n = 2^m evaluation points)
    /// * `k` - Number of monomials (rows of the generator matrix)
    ///
    /// # Panics
    ///
    /// Panics if `k` exceeds 2^m (more monomials than evaluation points)
    /// or if `m` is 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    ///
    /// // RM(2,5) has 16 monomials of degree <= 2
    /// let code = DrmCode::new(5, 16);
    /// assert_eq!(code.n(), 32);
    /// assert_eq!(code.k(), 16);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(k * 2^m) for monomial evaluation, plus O(k^2 * n) for Gaussian
    /// elimination to produce the systematic generator matrix.
    pub fn new(m: usize, k: usize) -> Self {
        assert!(m > 0, "m must be positive");
        let n = 1usize << m;
        assert!(k <= n, "k must not exceed 2^m = {}", n);

        // Enumerate all monomials in degree order, lexicographic within degree.
        let monomials = Self::enumerate_monomials(m, k);
        assert_eq!(monomials.len(), k);

        // Evaluate each monomial at all 2^m points of GF(2)^m.
        let mut g = BitMatrix::zeros(k, n);
        for (row, mono) in monomials.iter().enumerate() {
            for point in 0..n {
                let val = Self::evaluate_monomial(mono, point, m);
                if val {
                    g.set(row, point, true);
                }
            }
        }

        // Put G in systematic form via row reduction, and compute H.
        let (g_sys, h) = Self::systematic_form(g, k, n);

        let inner = LinearBlockCode::new_systematic(g_sys, Some(h));
        Self { inner }
    }

    /// Creates the dRM(32,21) code.
    ///
    /// This code uses m=5 variables (n=32) and the first 21 monomials in
    /// decreasing order: all degree-0, degree-1, degree-2 monomials plus
    /// the first 5 degree-3 monomials in lexicographic order.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = DrmCode::drm_32_21();
    /// assert_eq!(code.n(), 32);
    /// assert_eq!(code.k(), 21);
    ///
    /// let msg = BitVec::ones(21);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.len(), 32);
    /// ```
    pub fn drm_32_21() -> Self {
        Self::new(5, 21)
    }

    /// Returns the codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    ///
    /// assert_eq!(DrmCode::drm_32_21().n(), 32);
    /// ```
    pub fn n(&self) -> usize {
        self.inner.n()
    }

    /// Returns the message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    ///
    /// assert_eq!(DrmCode::drm_32_21().k(), 21);
    /// ```
    pub fn k(&self) -> usize {
        self.inner.k()
    }

    /// Returns the parity-check matrix H.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    ///
    /// let code = DrmCode::drm_32_21();
    /// let h = code.parity_check();
    /// assert_eq!(h.rows(), 32 - 21);
    /// assert_eq!(h.cols(), 32);
    /// ```
    pub fn parity_check(&self) -> &BitMatrix {
        self.inner
            .parity_check()
            .expect("dRM code always has H matrix")
    }

    /// Returns the inner [`LinearBlockCode`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    ///
    /// let code = DrmCode::drm_32_21();
    /// let inner = code.inner();
    /// assert_eq!(inner.n(), 32);
    /// ```
    pub fn inner(&self) -> &LinearBlockCode {
        &self.inner
    }

    /// Enumerates the first `k` monomials in decreasing monomial order
    /// over `m` variables.
    ///
    /// Each monomial is represented as a bitmask where bit i indicates
    /// that variable x\_i appears in the product.
    fn enumerate_monomials(m: usize, k: usize) -> Vec<u32> {
        let mut monomials = Vec::with_capacity(k);

        // Group by degree, enumerate lexicographically within each degree.
        for degree in 0..=m {
            let combos = Self::combinations(m, degree);
            for combo in combos {
                if monomials.len() >= k {
                    return monomials;
                }
                monomials.push(combo);
            }
        }

        monomials
    }

    /// Returns all k-element subsets of {0, 1, ..., m-1} as bitmasks,
    /// in lexicographic order.
    fn combinations(m: usize, degree: usize) -> Vec<u32> {
        let mut result = Vec::new();
        if degree == 0 {
            result.push(0u32); // constant monomial
            return result;
        }
        if degree > m {
            return result;
        }
        Self::combinations_helper(m, degree, 0, 0, &mut result);
        result
    }

    fn combinations_helper(
        m: usize,
        remaining: usize,
        start: usize,
        current: u32,
        result: &mut Vec<u32>,
    ) {
        if remaining == 0 {
            result.push(current);
            return;
        }
        if start + remaining > m {
            return;
        }
        for i in start..m {
            Self::combinations_helper(m, remaining - 1, i + 1, current | (1 << i), result);
        }
    }

    /// Evaluates a monomial (given as a variable bitmask) at a point of GF(2)^m.
    ///
    /// The point is encoded as an integer where bit i is the value of variable x\_i.
    fn evaluate_monomial(monomial: &u32, point: usize, _m: usize) -> bool {
        // The monomial evaluates to 1 iff all variables in the monomial are 1
        // at the given point.
        let mono = *monomial as usize;
        (point & mono) == mono
    }

    /// Converts a generator matrix to systematic form [I_k | P] via row
    /// reduction, and computes H = [P^T | I_r].
    fn systematic_form(g: BitMatrix, k: usize, n: usize) -> (BitMatrix, BitMatrix) {
        let r = n - k;
        let mut work = g;

        // Gaussian elimination to get RREF
        let mut pivot_cols = Vec::with_capacity(k);
        let mut current_row = 0;

        for col in 0..n {
            if current_row >= k {
                break;
            }
            // Find pivot in this column
            let mut pivot = None;
            for row in current_row..k {
                if work.get(row, col) {
                    pivot = Some(row);
                    break;
                }
            }
            if let Some(pivot_row) = pivot {
                // Swap rows
                if pivot_row != current_row {
                    work.swap_rows(current_row, pivot_row);
                }
                // Eliminate all other rows
                for row in 0..k {
                    if row != current_row && work.get(row, col) {
                        work.row_xor(row, current_row);
                    }
                }
                pivot_cols.push(col);
                current_row += 1;
            }
        }

        assert_eq!(
            pivot_cols.len(),
            k,
            "Generator matrix must have rank k = {}",
            k
        );

        // Now rearrange columns so pivot columns come first (systematic form).
        // Build a column permutation: pivot_cols first, then the rest.
        let non_pivot_cols: Vec<usize> = (0..n).filter(|c| !pivot_cols.contains(c)).collect();
        assert_eq!(non_pivot_cols.len(), r);

        let mut g_sys = BitMatrix::zeros(k, n);
        for row in 0..k {
            for (new_col, &old_col) in pivot_cols.iter().enumerate() {
                g_sys.set(row, new_col, work.get(row, old_col));
            }
            for (idx, &old_col) in non_pivot_cols.iter().enumerate() {
                g_sys.set(row, k + idx, work.get(row, old_col));
            }
        }

        // Build H = [P^T | I_r]
        let mut h = BitMatrix::zeros(r, n);
        for i in 0..r {
            for j in 0..k {
                h.set(i, j, g_sys.get(j, k + i));
            }
            h.set(i, k + i, true);
        }

        // We also need to un-permute the columns so the code operates on
        // the original coordinate system. Build the full permuted G and H.
        // Actually, for a code defined by evaluation, the column permutation
        // just reorders the coordinate positions. The code is the same code
        // in a permuted coordinate system. For our purposes (GRAND decoding
        // with H matrix), this is fine as long as G and H are consistent.

        (g_sys, h)
    }
}

impl BlockEncoder for DrmCode {
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

impl GeneratorMatrixAccess for DrmCode {
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
    fn test_drm_32_21_parameters() {
        let code = DrmCode::drm_32_21();
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 21);
    }

    #[test]
    fn test_drm_32_21_orthogonality() {
        let code = DrmCode::drm_32_21();
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
    fn test_drm_32_21_syndrome_zero() {
        let code = DrmCode::drm_32_21();
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

    #[test]
    fn test_drm_32_21_all_zeros() {
        let code = DrmCode::drm_32_21();
        let cw = code.encode(&BitVec::zeros(code.k()));
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_drm_32_21_all_ones() {
        let code = DrmCode::drm_32_21();
        let cw = code.encode(&BitVec::ones(code.k()));
        let syn = code.inner().syndrome(&cw).unwrap();
        assert_eq!(syn.count_ones(), 0);
    }

    #[test]
    fn test_drm_rm_2_5_is_subcode() {
        // RM(2,5) = dRM(32,16) should produce a valid code
        let code = DrmCode::new(5, 16);
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 16);

        let g = code.generator_matrix();
        let h = code.parity_check();
        let h_t = h.transpose();
        let product = &g * &h_t;
        for i in 0..product.rows() {
            for j in 0..product.cols() {
                assert!(!product.get(i, j));
            }
        }
    }

    #[test]
    fn test_drm_rm_1_5() {
        // RM(1,5) = dRM(32,6)
        let code = DrmCode::new(5, 6);
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 6);
    }

    #[test]
    fn test_monomial_enumeration() {
        // For m=3, degree ordering should be:
        // deg 0: {} (constant) -> 1 monomial
        // deg 1: {0}, {1}, {2} -> 3 monomials
        // deg 2: {0,1}, {0,2}, {1,2} -> 3 monomials
        // deg 3: {0,1,2} -> 1 monomial
        // Total: 8 = 2^3

        let monos = DrmCode::enumerate_monomials(3, 8);
        assert_eq!(monos.len(), 8);
        assert_eq!(monos[0], 0b000); // constant
        assert_eq!(monos[1], 0b001); // x0
        assert_eq!(monos[2], 0b010); // x1
        assert_eq!(monos[3], 0b100); // x2
        assert_eq!(monos[4], 0b011); // x0*x1
        assert_eq!(monos[5], 0b101); // x0*x2
        assert_eq!(monos[6], 0b110); // x1*x2
        assert_eq!(monos[7], 0b111); // x0*x1*x2
    }

    #[test]
    fn test_evaluate_monomial_constant() {
        // Constant monomial (no variables) evaluates to 1 at every point
        for point in 0..8 {
            assert!(DrmCode::evaluate_monomial(&0, point, 3));
        }
    }

    #[test]
    fn test_evaluate_monomial_single_var() {
        // x0 = variable 0: evaluates to bit 0 of the point
        for point in 0..8 {
            let expected = (point & 1) == 1;
            assert_eq!(DrmCode::evaluate_monomial(&1, point, 3), expected);
        }
    }

    #[test]
    fn test_evaluate_monomial_product() {
        // x0*x1 (mask = 0b11): evaluates to 1 only when both bits 0 and 1 are set
        for point in 0..8 {
            let expected = (point & 0b11) == 0b11;
            assert_eq!(DrmCode::evaluate_monomial(&0b11, point, 3), expected);
        }
    }

    /// Verify minimum distance by checking weight-1 and weight-2 patterns.
    #[test]
    fn test_drm_32_21_minimum_distance_lower_bound() {
        let code = DrmCode::drm_32_21();
        let n = code.n();

        // Weight 1
        for i in 0..n {
            let mut e = BitVec::zeros(n);
            e.set(i, true);
            let syn = code.inner().syndrome(&e).unwrap();
            assert!(syn.count_ones() > 0, "weight-1 at {} has zero syndrome", i);
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
        fn prop_drm_32_21_syndrome_zero(
            msg_bits in prop::collection::vec(any::<bool>(), 21)
        ) {
            let code = DrmCode::drm_32_21();
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = code.encode(&msg);
            let syn = code.inner().syndrome(&cw).unwrap();
            prop_assert_eq!(syn.count_ones(), 0, "syndrome must be zero for valid codeword");
        }

        /// The sum of two codewords must be a codeword (linearity).
        #[test]
        fn prop_drm_32_21_linearity(
            msg1_bits in prop::collection::vec(any::<bool>(), 21),
            msg2_bits in prop::collection::vec(any::<bool>(), 21)
        ) {
            let code = DrmCode::drm_32_21();

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
