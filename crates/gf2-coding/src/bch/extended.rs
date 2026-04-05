//! Extended BCH codes for GRAND decoding.
//!
//! An extended BCH code eBCH(n+1, k) is constructed from a standard BCH(n, k, t)
//! code by appending an overall parity-check bit. This forces all codewords to
//! have even Hamming weight, increasing the minimum distance by 1 (from odd to even).
//!
//! # Construction
//!
//! Given BCH(n, k, t) with generator matrix G (k × n) and parity-check matrix H ((n-k) × n):
//!
//! 1. **Generator matrix**: G_ext (k × (n+1)) — append a parity column so each row has even weight.
//! 2. **Parity-check matrix**: H_ext ((n-k+1) × (n+1)) — add an all-ones row and a column of zeros
//!    (except the last entry which is 1).
//!
//! # GRAND Interface
//!
//! Each code exposes:
//! - `h()` — parity-check matrix H for syndrome computation
//! - `n()`, `k()` — code dimensions
//! - `is_even()` — `true` for eBCH (enables GRAND weight-parity optimization)
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::extended::ExtendedBchCode;
//! use gf2_coding::traits::{BlockEncoder, GeneratorMatrixAccess};
//! use gf2_core::BitVec;
//!
//! let code = ExtendedBchCode::ebch_16_11();
//! assert_eq!(code.n(), 16);
//! assert_eq!(code.k(), 11);
//! assert!(code.is_even());
//!
//! // Encode a message
//! let msg = BitVec::ones(11);
//! let cw = code.encode(&msg);
//! assert_eq!(cw.len(), 16);
//! assert_eq!(cw.count_ones() % 2, 0); // even weight
//! ```

use crate::bch::BchCode;
use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};

/// An extended BCH code formed by appending an overall parity-check bit.
///
/// The extension increases the minimum distance by 1 (from 2t+1 to 2t+2)
/// and forces all codewords to have even Hamming weight. This property
/// enables the weight-parity optimization in GRAND decoders.
///
/// # Examples
///
/// ```
/// use gf2_coding::bch::extended::ExtendedBchCode;
/// use gf2_coding::traits::GeneratorMatrixAccess;
///
/// let code = ExtendedBchCode::ebch_16_11();
/// assert_eq!(code.n(), 16);
/// assert_eq!(code.k(), 11);
/// assert!(code.is_even());
/// ```
#[derive(Clone, Debug)]
pub struct ExtendedBchCode {
    /// The underlying BCH code
    inner: BchCode,
    /// Cached extended generator matrix (k × (n+1))
    g_ext: BitMatrix,
    /// Cached extended parity-check matrix ((n-k+1) × (n+1))
    h_ext: BitMatrix,
    /// Extended code length (n + 1)
    n_ext: usize,
    /// Message length (same as inner code)
    k: usize,
    /// Error correction capability of the inner BCH code
    t: usize,
}

impl ExtendedBchCode {
    /// Creates an extended BCH code from an underlying BCH code.
    ///
    /// Given BCH(n, k, t), produces eBCH(n+1, k) with minimum distance 2t+2.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying BCH(n, k, t) code
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::{BchCode, extended::ExtendedBchCode};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let bch = BchCode::new(15, 11, 1, field);
    /// let ebch = ExtendedBchCode::new(bch);
    /// assert_eq!(ebch.n(), 16);
    /// assert_eq!(ebch.k(), 11);
    /// ```
    pub fn new(inner: BchCode) -> Self {
        let n = inner.n();
        let k = inner.k();
        let t = inner.t();
        let n_ext = n + 1;

        // Compute extended generator matrix: append parity column
        let g_inner = <BchCode as GeneratorMatrixAccess>::generator_matrix(&inner);
        let g_ext = Self::extend_generator(&g_inner, k, n, n_ext);

        // Compute extended parity-check matrix
        let h_ext = Self::compute_h_from_g(&g_ext, k, n_ext);

        Self {
            inner,
            g_ext,
            h_ext,
            n_ext,
            k,
            t,
        }
    }

    /// Extends the generator matrix by appending an overall parity column.
    ///
    /// For each row of G, the parity bit is the XOR of all existing bits in that row.
    /// This ensures every row (and hence every codeword) has even Hamming weight.
    fn extend_generator(g: &BitMatrix, k: usize, n: usize, n_ext: usize) -> BitMatrix {
        let mut g_ext = BitMatrix::zeros(k, n_ext);

        for row in 0..k {
            // Copy original columns
            let mut parity = false;
            for col in 0..n {
                let bit = g.get(row, col);
                g_ext.set(row, col, bit);
                parity ^= bit;
            }
            // Append parity bit (position n)
            g_ext.set(row, n, parity);
        }

        g_ext
    }

    /// Computes H from G using the dual code relationship: H * G^T = 0.
    ///
    /// For a systematic code G = [I_k | P], H = [-P^T | I_{n-k}] = [P^T | I_{n-k}]
    /// (over GF(2), negation is identity).
    ///
    /// We use Gaussian elimination to find the null space of G.
    fn compute_h_from_g(g: &BitMatrix, k: usize, n: usize) -> BitMatrix {
        let r = n - k; // number of parity-check rows

        // Put G into RREF to identify systematic structure
        // Build augmented matrix [G | I_k] and row-reduce to find the null space
        // Instead, use the identity: for systematic G = [I_k | P],
        // H = [P^T | I_r]

        // First, get G into systematic form [I_k | P] via row reduction
        let mut work = g.clone();
        // Gaussian elimination to get identity in first k columns
        for col in 0..k {
            // Find pivot
            if let Some(pivot_row) = work.find_pivot_row(col, col) {
                if pivot_row != col {
                    work.swap_rows(col, pivot_row);
                }
                // Eliminate other rows
                for row in 0..k {
                    if row != col && work.get(row, col) {
                        work.row_xor(row, col);
                    }
                }
            }
        }

        // Now work = [I_k | P], extract P (k × r)
        // H = [P^T | I_r] which is (r × n)
        let mut h = BitMatrix::zeros(r, n);

        // P^T part: columns 0..k of H
        for i in 0..r {
            for j in 0..k {
                // P[j][i] = work[j][k + i]
                h.set(i, j, work.get(j, k + i));
            }
        }

        // I_r part: columns k..n of H
        for i in 0..r {
            h.set(i, k + i, true);
        }

        h
    }

    /// Returns the extended codeword length (n + 1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert_eq!(code.n(), 16);
    /// ```
    pub fn n(&self) -> usize {
        self.n_ext
    }

    /// Returns the message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert_eq!(code.k(), 11);
    /// ```
    pub fn k(&self) -> usize {
        self.k
    }

    /// Returns the error correction capability of the inner BCH code.
    ///
    /// The extended code has minimum distance 2t+2, so it can correct t errors
    /// and detect t+1 errors.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert_eq!(code.t(), 1);
    /// ```
    pub fn t(&self) -> usize {
        self.t
    }

    /// Returns the minimum distance of the extended code (2t + 2).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert_eq!(code.min_distance(), 4); // 2*1 + 2
    /// ```
    pub fn min_distance(&self) -> usize {
        2 * self.t + 2
    }

    /// Returns whether all codewords have even Hamming weight.
    ///
    /// Always `true` for extended BCH codes. Used by GRAND decoders to skip
    /// odd-weight error patterns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// assert!(ExtendedBchCode::ebch_16_11().is_even());
    /// ```
    pub fn is_even(&self) -> bool {
        true
    }

    /// Returns a reference to the extended parity-check matrix.
    ///
    /// The matrix has dimensions (n-k+1) × (n+1) for the extended code,
    /// or equivalently r × n_ext where r = n_ext - k.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// let h = code.h();
    /// assert_eq!(h.rows(), 5);  // 16 - 11
    /// assert_eq!(h.cols(), 16);
    /// ```
    pub fn h(&self) -> &BitMatrix {
        &self.h_ext
    }

    /// Returns a reference to the extended generator matrix.
    ///
    /// The matrix has dimensions k × (n+1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// let g = code.g();
    /// assert_eq!(g.rows(), 11);
    /// assert_eq!(g.cols(), 16);
    /// ```
    pub fn g(&self) -> &BitMatrix {
        &self.g_ext
    }

    /// Returns a reference to the underlying BCH code.
    pub fn inner(&self) -> &BchCode {
        &self.inner
    }

    /// Computes the syndrome of a received word.
    ///
    /// Returns the zero vector for valid codewords.
    ///
    /// # Arguments
    ///
    /// * `received` - A received word of length n_ext
    ///
    /// # Panics
    ///
    /// Panics if `received.len() != n()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// let msg = BitVec::ones(11);
    /// let cw = code.encode(&msg);
    /// let syn = code.syndrome(&cw);
    /// assert_eq!(syn.count_ones(), 0);
    /// ```
    pub fn syndrome(&self, received: &BitVec) -> BitVec {
        assert_eq!(
            received.len(),
            self.n_ext,
            "Received word must have length n = {}",
            self.n_ext
        );

        self.h_ext.matvec(received)
    }

    // ---- Factory constructors for specific codes used in GRAND ----

    /// Creates eBCH(16, 11) — BCH(15, 11, 1) extended by one parity bit.
    ///
    /// - Inner code: BCH(15, 11, 1) over GF(2^4) with primitive polynomial x^4+x+1
    /// - Minimum distance: 4 (= 2*1 + 2)
    /// - Error correction: t=1
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
    /// assert_eq!(code.t(), 1);
    /// assert_eq!(code.min_distance(), 4);
    ///
    /// let cw = code.encode(&BitVec::ones(11));
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_16_11() -> Self {
        let field = Gf2mField::new(4, 0b10011).with_tables(); // x^4 + x + 1
        let bch = BchCode::new(15, 11, 1, field);
        Self::new(bch)
    }

    /// Creates eBCH(16, 7) — BCH(15, 7, 2) extended by one parity bit.
    ///
    /// - Inner code: BCH(15, 7, 2) over GF(2^4)
    /// - Minimum distance: 6 (= 2*2 + 2)
    /// - Error correction: t=2
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
    /// assert_eq!(code.t(), 2);
    /// assert_eq!(code.min_distance(), 6);
    ///
    /// let cw = code.encode(&BitVec::ones(7));
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_16_7() -> Self {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let bch = BchCode::new(15, 7, 2, field);
        Self::new(bch)
    }

    /// Creates eBCH(32, 26) — BCH(31, 26, 1) extended by one parity bit.
    ///
    /// - Inner code: BCH(31, 26, 1) over GF(2^5) with primitive polynomial x^5+x^2+1
    /// - Minimum distance: 4 (= 2*1 + 2)
    /// - Error correction: t=1
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
    /// assert_eq!(code.t(), 1);
    /// assert_eq!(code.min_distance(), 4);
    ///
    /// let cw = code.encode(&BitVec::ones(26));
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_32_26() -> Self {
        let field = Gf2mField::new(5, 0b100101).with_tables(); // x^5 + x^2 + 1
        let bch = BchCode::new(31, 26, 1, field);
        Self::new(bch)
    }

    /// Creates eBCH(64, 57) — BCH(63, 57, 1) extended by one parity bit.
    ///
    /// - Inner code: BCH(63, 57, 1) over GF(2^6) with primitive polynomial x^6+x+1
    /// - Minimum distance: 4 (= 2*1 + 2)
    /// - Error correction: t=1
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
    /// assert_eq!(code.t(), 1);
    /// assert_eq!(code.min_distance(), 4);
    ///
    /// let cw = code.encode(&BitVec::ones(57));
    /// assert_eq!(cw.count_ones() % 2, 0);
    /// ```
    pub fn ebch_64_57() -> Self {
        let field = Gf2mField::new(6, 0b1000011).with_tables(); // x^6 + x + 1
        let bch = BchCode::new(63, 57, 1, field);
        Self::new(bch)
    }
}

impl BlockEncoder for ExtendedBchCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n_ext
    }

    /// Encodes a message using the extended generator matrix.
    ///
    /// The codeword is computed as `c = m * G_ext` where G_ext is the
    /// extended generator matrix. All codewords have even Hamming weight.
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

        // Compute codeword = message * G_ext
        let mut msg_matrix = BitMatrix::zeros(1, self.k);
        for i in 0..self.k {
            msg_matrix.set(0, i, message.get(i));
        }

        let cw_matrix = &msg_matrix * &self.g_ext;
        cw_matrix.row_as_bitvec(0)
    }
}

impl GeneratorMatrixAccess for ExtendedBchCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n_ext
    }

    fn generator_matrix(&self) -> BitMatrix {
        self.g_ext.clone()
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
    fn test_ebch_16_11_dimensions() {
        let code = ExtendedBchCode::ebch_16_11();
        assert_eq!(code.n(), 16);
        assert_eq!(code.k(), 11);
        assert_eq!(code.t(), 1);
        assert_eq!(code.min_distance(), 4);
        assert!(code.is_even());
    }

    #[test]
    fn test_ebch_16_7_dimensions() {
        let code = ExtendedBchCode::ebch_16_7();
        assert_eq!(code.n(), 16);
        assert_eq!(code.k(), 7);
        assert_eq!(code.t(), 2);
        assert_eq!(code.min_distance(), 6);
        assert!(code.is_even());
    }

    #[test]
    fn test_ebch_32_26_dimensions() {
        let code = ExtendedBchCode::ebch_32_26();
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 26);
        assert_eq!(code.t(), 1);
        assert_eq!(code.min_distance(), 4);
        assert!(code.is_even());
    }

    #[test]
    fn test_ebch_64_57_dimensions() {
        let code = ExtendedBchCode::ebch_64_57();
        assert_eq!(code.n(), 64);
        assert_eq!(code.k(), 57);
        assert_eq!(code.t(), 1);
        assert_eq!(code.min_distance(), 4);
        assert!(code.is_even());
    }

    // ---- Matrix dimension tests ----

    #[test]
    fn test_ebch_16_11_matrix_dimensions() {
        let code = ExtendedBchCode::ebch_16_11();
        assert_eq!(code.g().rows(), 11);
        assert_eq!(code.g().cols(), 16);
        assert_eq!(code.h().rows(), 5);
        assert_eq!(code.h().cols(), 16);
    }

    #[test]
    fn test_ebch_16_7_matrix_dimensions() {
        let code = ExtendedBchCode::ebch_16_7();
        assert_eq!(code.g().rows(), 7);
        assert_eq!(code.g().cols(), 16);
        assert_eq!(code.h().rows(), 9);
        assert_eq!(code.h().cols(), 16);
    }

    #[test]
    fn test_ebch_32_26_matrix_dimensions() {
        let code = ExtendedBchCode::ebch_32_26();
        assert_eq!(code.g().rows(), 26);
        assert_eq!(code.g().cols(), 32);
        assert_eq!(code.h().rows(), 6);
        assert_eq!(code.h().cols(), 32);
    }

    #[test]
    fn test_ebch_64_57_matrix_dimensions() {
        let code = ExtendedBchCode::ebch_64_57();
        assert_eq!(code.g().rows(), 57);
        assert_eq!(code.g().cols(), 64);
        assert_eq!(code.h().rows(), 7);
        assert_eq!(code.h().cols(), 64);
    }

    // ---- H * G^T = 0 tests ----

    #[test]
    fn test_ebch_16_11_h_gt_zero() {
        let code = ExtendedBchCode::ebch_16_11();
        let gt = code.g().transpose();
        let product = code.h() * &gt;
        for r in 0..product.rows() {
            for c in 0..product.cols() {
                assert!(
                    !product.get(r, c),
                    "H * G^T not zero at ({}, {}) for eBCH(16,11)",
                    r,
                    c
                );
            }
        }
    }

    #[test]
    fn test_ebch_16_7_h_gt_zero() {
        let code = ExtendedBchCode::ebch_16_7();
        let gt = code.g().transpose();
        let product = code.h() * &gt;
        for r in 0..product.rows() {
            for c in 0..product.cols() {
                assert!(
                    !product.get(r, c),
                    "H * G^T not zero at ({}, {}) for eBCH(16,7)",
                    r,
                    c
                );
            }
        }
    }

    #[test]
    fn test_ebch_32_26_h_gt_zero() {
        let code = ExtendedBchCode::ebch_32_26();
        let gt = code.g().transpose();
        let product = code.h() * &gt;
        for r in 0..product.rows() {
            for c in 0..product.cols() {
                assert!(
                    !product.get(r, c),
                    "H * G^T not zero at ({}, {}) for eBCH(32,26)",
                    r,
                    c
                );
            }
        }
    }

    #[test]
    fn test_ebch_64_57_h_gt_zero() {
        let code = ExtendedBchCode::ebch_64_57();
        let gt = code.g().transpose();
        let product = code.h() * &gt;
        for r in 0..product.rows() {
            for c in 0..product.cols() {
                assert!(
                    !product.get(r, c),
                    "H * G^T not zero at ({}, {}) for eBCH(64,57)",
                    r,
                    c
                );
            }
        }
    }

    // ---- Even-weight tests ----

    #[test]
    fn test_ebch_16_11_all_codewords_even_weight() {
        let code = ExtendedBchCode::ebch_16_11();
        // Exhaustive check: 2^11 = 2048 messages
        for i in 0u32..(1 << 11) {
            let mut msg = BitVec::new();
            for bit in 0..11 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for message {} has odd weight",
                i
            );
        }
    }

    #[test]
    fn test_ebch_16_7_all_codewords_even_weight() {
        let code = ExtendedBchCode::ebch_16_7();
        // Exhaustive check: 2^7 = 128 messages
        for i in 0u32..(1 << 7) {
            let mut msg = BitVec::new();
            for bit in 0..7 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for message {} has odd weight",
                i
            );
        }
    }

    #[test]
    fn test_ebch_32_26_sample_codewords_even_weight() {
        let code = ExtendedBchCode::ebch_32_26();
        // Sample check: test specific messages
        for seed in 0u32..100 {
            let mut msg = BitVec::new();
            for bit in 0..26 {
                msg.push_bit(((seed.wrapping_mul(31).wrapping_add(bit)) % 2) == 0);
            }
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for seed {} has odd weight",
                seed
            );
        }
    }

    #[test]
    fn test_ebch_64_57_sample_codewords_even_weight() {
        let code = ExtendedBchCode::ebch_64_57();
        for seed in 0u32..100 {
            let mut msg = BitVec::new();
            for bit in 0..57 {
                msg.push_bit(((seed.wrapping_mul(37).wrapping_add(bit)) % 2) == 0);
            }
            let cw = code.encode(&msg);
            assert_eq!(
                cw.count_ones() % 2,
                0,
                "Codeword for seed {} has odd weight",
                seed
            );
        }
    }

    // ---- Syndrome tests ----

    #[test]
    fn test_ebch_16_11_zero_syndrome_for_codewords() {
        let code = ExtendedBchCode::ebch_16_11();
        // Check that syndrome is zero for all codewords
        for i in 0u32..(1 << 11) {
            let mut msg = BitVec::new();
            for bit in 0..11 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);
            let syn = code.syndrome(&cw);
            assert_eq!(syn.count_ones(), 0, "Non-zero syndrome for message {}", i);
        }
    }

    #[test]
    fn test_ebch_16_7_zero_syndrome_for_codewords() {
        let code = ExtendedBchCode::ebch_16_7();
        for i in 0u32..(1 << 7) {
            let mut msg = BitVec::new();
            for bit in 0..7 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);
            let syn = code.syndrome(&cw);
            assert_eq!(syn.count_ones(), 0, "Non-zero syndrome for message {}", i);
        }
    }

    #[test]
    fn test_ebch_32_26_zero_syndrome_for_sample_codewords() {
        let code = ExtendedBchCode::ebch_32_26();
        for seed in 0u32..100 {
            let mut msg = BitVec::new();
            for bit in 0..26 {
                msg.push_bit(((seed.wrapping_mul(31).wrapping_add(bit)) % 2) == 0);
            }
            let cw = code.encode(&msg);
            let syn = code.syndrome(&cw);
            assert_eq!(syn.count_ones(), 0, "Non-zero syndrome for seed {}", seed);
        }
    }

    #[test]
    fn test_ebch_64_57_zero_syndrome_for_sample_codewords() {
        let code = ExtendedBchCode::ebch_64_57();
        for seed in 0u32..100 {
            let mut msg = BitVec::new();
            for bit in 0..57 {
                msg.push_bit(((seed.wrapping_mul(37).wrapping_add(bit)) % 2) == 0);
            }
            let cw = code.encode(&msg);
            let syn = code.syndrome(&cw);
            assert_eq!(syn.count_ones(), 0, "Non-zero syndrome for seed {}", seed);
        }
    }

    // ---- Non-zero syndrome for errors ----

    #[test]
    fn test_ebch_16_11_nonzero_syndrome_for_single_error() {
        let code = ExtendedBchCode::ebch_16_11();
        let msg = BitVec::ones(11);
        let cw = code.encode(&msg);

        for pos in 0..16 {
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

    // ---- Minimum distance tests ----

    #[test]
    fn test_ebch_16_11_minimum_distance() {
        let code = ExtendedBchCode::ebch_16_11();
        let mut min_weight = usize::MAX;

        // Check all non-zero codewords (2^11 - 1 = 2047)
        for i in 1u32..(1 << 11) {
            let mut msg = BitVec::new();
            for bit in 0..11 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);
            let w = cw.count_ones();
            if w < min_weight {
                min_weight = w;
            }
        }

        assert_eq!(
            min_weight, 4,
            "Minimum distance should be 4 for eBCH(16,11)"
        );
    }

    #[test]
    fn test_ebch_16_7_minimum_distance() {
        let code = ExtendedBchCode::ebch_16_7();
        let mut min_weight = usize::MAX;

        // Check all non-zero codewords (2^7 - 1 = 127)
        for i in 1u32..(1 << 7) {
            let mut msg = BitVec::new();
            for bit in 0..7 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);
            let w = cw.count_ones();
            if w < min_weight {
                min_weight = w;
            }
        }

        assert_eq!(min_weight, 6, "Minimum distance should be 6 for eBCH(16,7)");
    }

    // ---- GeneratorMatrixAccess trait ----

    #[test]
    fn test_ebch_generator_matrix_access_trait() {
        let code = ExtendedBchCode::ebch_16_11();
        let g = <ExtendedBchCode as GeneratorMatrixAccess>::generator_matrix(&code);
        assert_eq!(g.rows(), 11);
        assert_eq!(g.cols(), 16);
        assert!(<ExtendedBchCode as GeneratorMatrixAccess>::is_systematic(
            &code
        ));
    }

    // ---- Encoding correctness: message recovery ----

    #[test]
    fn test_ebch_16_11_systematic_encoding() {
        let code = ExtendedBchCode::ebch_16_11();

        // For systematic codes, the first k bits of the codeword should be the message
        for i in 0u32..32 {
            let mut msg = BitVec::new();
            for bit in 0..11 {
                msg.push_bit((i >> bit) & 1 == 1);
            }
            let cw = code.encode(&msg);

            // Check systematic property: first k bits = message
            for bit in 0..11 {
                assert_eq!(
                    cw.get(bit),
                    msg.get(bit),
                    "Systematic bit {} mismatch for message {}",
                    bit,
                    i
                );
            }
        }
    }

    // ---- From generic constructor ----

    #[test]
    fn test_ebch_from_generic_bch() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let bch = BchCode::new(15, 11, 1, field);
        let code = ExtendedBchCode::new(bch);

        assert_eq!(code.n(), 16);
        assert_eq!(code.k(), 11);
        assert_eq!(code.t(), 1);

        let msg = BitVec::ones(11);
        let cw = code.encode(&msg);
        assert_eq!(cw.count_ones() % 2, 0);
    }
}
