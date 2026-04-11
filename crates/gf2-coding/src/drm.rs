//! Dynamic Reed-Muller (dRM) codes.
//!
//! A dynamic Reed-Muller code is constructed from the polar transform
//! matrix G\_N = G\_2^{⊗m} (Kronecker power of the 2×2 Hadamard kernel),
//! selecting rows to form a (n, k) code with improved minimum distance
//! properties compared to standard monomial-degree-based RM subcodes.
//!
//! # Construction
//!
//! The [`DrmCode::new`] constructor builds a generic RM subcode from
//! monomial evaluations (degree-then-lexicographic order). For the
//! flagship [`DrmCode::drm_32_21`], a stronger construction is used:
//!
//! 1. Start with the 16 RM(2,5) rows (polar transform indices with
//!    popcount ≥ 3), which form a (32, 16, 8) code.
//! 2. Add 5 extension rows — random linear combinations of G\_32 rows,
//!    selected by greedy d\_min maximization — to reach k=21.
//! 3. The resulting (32, 21, 6) code has d\_min=6, the maximum achievable
//!    for any (32, 21) code by the Hamming sphere-packing bound.
//!
//! This follows the dynamic frozen-bit construction of Coskun & Pfister
//! (arxiv:2103.16680), where frozen bit values are linear combinations
//! of preceding information bits. The extension rows in our construction
//! are members of the dRM ensemble defined therein.
//!
//! # Systematic form
//!
//! The constructor applies Gaussian elimination with column permutation
//! to produce a systematic generator matrix G = [I\_k | P]. The
//! parity-check matrix is H = [P^T | I\_r]. This is standard practice
//! for GRAND decoding, which only needs G and H with the orthogonality
//! property.
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

    /// Creates the dRM(32,21) code using the dynamic frozen-bit construction.
    ///
    /// This is a (32, 21, 6) code built from the polar transform G_32:
    /// 16 RM(2,5) rows (indices with popcount ≥ 3) plus 5 extension rows
    /// selected by greedy d_min maximization with dynamic frozen-bit
    /// constraints (Coskun & Pfister, arxiv:2103.16680).
    ///
    /// The resulting code has d_min=6 (the maximum achievable for (32,21)
    /// by the Hamming sphere-packing bound), compared to d_min=4 for the
    /// naive monomial-degree-based construction.
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
        Self::build_dynamic_drm_32_21()
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

    /// Returns whether all codewords have even Hamming weight.
    ///
    /// Used by ORBGRAND's even-code optimization to skip half the
    /// noise pattern search space.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    ///
    /// let code = DrmCode::drm_32_21();
    /// let _is_even = code.is_even();
    /// ```
    pub fn is_even(&self) -> bool {
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

    /// Alias for [`drm_32_21`](Self::drm_32_21) — the dynamic (32, 21, 6) code.
    ///
    /// Retained for explicitness when emphasizing the dynamic construction.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = DrmCode::drm_32_21_dynamic();
    /// assert_eq!(code.n(), 32);
    /// assert_eq!(code.k(), 21);
    ///
    /// let msg = BitVec::ones(21);
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.len(), 32);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(k * n) for construction using precomputed generator rows.
    pub fn drm_32_21_dynamic() -> Self {
        Self::build_dynamic_drm_32_21()
    }

    /// Builds the (32, 21, 6) code by extending RM(2,5) with 5 extra rows.
    ///
    /// Uses precomputed generator row words from [`DYNAMIC_DRM_32_21_ROWS`].
    fn build_dynamic_drm_32_21() -> Self {
        let n = 32usize;
        let k = 21usize;

        // Build generator matrix from precomputed row words.
        let row_words = Self::dynamic_32_21_rows();
        let mut g = BitMatrix::zeros(k, n);
        for (row, &word) in row_words.iter().enumerate() {
            for col in 0..n {
                if (word >> col) & 1 == 1 {
                    g.set(row, col, true);
                }
            }
        }

        // Put G in systematic form and compute H.
        let (g_sys, h) = Self::systematic_form(g, k, n);

        let inner = LinearBlockCode::new_systematic(g_sys, Some(h));
        Self { inner }
    }

    /// Returns the 21 precomputed generator row words for the (32, 21, 6) code.
    ///
    /// The first 16 rows are RM(2,5) rows from the polar transform G_32
    /// (all indices with popcount >= 3, each having weight >= 8). The last
    /// 5 rows are random linear combinations of G_32 rows, greedily chosen
    /// to maintain d_min >= 6.
    ///
    /// Each u32 represents a 32-bit codeword row where bit j is the value
    /// at position j.
    ///
    /// Generated with seed=3 (see `test_find_dynamic_seed` and
    /// `test_drm_dynamic_rows_match_seed`).
    fn dynamic_32_21_rows() -> &'static [u32; 21] {
        &DYNAMIC_DRM_32_21_ROWS
    }

    /// Searches for a seed that produces a (32, 21) extension of RM(2,5)
    /// with d_min >= `target_dmin`.
    ///
    /// Computes the 32 rows of the polar transform G_32 as u32 bitmasks.
    ///
    /// Row i of G_32 has bit j set iff (i AND j) == j (the standard polar
    /// transform / Kronecker power of [[1,0],[1,1]]).
    #[cfg(test)]
    fn polar_transform_rows_32() -> Vec<u32> {
        (0..32)
            .map(|i| {
                let mut row = 0u32;
                for j in 0..32 {
                    if (i & j) == j {
                        row |= 1u32 << j;
                    }
                }
                row
            })
            .collect()
    }

    /// Uses a greedy approach: for each seed, adds extension rows one at a
    /// time, accepting each row only if the new coset (row XOR all existing
    /// codewords) has minimum weight >= `target_dmin`. This avoids a full
    /// d_min recomputation at each step.
    #[cfg(test)]
    fn find_seed_for_dmin(target_dmin: usize, max_seeds: u64) -> Option<u64> {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let n = 32usize;
        let g_n = Self::polar_transform_rows_32();

        // RM(2,5) base rows (popcount >= 3).
        let base_rows: Vec<u32> = (0..n)
            .filter(|&i| (i as u32).count_ones() >= 3)
            .map(|i| g_n[i])
            .collect();
        assert_eq!(base_rows.len(), 16);

        for seed in 0..max_seeds {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut rows = base_rows.clone();
            let mut success = true;

            // Enumerate all codewords of the current code (for coset check).
            // Start with RM(2,5) codewords.
            let mut codewords = Self::enumerate_codewords(&rows);

            // Greedily add 5 rows, each maintaining d_min >= target.
            for _ext in 0..5 {
                let mut found_row = false;
                for _ in 0..10000 {
                    let mut candidate = 0u32;
                    for &g_row in &g_n {
                        if rng.gen_bool(0.5) {
                            candidate ^= g_row;
                        }
                    }
                    if candidate == 0 || candidate.count_ones() < target_dmin as u32 {
                        continue;
                    }

                    // Check the new coset: candidate XOR each existing codeword.
                    let coset_ok = codewords
                        .iter()
                        .all(|&c| (candidate ^ c).count_ones() >= target_dmin as u32);

                    if coset_ok {
                        // Extend codeword list with the new coset.
                        let new_codewords: Vec<u32> =
                            codewords.iter().map(|&c| candidate ^ c).collect();
                        codewords.extend_from_slice(&new_codewords);
                        rows.push(candidate);
                        found_row = true;
                        break;
                    }
                }
                if !found_row {
                    success = false;
                    break;
                }
            }

            if success && rows.len() == 21 {
                return Some(seed);
            }
        }
        None
    }

    /// Enumerates all codewords of a code given by its generator row words.
    #[cfg(test)]
    fn enumerate_codewords(rows: &[u32]) -> Vec<u32> {
        let k = rows.len();
        let total = 1u64 << k;
        let mut codewords = Vec::with_capacity(total as usize);
        let mut cw: u32 = 0;
        codewords.push(cw);
        for msg in 1..total {
            let changed_bit = msg.trailing_zeros() as usize;
            cw ^= rows[changed_bit];
            codewords.push(cw);
        }
        codewords
    }

    /// Computes the exact minimum distance by enumerating all 2^k codewords.
    ///
    /// Uses a Gray code enumeration to update the codeword incrementally
    /// (one row XOR per step), achieving O(2^k) XOR operations total.
    ///
    /// # Complexity
    ///
    /// O(2^k) — only practical for small k (k <= ~22).
    #[cfg(test)]
    fn compute_dmin_exhaustive(code: &DrmCode) -> usize {
        let k = code.k();
        let n = code.n();
        let g = code.inner.generator();

        // Precompute each row of G as a u32 word (n <= 32).
        assert!(n <= 32, "compute_dmin_exhaustive only supports n <= 32");
        let row_words: Vec<u32> = (0..k)
            .map(|row| {
                let mut w = 0u32;
                for col in 0..n {
                    if g.get(row, col) {
                        w |= 1u32 << col;
                    }
                }
                w
            })
            .collect();

        let total = 1u64 << k;
        let mut dmin = n + 1;

        // Gray code enumeration: codeword updates by XORing one row per step.
        let mut cw: u32 = 0;
        for msg in 1..total {
            // The bit that changes in Gray code step msg is the position of
            // the lowest set bit of msg.
            let changed_bit = msg.trailing_zeros() as usize;
            cw ^= row_words[changed_bit];
            let w = cw.count_ones() as usize;
            if w < dmin {
                dmin = w;
                if dmin <= 1 {
                    return dmin;
                }
            }
        }
        dmin
    }

    // compute_dmin_from_rows removed — use compute_dmin_exhaustive instead.
}

/// Precomputed generator row words for the (32, 21, 6) dynamic dRM code.
///
/// Found with seed=3 by greedy search (see `test_find_dynamic_seed`).
///
/// Rows 0-15: RM(2,5) rows from G_32 (popcount >= 3 indices).
/// Rows 16-20: greedy RM(2,5)-extensions found with seed 3.
///
/// Each u32 encodes a 32-bit row in little-endian bit order (bit j = column j).
const DYNAMIC_DRM_32_21_ROWS: [u32; 21] = [
    // RM(2,5) base rows (popcount >= 3 indices of G_32)
    0x0000_00FF, // row  0, weight  8, G_32[7]
    0x0000_0F0F, // row  1, weight  8, G_32[11]
    0x0000_3333, // row  2, weight  8, G_32[13]
    0x0000_5555, // row  3, weight  8, G_32[14]
    0x0000_FFFF, // row  4, weight 16, G_32[15]
    0x000F_000F, // row  5, weight  8, G_32[19]
    0x0033_0033, // row  6, weight  8, G_32[21]
    0x0055_0055, // row  7, weight  8, G_32[22]
    0x00FF_00FF, // row  8, weight 16, G_32[23]
    0x0303_0303, // row  9, weight  8, G_32[25]
    0x0505_0505, // row 10, weight  8, G_32[26]
    0x0F0F_0F0F, // row 11, weight 16, G_32[27]
    0x1111_1111, // row 12, weight  8, G_32[28]
    0x3333_3333, // row 13, weight 16, G_32[29]
    0x5555_5555, // row 14, weight 16, G_32[30]
    0xFFFF_FFFF, // row 15, weight 32, G_32[31]
    // Extension rows (seed=3, greedy coset verification)
    0x5A18_6E32, // row 16, weight 14
    0x900C_331B, // row 17, weight 12
    0x7A65_81F4, // row 18, weight 16
    0xC521_DCE0, // row 19, weight 14
    0xC115_7516, // row 20, weight 14
];

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
        // d_min >= 4 proven above. The dynamic dRM(32,21) has d_min=6,
        // so we verify no weight-4 or weight-5 codewords exist among
        // single-row generator codewords.
        use crate::traits::BlockEncoder;
        let k = code.k();
        for bit in 0..k {
            let mut msg = BitVec::zeros(k);
            msg.set(bit, true);
            let cw = code.encode(&msg);
            assert!(
                cw.count_ones() >= 6,
                "generator row {bit} has weight {} < 6",
                cw.count_ones()
            );
        }
    }

    #[test]
    #[ignore] // ~2s: searches for valid seed (already hardcoded)
    fn test_find_dynamic_seed() {
        // Search for a seed where 5 greedy extension rows added to RM(2,5)
        // produce a (32, 21) code with d_min >= 6.
        //
        // The Hamming bound limits d_min for (32, 21):
        //   V(32, 3) = 5489 > 2^11 = 2048, so d_min = 8 is impossible.
        //   V(32, 2) = 529 <= 2048, so d_min = 6 is feasible.
        let result = DrmCode::find_seed_for_dmin(6, 1000);
        assert!(
            result.is_some(),
            "could not find seed with d_min >= 6 in 1000 attempts"
        );
        let seed = result.unwrap();
        eprintln!("Found seed with d_min >= 6: {}", seed);
    }

    // ---- Dynamic dRM(32,21) tests ----

    #[test]
    fn test_drm_dynamic_rows_match_seed() {
        // Verify that the hardcoded rows in DYNAMIC_DRM_32_21_ROWS match
        // what greedy search with seed=3 produces.
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let target_dmin = 6usize;
        let seed = 3u64;

        let g_n = DrmCode::polar_transform_rows_32();

        let mut rows: Vec<u32> = Vec::with_capacity(21);
        for (i, &g_row) in g_n.iter().enumerate() {
            if (i as u32).count_ones() >= 3 {
                rows.push(g_row);
            }
        }
        assert_eq!(rows.len(), 16);

        let mut codewords = DrmCode::enumerate_codewords(&rows);
        let mut rng = StdRng::seed_from_u64(seed);
        for _ in 0..5 {
            loop {
                let mut candidate = 0u32;
                for &g_row in &g_n {
                    if rng.gen_bool(0.5) {
                        candidate ^= g_row;
                    }
                }
                if candidate == 0 || candidate.count_ones() < target_dmin as u32 {
                    continue;
                }
                let coset_ok = codewords
                    .iter()
                    .all(|&c| (candidate ^ c).count_ones() >= target_dmin as u32);
                if coset_ok {
                    let new_cw: Vec<u32> = codewords.iter().map(|&c| candidate ^ c).collect();
                    codewords.extend_from_slice(&new_cw);
                    rows.push(candidate);
                    break;
                }
            }
        }

        let hardcoded = DrmCode::dynamic_32_21_rows();
        assert_eq!(rows.len(), hardcoded.len());
        for (i, (&computed, &stored)) in rows.iter().zip(hardcoded.iter()).enumerate() {
            assert_eq!(
                computed, stored,
                "row {} mismatch: computed 0x{:08X} vs stored 0x{:08X}",
                i, computed, stored
            );
        }
    }

    #[test]
    fn test_drm_dynamic_parameters() {
        let code = DrmCode::drm_32_21_dynamic();
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 21);
    }

    #[test]
    fn test_drm_dynamic_orthogonality() {
        let code = DrmCode::drm_32_21_dynamic();
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
    fn test_drm_dynamic_dmin_at_least_6() {
        // Exhaustively verify d_min >= 6 by enumerating all 2^21 codewords.
        let code = DrmCode::drm_32_21_dynamic();
        let dmin = DrmCode::compute_dmin_exhaustive(&code);
        assert!(
            dmin >= 6,
            "dynamic dRM(32,21) d_min must be >= 6, got {}",
            dmin
        );
        eprintln!("dynamic dRM(32,21) d_min = {}", dmin);
    }

    #[test]
    fn test_drm_dynamic_encode_decode_roundtrip() {
        use crate::bcjr::BcjrDecoder;
        use crate::llr::Llr;
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let code = DrmCode::drm_32_21_dynamic();
        let decoder = BcjrDecoder::new(code.parity_check());
        let mut rng = StdRng::seed_from_u64(42);

        for trial in 0..100 {
            let mut msg = BitVec::new();
            for _ in 0..21 {
                msg.push_bit(rng.gen_bool(0.5));
            }
            let cw = code.encode(&msg);
            assert_eq!(cw.len(), 32);

            // Verify syndrome is zero
            let syn = code.inner().syndrome(&cw).unwrap();
            assert_eq!(syn.count_ones(), 0, "trial {trial}: nonzero syndrome");

            // BCJR decode at high SNR and verify message recovery
            let llrs: Vec<Llr> = (0..32)
                .map(|j| {
                    if cw.get(j) {
                        Llr::new(-10.0)
                    } else {
                        Llr::new(10.0)
                    }
                })
                .collect();
            let result = decoder.decode_siso(&llrs);
            for j in 0..32 {
                let hard = result.app_llrs[j].value() < 0.0;
                assert_eq!(hard, cw.get(j), "trial {trial}: BCJR mismatch at bit {j}");
            }
        }
    }

    #[test]
    fn test_drm_dynamic_is_even() {
        let code = DrmCode::drm_32_21_dynamic();
        // The code is an extension of RM(2,5) where all rows have even
        // weight (weight is always a power of 2). Extension rows are also
        // even-weight since they are XORs of G_32 rows (which all have
        // weights that are powers of 2). So the code is even.
        assert!(
            code.is_even(),
            "dynamic dRM(32,21) should be an even-weight code"
        );
    }

    #[test]
    fn test_drm_dynamic_bcjr_noiseless() {
        use crate::bcjr::BcjrDecoder;
        use crate::llr::Llr;

        let code = DrmCode::drm_32_21_dynamic();
        let decoder = BcjrDecoder::new(code.parity_check());

        // Encode the all-ones message.
        let msg = BitVec::ones(21);
        let cw = code.encode(&msg);

        // High-SNR LLR: +10.0 for 0, -10.0 for 1.
        let llrs: Vec<Llr> = (0..32)
            .map(|j| {
                if cw.get(j) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let result = decoder.decode_siso(&llrs);
        // Check hard decisions match the original codeword.
        for j in 0..32 {
            let hard = result.app_llrs[j].value() < 0.0;
            assert_eq!(hard, cw.get(j), "BCJR hard decision mismatch at bit {}", j);
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
