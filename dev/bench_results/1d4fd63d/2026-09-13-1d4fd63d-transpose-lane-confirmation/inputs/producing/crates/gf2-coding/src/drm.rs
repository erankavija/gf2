//! Reed-Muller subcodes for GRAND decoding.
//!
//! This module provides Reed-Muller (RM) subcodes suitable as component
//! codes in product code turbo decoders with GRAND-family algorithms.
//!
//! # Constructions
//!
//! Two constructions are available:
//!
//! - [`DrmCode::new`]: generic RM subcode from monomial evaluations
//!   (degree-then-lexicographic order).
//! - [`DrmCode::extended_rm`]: a generalized construction that extends
//!   RM(r,m) with greedy d\_min-maximizing rows from the polar transform.
//! - [`DrmCode::drm_32_21`]: the standard (32, 21, 6) code, computed via
//!   [`extended_rm(5, 21)`](DrmCode::extended_rm) and cached with `OnceLock`.
//!
//! The (32, 21, 6) code achieves d\_min=6, the maximum for any binary
//! linear (32, 21) code by the Hamming sphere-packing bound. The
//! construction extends RM(2,5) with 5 additional rows found by greedy
//! d\_min-maximizing search over random linear combinations of polar
//! transform rows, inspired by the dRM ensemble of Coskun & Pfister
//! (arxiv:2103.16680).
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
use std::sync::OnceLock;

/// Cached (32, 21, 6) code constructed by `extended_rm(5, 21)`.
static DRM_32_21_CACHE: OnceLock<LinearBlockCode> = OnceLock::new();

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

    /// Creates a (2^m, k) code by extending RM(r,m) with greedy
    /// d\_min-maximizing rows from the polar transform.
    ///
    /// The algorithm:
    /// 1. Compute the polar transform G\_N (N = 2^m).
    /// 2. Select RM(r,m) base rows: all G\_N rows whose index has
    ///    popcount >= m-r, where r is the maximum order such that
    ///    the resulting RM code has at most k rows.
    /// 3. Greedily extend by adding random XOR combinations of G\_N
    ///    rows, accepting a candidate only if d\_min of the extended
    ///    code remains above a threshold computed from the base RM code.
    ///
    /// The construction is deterministic: a fixed seed derived from
    /// (m, k) always produces the same code.
    ///
    /// # Arguments
    ///
    /// * `m` - Number of variables (n = 2^m)
    /// * `k` - Target dimension (number of generator rows)
    ///
    /// # Panics
    ///
    /// Panics if `m` is 0, `k` is 0, `k > 2^m`, `m > 5` (n must fit
    /// in u32), or if the greedy search fails to find enough extension
    /// rows with the required d\_min.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::drm::DrmCode;
    ///
    /// // (32, 21) extended RM code with d_min >= 6
    /// let code = DrmCode::extended_rm(5, 21);
    /// assert_eq!(code.n(), 32);
    /// assert_eq!(code.k(), 21);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(2^k) for coset weight verification at each extension step,
    /// with up to O(k\_ext * max\_candidates) extension attempts.
    pub fn extended_rm(m: usize, k: usize) -> Self {
        assert!(m > 0, "m must be positive");
        let n = 1usize << m;
        assert!(k > 0 && k <= n, "k must be in 1..={}", n);
        assert!(m <= 5, "extended_rm requires m <= 5 (n fits in u32)");

        let inner = Self::build_extended_rm(m, k);
        Self { inner }
    }

    /// Creates the (32, 21, 6) code — the standard dRM for GRAND product codes.
    ///
    /// Delegates to [`extended_rm(5, 21)`](Self::extended_rm) and caches
    /// the result with `OnceLock` for efficient repeated access.
    ///
    /// The code achieves d\_min=6, the maximum for any binary linear
    /// (32, 21) code by the Hamming sphere-packing bound.
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
        let inner = DRM_32_21_CACHE.get_or_init(|| Self::build_extended_rm(5, 21));
        Self {
            inner: inner.clone(),
        }
    }

    /// Alias for [`drm_32_21`](Self::drm_32_21) — the dynamic (32, 21, 6) code.
    ///
    /// Retained for backward compatibility and explicitness when
    /// emphasizing the dynamic construction.
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
    pub fn drm_32_21_dynamic() -> Self {
        Self::drm_32_21()
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

    // ---- Polar transform and extended RM construction ----

    /// Computes the N=2^m rows of the polar transform G\_N.
    ///
    /// Row i of G\_N has bit j set iff (i AND j) == j, i.e., the
    /// support of j is a subset of the support of i. This is the
    /// standard Kronecker power of [[1,0],[1,1]].
    ///
    /// # Arguments
    ///
    /// * `m` - Number of variables (N = 2^m rows, each an N-bit word)
    ///
    /// # Returns
    ///
    /// A vector of N `u32` values, each representing a row of G\_N.
    ///
    /// # Complexity
    ///
    /// O(N^2) where N = 2^m.
    fn polar_transform(m: usize) -> Vec<u32> {
        let n = 1usize << m;
        (0..n)
            .map(|i| {
                let mut row = 0u32;
                for j in 0..n {
                    if (i & j) == j {
                        row |= 1u32 << j;
                    }
                }
                row
            })
            .collect()
    }

    /// Selects RM(r,m) base rows from the polar transform by popcount
    /// threshold, returning the maximum r such that the number of
    /// selected rows does not exceed `k_max`.
    ///
    /// RM(r,m) consists of all polar transform rows whose index has
    /// popcount >= m-r. The function finds the largest r (equivalently,
    /// lowest popcount threshold) that keeps the row count <= k\_max.
    ///
    /// # Returns
    ///
    /// `(base_rows, popcount_threshold, target_dmin)` where:
    /// - `base_rows` are the selected polar transform row words
    /// - `popcount_threshold` is the minimum popcount used
    /// - `target_dmin` is the minimum distance of the base RM code (2^(m-r))
    fn select_rm_base(g_n: &[u32], m: usize, k_max: usize) -> (Vec<u32>, u32, usize) {
        let n = g_n.len();

        // Try increasing popcount thresholds (decreasing r) to find the
        // largest RM(r,m) that fits in k_max rows.
        // popcount_threshold = m - r, so lower threshold = higher r = more rows.
        // We want the smallest threshold such that count <= k_max.
        let mut best_threshold = m as u32; // RM(0,m): only the all-ones row
        for threshold in 0..=m as u32 {
            let count = (0..n)
                .filter(|&i| (i as u32).count_ones() >= threshold)
                .count();
            if count <= k_max {
                best_threshold = threshold;
                break;
            }
        }

        let base_rows: Vec<u32> = (0..n)
            .filter(|&i| (i as u32).count_ones() >= best_threshold)
            .map(|i| g_n[i])
            .collect();

        // d_min of RM(r,m) = 2^(m-r) where r = m - threshold
        let r = m as u32 - best_threshold;
        let base_dmin = 1usize << (m as u32 - r);

        (base_rows, best_threshold, base_dmin)
    }

    /// Builds the extended RM code as a `LinearBlockCode`.
    ///
    /// This is the core algorithm: compute polar transform, select RM
    /// base rows, greedily extend to k rows while maintaining d\_min.
    fn build_extended_rm(m: usize, k: usize) -> LinearBlockCode {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let n = 1usize << m;
        let g_n = Self::polar_transform(m);

        let (mut rows, _threshold, base_dmin) = Self::select_rm_base(&g_n, m, k);
        let k_base = rows.len();

        // If base RM already has enough rows, truncate to k.
        if k_base >= k {
            rows.truncate(k);
            return Self::rows_to_code(&rows, n);
        }

        // Target d_min for extension: we aim for d_min >= base_dmin / 2
        // but at least 4, and for the specific (32,21) case we know d_min=6
        // is achievable.
        // For RM(2,5) base (d_min=8), target = max(8/2, 4) = max(4, 4) = 4.
        // But we actually want d_min=6 for (32,21). Use a heuristic:
        // try base_dmin first, then base_dmin-2, etc.
        let target_dmin = Self::compute_target_dmin(m, k, base_dmin);

        // Derive a deterministic seed from (m, k).
        let seed = Self::deterministic_seed(m, k);

        let k_ext = k - k_base;
        let mut rng = StdRng::seed_from_u64(seed);

        // Enumerate all codewords of the current base code.
        let mut codewords = Self::enumerate_codewords_internal(&rows);

        // Greedily add extension rows.
        let max_candidates_per_row = 100_000;
        for _ext in 0..k_ext {
            let mut found_row = false;
            for _ in 0..max_candidates_per_row {
                // Generate a random linear combination of G_N rows.
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
            assert!(
                found_row,
                "extended_rm({}, {}): failed to find extension row {} \
                 with d_min >= {} after {} candidates",
                m, k, _ext, target_dmin, max_candidates_per_row
            );
        }

        Self::rows_to_code(&rows, n)
    }

    /// Computes a target d\_min for the greedy extension.
    ///
    /// For known good parameters, returns the optimal d\_min. Otherwise
    /// uses a heuristic based on the base RM code's d\_min.
    fn compute_target_dmin(m: usize, k: usize, base_dmin: usize) -> usize {
        // Known optimal d_min values for specific (n, k) pairs.
        let n = 1usize << m;
        match (n, k) {
            (32, 21) => 6,
            (16, 11) => 4,
            _ => {
                // Heuristic: half of base d_min, at least 4.
                let half = base_dmin / 2;
                if half >= 4 {
                    half
                } else {
                    4.min(base_dmin)
                }
            }
        }
    }

    /// Derives a deterministic seed from (m, k) parameters.
    ///
    /// For the known (32, 21) case, uses seed=3 which is known to produce
    /// a d\_min=6 code. For other parameters, uses a hash-like combination.
    fn deterministic_seed(m: usize, k: usize) -> u64 {
        match (1usize << m, k) {
            (32, 21) => 3,
            _ => {
                // Simple deterministic hash: m * large_prime + k
                (m as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (k as u64)
            }
        }
    }

    /// Converts a set of generator row words into a `LinearBlockCode`
    /// in systematic form.
    fn rows_to_code(rows: &[u32], n: usize) -> LinearBlockCode {
        let k = rows.len();
        let mut g = BitMatrix::zeros(k, n);
        for (row, &word) in rows.iter().enumerate() {
            for col in 0..n {
                if (word >> col) & 1 == 1 {
                    g.set(row, col, true);
                }
            }
        }

        let (g_sys, h) = Self::systematic_form(g, k, n);
        LinearBlockCode::new_systematic(g_sys, Some(h))
    }

    /// Enumerates all codewords of a code given by its generator row words.
    ///
    /// Uses Gray code enumeration for O(2^k) XOR operations.
    fn enumerate_codewords_internal(rows: &[u32]) -> Vec<u32> {
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

    // ---- Monomial construction helpers ----

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

    // ---- Extended RM tests ----

    #[test]
    fn test_extended_rm_32_21_parameters() {
        let code = DrmCode::extended_rm(5, 21);
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 21);
    }

    #[test]
    fn test_extended_rm_32_21_orthogonality() {
        let code = DrmCode::extended_rm(5, 21);
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
    fn test_extended_rm_16_11() {
        // RM(2,4) has C(4,0)+C(4,1)+C(4,2) = 1+4+6 = 11 rows.
        // So extended_rm(4, 11) should just use RM(2,4) directly
        // with no extension needed.
        let code = DrmCode::extended_rm(4, 11);
        assert_eq!(code.n(), 16);
        assert_eq!(code.k(), 11);

        // Verify G*H^T = 0
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
    fn test_polar_transform_m3() {
        // G_8 should be 8x8 with row i having bit j set iff (i&j)==j.
        let g = DrmCode::polar_transform(3);
        assert_eq!(g.len(), 8);

        // Row 0 (0b000): only j=0 has (0&j)==j, so row = 0b00000001 = 1
        assert_eq!(g[0], 1);
        // Row 7 (0b111): all j have (7&j)==j, so row = 0xFF = 255
        assert_eq!(g[7], 0xFF);
        // Row 3 (0b011): j must be subset of {0,1} -> j in {0,1,2,3}
        assert_eq!(g[3], 0x0F);
    }

    #[test]
    fn test_select_rm_base_m5_k21() {
        let g_n = DrmCode::polar_transform(5);
        let (base_rows, threshold, dmin) = DrmCode::select_rm_base(&g_n, 5, 21);
        // RM(2,5): popcount >= 3 gives 16 rows, d_min = 8
        assert_eq!(base_rows.len(), 16);
        assert_eq!(threshold, 3);
        assert_eq!(dmin, 8);
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
    #[ignore = "slow: 100-trial BCJR decode roundtrip for dRM(32,21)"]
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

    #[test]
    #[ignore = "slow: constructs two DrmCode::extended_rm(5,21) instances for determinism check"]
    fn test_extended_rm_deterministic() {
        // Two calls to extended_rm with the same parameters must produce
        // the same code.
        let code1 = DrmCode::extended_rm(5, 21);
        let code2 = DrmCode::extended_rm(5, 21);
        let g1 = code1.generator_matrix();
        let g2 = code2.generator_matrix();
        for i in 0..g1.rows() {
            for j in 0..g1.cols() {
                assert_eq!(
                    g1.get(i, j),
                    g2.get(i, j),
                    "extended_rm must be deterministic: G[{},{}] differs",
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
