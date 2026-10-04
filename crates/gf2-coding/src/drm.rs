//! Reed-Muller subcodes used as component codes with GRAND-family decoders,
//! after the dRM ensemble of `@/citation/CoskunPfister2022`. Generators are
//! held in systematic form `G = [I_k | P]` with `H = [P^T | I_r]`, in a
//! coordinate order permuted so that the pivot columns come first.

use crate::linear::LinearBlockCode;
use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::{BitMatrix, BitVec};
use std::sync::OnceLock;

static DRM_32_21_CACHE: OnceLock<LinearBlockCode> = OnceLock::new();

/// A decreasing Reed-Muller code, held as a systematic
/// [`LinearBlockCode`].
#[derive(Debug, Clone)]
pub struct DrmCode {
    inner: LinearBlockCode,
}

impl DrmCode {
    /// Constructs the code spanned by the first `k` monomials over `m`
    /// variables, ordered by degree then lexicographically, evaluated at the
    /// 2^m points of GF(2)^m.
    ///
    /// # Panics
    ///
    /// Panics if `k` exceeds 2^m or if `m` is 0.
    ///
    /// # Complexity
    ///
    /// O(k * 2^m) for monomial evaluation, plus O(k^2 * n) for Gaussian
    /// elimination to produce the systematic generator matrix.
    pub fn new(m: usize, k: usize) -> Self {
        assert!(m > 0, "m must be positive");
        let n = 1usize << m;
        assert!(k <= n, "k must not exceed 2^m = {}", n);

        let monomials = Self::enumerate_monomials(m, k);
        assert_eq!(monomials.len(), k);

        let mut g = BitMatrix::zeros(k, n);
        for (row, mono) in monomials.iter().enumerate() {
            for point in 0..n {
                let val = Self::evaluate_monomial(mono, point, m);
                if val {
                    g.set(row, point, true);
                }
            }
        }

        let (g_sys, h) = Self::systematic_form(g, k, n);

        let inner = LinearBlockCode::new_systematic(g_sys, Some(h));
        Self { inner }
    }

    /// Creates a (2^m, k) code by extending RM(r,m) with greedy
    /// d\_min-maximizing rows from the polar transform.
    ///
    /// The base is the largest RM(r,m) with at most `k` rows. Each extension
    /// row is a random XOR combination of polar-transform rows, drawn from an
    /// RNG seeded by `(m, k)` and accepted only if the extended code keeps
    /// the target d\_min.
    ///
    /// # Panics
    ///
    /// Panics if `m` is 0, `k` is 0, `k > 2^m`, `m > 5` (n must fit
    /// in u32), or if the greedy search fails to find enough extension
    /// rows with the required d\_min.
    ///
    /// # Complexity
    ///
    /// O(2^k) per candidate extension row, for the coset weight check.
    pub fn extended_rm(m: usize, k: usize) -> Self {
        assert!(m > 0, "m must be positive");
        let n = 1usize << m;
        assert!(k > 0 && k <= n, "k must be in 1..={}", n);
        assert!(m <= 5, "extended_rm requires m <= 5 (n fits in u32)");

        let inner = Self::build_extended_rm(m, k);
        Self { inner }
    }

    /// The (32, 21, 6) code of [`extended_rm(5, 21)`](Self::extended_rm),
    /// built once per process and cloned from the cache.
    ///
    /// d\_min = 6 is the maximum for a binary linear (32, 21) code by the
    /// Hamming sphere-packing bound.
    pub fn drm_32_21() -> Self {
        let inner = DRM_32_21_CACHE.get_or_init(|| Self::build_extended_rm(5, 21));
        Self {
            inner: inner.clone(),
        }
    }

    /// Alias for [`drm_32_21`](Self::drm_32_21).
    pub fn drm_32_21_dynamic() -> Self {
        Self::drm_32_21()
    }

    /// Returns the codeword length.
    pub fn n(&self) -> usize {
        self.inner.n()
    }

    /// Returns the message length.
    pub fn k(&self) -> usize {
        self.inner.k()
    }

    /// Returns the parity-check matrix H.
    pub fn parity_check(&self) -> &BitMatrix {
        self.inner
            .parity_check()
            .expect("dRM code always has H matrix")
    }

    /// Returns whether all codewords have even Hamming weight.
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
    pub fn inner(&self) -> &LinearBlockCode {
        &self.inner
    }

    /// Rows of the polar transform G\_N, N = 2^m, as bit words: row i has
    /// bit j set iff (i AND j) == j (the Kronecker power of [[1,0],[1,1]]).
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

    /// Selects the largest RM(r,m) with at most `k_max` rows: the polar
    /// transform rows whose index has popcount >= m-r.
    ///
    /// Returns `(base_rows, popcount_threshold, base_dmin)` with
    /// `popcount_threshold = m-r` and `base_dmin = 2^(m-r)`.
    fn select_rm_base(g_n: &[u32], m: usize, k_max: usize) -> (Vec<u32>, u32, usize) {
        let n = g_n.len();

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

    /// Construction behind [`Self::extended_rm`].
    fn build_extended_rm(m: usize, k: usize) -> LinearBlockCode {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let n = 1usize << m;
        let g_n = Self::polar_transform(m);

        let (mut rows, _threshold, base_dmin) = Self::select_rm_base(&g_n, m, k);
        let k_base = rows.len();

        if k_base >= k {
            rows.truncate(k);
            return Self::rows_to_code(&rows, n);
        }

        let target_dmin = Self::compute_target_dmin(m, k, base_dmin);

        let seed = Self::deterministic_seed(m, k);

        let k_ext = k - k_base;
        let mut rng = StdRng::seed_from_u64(seed);

        let mut codewords = Self::enumerate_codewords_internal(&rows);

        let max_candidates_per_row = 100_000;
        for _ext in 0..k_ext {
            let mut found_row = false;
            for _ in 0..max_candidates_per_row {
                let mut candidate = 0u32;
                for &g_row in &g_n {
                    if rng.gen_bool(0.5) {
                        candidate ^= g_row;
                    }
                }
                if candidate == 0 || candidate.count_ones() < target_dmin as u32 {
                    continue;
                }

                // Only the new coset `candidate ^ c` can lower d_min.
                let coset_ok = codewords
                    .iter()
                    .all(|&c| (candidate ^ c).count_ones() >= target_dmin as u32);

                if coset_ok {
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

    /// Target d\_min for the greedy extension.
    fn compute_target_dmin(m: usize, k: usize, base_dmin: usize) -> usize {
        let n = 1usize << m;
        match (n, k) {
            (32, 21) => 6,
            (16, 11) => 4,
            _ => {
                let half = base_dmin / 2;
                if half >= 4 {
                    half
                } else {
                    4.min(base_dmin)
                }
            }
        }
    }

    /// Seed of the extension search; seed 3 yields d\_min = 6 for (32, 21).
    fn deterministic_seed(m: usize, k: usize) -> u64 {
        match (1usize << m, k) {
            (32, 21) => 3,
            _ => {
                // The multiplier is 2^64 / φ.
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

    /// All 2^k codewords spanned by `rows`, in Gray-code order of the message.
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

    /// First `k` monomials over `m` variables, by degree then
    /// lexicographically, as bitmasks: bit i is set when x\_i is a factor.
    fn enumerate_monomials(m: usize, k: usize) -> Vec<u32> {
        let mut monomials = Vec::with_capacity(k);

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

    /// Returns all `degree`-element subsets of {0, 1, ..., m-1} as bitmasks,
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
        let mono = *monomial as usize;
        (point & mono) == mono
    }

    /// Row-reduces `g` and moves the pivot columns first, giving [I_k | P] in
    /// a permuted coordinate order, and computes H = [P^T | I_r].
    fn systematic_form(g: BitMatrix, k: usize, n: usize) -> (BitMatrix, BitMatrix) {
        let r = n - k;
        let mut work = g;

        let mut pivot_cols = Vec::with_capacity(k);
        let mut current_row = 0;

        for col in 0..n {
            if current_row >= k {
                break;
            }
            let mut pivot = None;
            for row in current_row..k {
                if work.get(row, col) {
                    pivot = Some(row);
                    break;
                }
            }
            if let Some(pivot_row) = pivot {
                if pivot_row != current_row {
                    work.swap_rows(current_row, pivot_row);
                }
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

        let mut h = BitMatrix::zeros(r, n);
        for i in 0..r {
            for j in 0..k {
                h.set(i, j, g_sys.get(j, k + i));
            }
            h.set(i, k + i, true);
        }

        (g_sys, h)
    }

    /// Exact minimum distance over all 2^k codewords, by Gray-code stepping.
    #[cfg(test)]
    fn compute_dmin_exhaustive(code: &DrmCode) -> usize {
        let k = code.k();
        let n = code.n();
        let g = code.inner.generator();

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
        let code = DrmCode::new(5, 6);
        assert_eq!(code.n(), 32);
        assert_eq!(code.k(), 6);
    }

    #[test]
    fn test_monomial_enumeration() {
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
        for point in 0..8 {
            assert!(DrmCode::evaluate_monomial(&0, point, 3));
        }
    }

    #[test]
    fn test_evaluate_monomial_single_var() {
        for point in 0..8 {
            let expected = (point & 1) == 1;
            assert_eq!(DrmCode::evaluate_monomial(&1, point, 3), expected);
        }
    }

    #[test]
    fn test_evaluate_monomial_product() {
        for point in 0..8 {
            let expected = (point & 0b11) == 0b11;
            assert_eq!(DrmCode::evaluate_monomial(&0b11, point, 3), expected);
        }
    }

    #[test]
    fn test_drm_32_21_minimum_distance_lower_bound() {
        let code = DrmCode::drm_32_21();
        let n = code.n();

        for i in 0..n {
            let mut e = BitVec::zeros(n);
            e.set(i, true);
            let syn = code.inner().syndrome(&e).unwrap();
            assert!(syn.count_ones() > 0, "weight-1 at {} has zero syndrome", i);
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
        // No error of weight <= 3 has zero syndrome, so d_min >= 4.
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
        // RM(2,4) has C(4,0)+C(4,1)+C(4,2) = 11 rows: no extension rows.
        let code = DrmCode::extended_rm(4, 11);
        assert_eq!(code.n(), 16);
        assert_eq!(code.k(), 11);
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
            let syn = code.inner().syndrome(&cw).unwrap();
            assert_eq!(syn.count_ones(), 0, "trial {trial}: nonzero syndrome");
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
        let msg = BitVec::ones(21);
        let cw = code.encode(&msg);
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
            assert_eq!(hard, cw.get(j), "BCJR hard decision mismatch at bit {}", j);
        }
    }

    #[test]
    #[ignore = "slow: constructs two DrmCode::extended_rm(5,21) instances for determinism check"]
    fn test_extended_rm_deterministic() {
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
