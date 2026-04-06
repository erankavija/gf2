//! 5G NR LDPC code construction from 3GPP TS 38.212.
//!
//! This module provides factory methods for creating 5G NR standard LDPC codes
//! as defined in 3GPP TS 38.212 Section 5.3.2.
//!
//! 5G NR uses two base graphs:
//! - **BG1**: 46x68 base matrix (K_b = 22, higher code rates, larger blocks)
//! - **BG2**: 42x52 base matrix (K_b = 10, lower code rates, smaller blocks)
//!
//! # Quasi-Cyclic Structure
//!
//! The parity-check matrix H is constructed by replacing each entry in the
//! base matrix with a Z x Z circulant permutation matrix (or a zero matrix
//! for entries equal to -1). The actual shift for entry V is `V mod Z`.
//!
//! # Lifting Sizes
//!
//! Valid lifting sizes Z range from 2 to 384 and belong to one of 8 sets
//! defined in Table 5.3.2-1. Each set contains Z values of the form `a * 2^j`
//! where `a` is the set's base factor (2, 3, 5, 7, 9, 11, 13, or 15).
//!
//! # Rate Matching
//!
//! The mother code (full base graph expanded by Z) can be shortened and
//! punctured to achieve target (n, k) dimensions:
//!
//! 1. **Select Z**: smallest valid Z such that `K_b * Z >= target_k`.
//! 2. **Shortening**: The first `K_b * Z - target_k` systematic bit-columns
//!    of the expanded H are removed. These positions correspond to information
//!    bits forced to zero.
//! 3. **Puncturing**: The first `2 * Z` parity bit-columns are always
//!    punctured (not transmitted). Further parity columns are trimmed from
//!    the end so that exactly `target_n - target_k` parity columns remain.
//!
//! After removal, the resulting H has exactly `target_n` columns. Rows that
//! become all-zero are pruned so that m = n - k holds.
//!
//! # Target Code Construction Parameters
//!
//! The following table documents the exact parameters for each of the 6
//! downstream target codes. "Shortened" and "Punctured" counts are
//! bit-columns removed from the expanded mother code.
//!
//! | Target (n, k) | Rate  | BG  | Z   | Mother (n, k)  | Shortened | Parity punct. | Notes |
//! |---------------|-------|-----|-----|----------------|-----------|---------------|-------|
//! | (256, 121)    | 0.473 | BG2 | 13  | (676, 130)     | 9         | 411           | Z=13 from set 6 |
//! | (256, 49)     | 0.191 | BG2 | 5   | (260, 50)      | 1         | 3             | Z=5 from set 2 |
//! | (625, 225)    | 0.360 | BG2 | 24  | (1248, 240)    | 15        | 608           | Z=24 from set 1 |
//! | (1024, 441)   | 0.431 | BG2 | 48  | (2496, 480)    | 39        | 1433          | Z=48 from set 1 |
//! | (1024, 640)   | 0.625 | BG1 | 30  | (2040, 660)    | 20        | 996           | Z=30 from set 7; BG1 chosen for high rate |
//! | (4096, 3249)  | 0.793 | BG1 | 160 | (10880, 3520)  | 271       | 6513          | Z=160 from set 2 |
//!
//! **Ambiguities**: For (1024, 640) either BG1 or BG2 could work. BG1 is
//! preferred because TS 38.212 recommends BG1 for rates above 0.25 when the
//! information block size permits. For BG2, K_b=10 gives Z=64 and a 3328-column
//! mother code, which also works but is less standard at this rate.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
//!
//! // Create a 5G NR LDPC code with BG2 and lifting factor Z=52
//! let qc = QuasiCyclicLdpc::nr_5g(2, 52);
//! let code = LdpcCode::from_quasi_cyclic(&qc);
//!
//! // BG2: 42 rows x 52 cols, expanded by Z=52
//! assert_eq!(code.m(), 42 * 52);
//! assert_eq!(code.n(), 52 * 52);
//! ```
//!
//! # References
//!
//! 3GPP TS 38.212 V15.0.0 (2017-12): Multiplexing and channel coding

pub(crate) mod bg1;
pub(crate) mod bg2;
pub mod lifting;

pub use lifting::{all_lifting_sizes, is_valid_lifting_size, lifting_set_index};

use super::{LdpcCode, QuasiCyclicLdpc};

impl QuasiCyclicLdpc {
    /// Creates a 5G NR LDPC code from a base graph and lifting factor.
    ///
    /// Constructs the full quasi-cyclic parity-check matrix by expanding
    /// the specified base graph with the given lifting size Z. Each entry V
    /// in the base matrix becomes a Z x Z circulant permutation matrix with
    /// shift `V mod Z`, or a zero matrix if V = -1.
    ///
    /// # Arguments
    ///
    /// * `base_graph` - Base graph number: 1 (BG1, 46x68, K_b=22) or 2 (BG2, 42x52, K_b=10)
    /// * `lifting_factor` - Expansion factor Z. Must be a valid 5G NR lifting size
    ///   from 3GPP TS 38.212 Table 5.3.2-1.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `base_graph` is not 1 or 2
    /// - `lifting_factor` is not a valid 5G NR lifting size
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// // BG2 with Z=52 (from lifting set i_LS=6)
    /// let qc = QuasiCyclicLdpc::nr_5g(2, 52);
    /// assert_eq!(qc.base_rows(), 42);
    /// assert_eq!(qc.base_cols(), 52);
    /// assert_eq!(qc.expansion_factor(), 52);
    ///
    /// // BG1 with Z=384 (largest lifting size)
    /// let qc = QuasiCyclicLdpc::nr_5g(1, 384);
    /// assert_eq!(qc.base_rows(), 46);
    /// assert_eq!(qc.base_cols(), 68);
    /// assert_eq!(qc.expansion_factor(), 384);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(mb * nb) where mb x nb is the base matrix size.
    pub fn nr_5g(base_graph: u8, lifting_factor: usize) -> Self {
        assert!(
            base_graph == 1 || base_graph == 2,
            "base_graph must be 1 or 2, got {base_graph}"
        );
        assert!(
            is_valid_lifting_size(lifting_factor as u16),
            "lifting_factor {lifting_factor} is not a valid 5G NR lifting size"
        );

        let base_matrix = match base_graph {
            1 => bg1::bg1_base_matrix(lifting_factor),
            2 => bg2::bg2_base_matrix(lifting_factor),
            _ => unreachable!(),
        };

        Self::new(base_matrix, lifting_factor)
    }

    /// Creates a rate-matched 5G NR LDPC code with exact target dimensions.
    ///
    /// Builds the full mother code from the base graph expanded by Z, then
    /// applies shortening (removal of systematic bit-columns) and puncturing
    /// (removal of parity bit-columns) at the expanded-matrix level to produce
    /// an LDPC code with exactly the target (n, k) dimensions.
    ///
    /// # Rate Matching Algorithm
    ///
    /// 1. **Select Z**: The smallest valid lifting size Z such that `K_b * Z >= target_k`.
    /// 2. **Shortening**: Remove the first `K_b * Z - target_k` systematic columns
    ///    from the expanded H. These bits are set to known zero.
    /// 3. **Puncturing**: The first `2 * Z` parity columns are always punctured.
    ///    Additional parity columns are removed from the end until exactly
    ///    `target_n - target_k` parity columns remain.
    /// 4. **Row pruning**: Any all-zero rows (checks involving only removed columns)
    ///    are dropped so that m equals the number of non-trivial constraints.
    ///
    /// # Arguments
    ///
    /// * `base_graph` - Base graph number: 1 or 2
    /// * `target_n` - Target codeword length (must satisfy `target_n > target_k`)
    /// * `target_k` - Target message length
    ///
    /// # Returns
    ///
    /// A tuple of `(LdpcCode, NrRateMatchParams)` containing the rate-matched
    /// LDPC code with exact target dimensions, and the construction parameters.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `base_graph` is not 1 or 2
    /// - `target_n <= target_k`
    /// - No valid lifting size exists for the given k
    /// - The mother code has insufficient parity bits for the target n
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(code.n(), 256);
    /// assert_eq!(code.k(), 121);
    /// assert_eq!(params.target_n, 256);
    /// assert_eq!(params.target_k, 121);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(mb * nb * Z) for expanding and filtering the parity-check matrix.
    pub fn nr_5g_rate_matched(
        base_graph: u8,
        target_n: usize,
        target_k: usize,
    ) -> (LdpcCode, NrRateMatchParams) {
        assert!(
            base_graph == 1 || base_graph == 2,
            "base_graph must be 1 or 2, got {base_graph}"
        );
        assert!(
            target_n > target_k,
            "target_n ({target_n}) must be greater than target_k ({target_k})"
        );

        let kb = match base_graph {
            1 => bg1::BG1_KB,
            2 => bg2::BG2_KB,
            _ => unreachable!(),
        };

        let nb = match base_graph {
            1 => bg1::BG1_COLS,
            2 => bg2::BG2_COLS,
            _ => unreachable!(),
        };

        let mb = match base_graph {
            1 => bg1::BG1_ROWS,
            2 => bg2::BG2_ROWS,
            _ => unreachable!(),
        };

        // Step 1: Find the smallest valid Z such that K_b * Z >= target_k
        let all_z = all_lifting_sizes();
        let z = all_z
            .iter()
            .copied()
            .find(|&z| (kb as u16 * z) as usize >= target_k)
            .unwrap_or_else(|| {
                panic!(
                    "No valid lifting size for BG{} with k={}: max possible k = {} * {} = {}",
                    base_graph,
                    target_k,
                    kb,
                    all_z.last().unwrap(),
                    kb as u16 * all_z.last().unwrap()
                )
            });
        let z = z as usize;

        // Mother code dimensions
        let full_k = kb * z;
        let full_n = nb * z;
        let full_m = mb * z;

        // Step 2: Compute shortening count
        let num_shortened = full_k - target_k;

        // Step 3: Compute puncturing
        // Parity bits in the mother code occupy expanded columns [full_k .. full_n)
        let total_parity = full_n - full_k; // = mb * z
        let target_parity = target_n - target_k;

        assert!(
            target_parity <= total_parity,
            "Target requires {} parity bits but mother code only has {} \
             (BG{} Z={}, mb={})",
            target_parity,
            total_parity,
            base_graph,
            z,
            mb
        );

        // Puncture from the end of the parity section so the earliest
        // (highest-weight) parity columns are preserved.
        let num_parity_punctured = total_parity - target_parity;

        // Build the set of retained expanded-column indices.
        // Systematic columns: skip the first `num_shortened`, keep the rest.
        // Parity columns: keep the first `target_parity`, skip the rest.
        let mut retained_cols: Vec<usize> = Vec::with_capacity(target_n);

        // Retained systematic columns: [num_shortened .. full_k)
        for c in num_shortened..full_k {
            retained_cols.push(c);
        }

        // Retained parity columns: [full_k .. full_k + target_parity)
        let parity_start = full_k;
        for c in parity_start..(parity_start + target_parity) {
            retained_cols.push(c);
        }

        assert_eq!(
            retained_cols.len(),
            target_n,
            "Retained column count ({}) must equal target_n ({})",
            retained_cols.len(),
            target_n
        );

        // Build a reverse map: old column -> new column index (or None if removed)
        let mut col_map = vec![None::<usize>; full_n];
        for (new_idx, &old_idx) in retained_cols.iter().enumerate() {
            col_map[old_idx] = Some(new_idx);
        }

        // Step 4: Expand the mother code QC structure and filter edges
        let qc = Self::nr_5g(base_graph, z);
        let mother_edges = qc.to_edges();

        // Filter edges to only those involving retained columns, remap column indices
        let mut filtered_edges: Vec<(usize, usize)> = Vec::new();
        for &(row, col) in &mother_edges {
            if let Some(new_col) = col_map[col] {
                filtered_edges.push((row, new_col));
            }
        }

        // Step 5: Select exactly m_target = target_n - target_k rows.
        //
        // After column removal, the expanded H still has full_m rows but many
        // are linearly dependent. We select the first m_target rows (in
        // expanded-row order) that still have at least one edge. This keeps
        // the high-connectivity core rows first, then extension rows in order.
        let m_target = target_n - target_k;

        let mut row_has_edge = vec![false; full_m];
        for &(row, _) in &filtered_edges {
            row_has_edge[row] = true;
        }

        // Collect the first m_target active rows
        let mut row_map = vec![None::<usize>; full_m];
        let mut new_m = 0;
        for (old_row, &has_edge) in row_has_edge.iter().enumerate() {
            if new_m >= m_target {
                break;
            }
            if has_edge {
                row_map[old_row] = Some(new_m);
                new_m += 1;
            }
        }

        assert_eq!(
            new_m, m_target,
            "Could only find {} active rows, need {} (target_n={}, target_k={})",
            new_m, m_target, target_n, target_k
        );

        // Remap row indices, keeping only edges in selected rows
        let final_edges: Vec<(usize, usize)> = filtered_edges
            .iter()
            .filter_map(|&(row, col)| row_map[row].map(|new_row| (new_row, col)))
            .collect();

        let code = LdpcCode::from_edges(m_target, target_n, &final_edges);

        // Verify dimensions
        assert_eq!(
            code.n(),
            target_n,
            "Constructed code n={} does not match target_n={}",
            code.n(),
            target_n
        );
        assert_eq!(
            code.k(),
            target_k,
            "Constructed code k={} does not match target_k={} (n={}, m={})",
            code.k(),
            target_k,
            code.n(),
            m_target
        );

        let params = NrRateMatchParams {
            base_graph,
            lifting_factor: z,
            target_n,
            target_k,
            full_k,
            full_n,
            num_shortened,
            num_parity_punctured,
            kb,
            active_systematic_bits: target_k,
            transmitted_parity_bits: target_parity,
        };

        (code, params)
    }
}

/// Parameters describing the rate matching applied to a 5G NR LDPC code.
///
/// Documents the exact construction choices (Z, shortening count, puncturing
/// count) so that encoders and decoders can correctly interpret the code.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::QuasiCyclicLdpc;
///
/// let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
/// assert_eq!(params.base_graph, 2);
/// assert_eq!(params.lifting_factor, 13);
/// assert_eq!(params.num_shortened, 9);
/// assert_eq!(params.target_n, 256);
/// assert_eq!(params.target_k, 121);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NrRateMatchParams {
    /// Base graph number (1 or 2).
    pub base_graph: u8,
    /// Lifting factor Z used for expansion.
    pub lifting_factor: usize,
    /// Target codeword length after rate matching.
    pub target_n: usize,
    /// Target message length.
    pub target_k: usize,
    /// Full message length before shortening (K_b * Z).
    pub full_k: usize,
    /// Full codeword length before puncturing (nb * Z).
    pub full_n: usize,
    /// Number of shortened systematic bits (removed from the front).
    pub num_shortened: usize,
    /// Total number of punctured parity bits (always includes 2*Z mandatory).
    pub num_parity_punctured: usize,
    /// K_b: number of systematic base columns in the base graph.
    pub kb: usize,
    /// Number of active (non-shortened) systematic bits (equals target_k).
    pub active_systematic_bits: usize,
    /// Number of transmitted parity bits (equals target_n - target_k).
    pub transmitted_parity_bits: usize,
}

impl NrRateMatchParams {
    /// Returns the effective code rate after rate matching.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let (_, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let rate = params.effective_rate();
    /// assert!((rate - 121.0 / 256.0).abs() < 1e-6);
    /// ```
    pub fn effective_rate(&self) -> f64 {
        self.target_k as f64 / self.target_n as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::LdpcCode;

    // ========================================================================
    // Basic construction tests
    // ========================================================================

    #[test]
    fn test_nr_5g_bg2_z2_dimensions() {
        let qc = QuasiCyclicLdpc::nr_5g(2, 2);
        assert_eq!(qc.base_rows(), 42);
        assert_eq!(qc.base_cols(), 52);
        assert_eq!(qc.expansion_factor(), 2);
        assert_eq!(qc.expanded_rows(), 84);
        assert_eq!(qc.expanded_cols(), 104);
    }

    #[test]
    fn test_nr_5g_bg1_z2_dimensions() {
        let qc = QuasiCyclicLdpc::nr_5g(1, 2);
        assert_eq!(qc.base_rows(), 46);
        assert_eq!(qc.base_cols(), 68);
        assert_eq!(qc.expansion_factor(), 2);
        assert_eq!(qc.expanded_rows(), 92);
        assert_eq!(qc.expanded_cols(), 136);
    }

    #[test]
    fn test_nr_5g_bg2_z384_dimensions() {
        let qc = QuasiCyclicLdpc::nr_5g(2, 384);
        assert_eq!(qc.base_rows(), 42);
        assert_eq!(qc.base_cols(), 52);
        assert_eq!(qc.expansion_factor(), 384);
        assert_eq!(qc.expanded_rows(), 42 * 384);
        assert_eq!(qc.expanded_cols(), 52 * 384);
    }

    #[test]
    fn test_nr_5g_bg1_z384_dimensions() {
        let qc = QuasiCyclicLdpc::nr_5g(1, 384);
        assert_eq!(qc.base_rows(), 46);
        assert_eq!(qc.base_cols(), 68);
        assert_eq!(qc.expansion_factor(), 384);
    }

    #[test]
    #[should_panic(expected = "base_graph must be 1 or 2")]
    fn test_nr_5g_invalid_base_graph() {
        QuasiCyclicLdpc::nr_5g(3, 2);
    }

    #[test]
    #[should_panic(expected = "not a valid 5G NR lifting size")]
    fn test_nr_5g_invalid_lifting_factor() {
        QuasiCyclicLdpc::nr_5g(2, 100);
    }

    // ========================================================================
    // Code construction validity tests
    // ========================================================================

    #[test]
    fn test_nr_5g_bg2_z2_valid_code() {
        let qc = QuasiCyclicLdpc::nr_5g(2, 2);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        assert_eq!(code.n(), 104);
        assert_eq!(code.m(), 84);
        assert_eq!(code.k(), 20);
    }

    #[test]
    fn test_nr_5g_bg2_z52_valid_code() {
        let qc = QuasiCyclicLdpc::nr_5g(2, 52);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        assert_eq!(code.n(), 52 * 52);
        assert_eq!(code.m(), 42 * 52);
        assert_eq!(code.k(), 10 * 52);
    }

    #[test]
    fn test_nr_5g_bg2_zero_codeword_is_valid() {
        let qc = QuasiCyclicLdpc::nr_5g(2, 2);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    #[test]
    fn test_nr_5g_bg1_zero_codeword_is_valid() {
        let qc = QuasiCyclicLdpc::nr_5g(1, 2);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    #[test]
    fn test_nr_5g_bg2_z7_zero_codeword() {
        let qc = QuasiCyclicLdpc::nr_5g(2, 7);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    #[test]
    fn test_nr_5g_bg1_z7_zero_codeword() {
        let qc = QuasiCyclicLdpc::nr_5g(1, 7);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    // ========================================================================
    // Test all valid lifting sizes
    // ========================================================================

    #[test]
    fn test_nr_5g_bg2_all_lifting_sizes() {
        for z in all_lifting_sizes() {
            let z = z as usize;
            let qc = QuasiCyclicLdpc::nr_5g(2, z);
            assert_eq!(qc.expansion_factor(), z, "Z={z}: expansion factor mismatch");
            assert_eq!(qc.base_rows(), 42, "Z={z}: wrong base rows");
            assert_eq!(qc.base_cols(), 52, "Z={z}: wrong base cols");
        }
    }

    #[test]
    fn test_nr_5g_bg1_all_lifting_sizes() {
        for z in all_lifting_sizes() {
            let z = z as usize;
            let qc = QuasiCyclicLdpc::nr_5g(1, z);
            assert_eq!(qc.expansion_factor(), z, "Z={z}: expansion factor mismatch");
            assert_eq!(qc.base_rows(), 46, "Z={z}: wrong base rows");
            assert_eq!(qc.base_cols(), 68, "Z={z}: wrong base cols");
        }
    }

    // ========================================================================
    // Spot-check: verify shift values are within range
    // ========================================================================

    #[test]
    fn test_nr_5g_bg2_shifts_in_range() {
        for z in [2u16, 3, 5, 7, 52, 384] {
            let matrix = bg2::bg2_base_matrix(z as usize);
            for (r, row) in matrix.iter().enumerate() {
                for (c, &val) in row.iter().enumerate() {
                    if val >= 0 {
                        assert!(
                            (val as usize) < z as usize,
                            "BG2 Z={z} shift at ({r},{c}) = {val} >= Z"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_nr_5g_bg1_shifts_in_range() {
        for z in [2u16, 3, 7, 384] {
            let matrix = bg1::bg1_base_matrix(z as usize);
            for (r, row) in matrix.iter().enumerate() {
                for (c, &val) in row.iter().enumerate() {
                    if val >= 0 {
                        assert!(
                            (val as usize) < z as usize,
                            "BG1 Z={z} shift at ({r},{c}) = {val} >= Z"
                        );
                    }
                }
            }
        }
    }

    // ========================================================================
    // Base matrix structural checks
    // ========================================================================

    #[test]
    fn test_bg2_core_matrix_fully_connected() {
        let matrix = bg2::bg2_base_matrix(384);
        for (r, row) in matrix.iter().enumerate().take(4) {
            let non_neg_count = row.iter().filter(|&&v| v >= 0).count();
            assert!(
                non_neg_count > 5,
                "BG2 core row {r} has only {non_neg_count} non-negative entries"
            );
        }
    }

    #[test]
    fn test_bg2_extension_has_identity_diagonal() {
        let matrix = bg2::bg2_base_matrix(384);
        for (r, row) in matrix.iter().enumerate().take(40).skip(4) {
            let has_identity = row[bg2::BG2_KB..].contains(&0);
            assert!(
                has_identity,
                "BG2 extension row {r} missing identity entry in parity part"
            );
        }
        for (r, row) in matrix.iter().enumerate().take(42).skip(40) {
            let has_identity = row[bg2::BG2_KB..].contains(&0);
            assert!(
                !has_identity,
                "BG2 row {r} unexpectedly has identity entry in parity part"
            );
        }
    }

    #[test]
    fn test_bg1_dimensions() {
        let matrix = bg1::bg1_base_matrix(384);
        assert_eq!(matrix.len(), bg1::BG1_ROWS);
        for row in &matrix {
            assert_eq!(row.len(), bg1::BG1_COLS);
        }
    }

    #[test]
    fn test_bg2_dimensions() {
        let matrix = bg2::bg2_base_matrix(384);
        assert_eq!(matrix.len(), bg2::BG2_ROWS);
        for row in &matrix {
            assert_eq!(row.len(), bg2::BG2_COLS);
        }
    }

    // ========================================================================
    // Rate matching: exact dimension tests for all 6 target codes
    // ========================================================================

    #[test]
    fn test_rate_matched_bg2_256_121_exact_dimensions() {
        let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        assert_eq!(code.n(), 256, "n mismatch");
        assert_eq!(code.k(), 121, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 13);
        assert_eq!(params.full_k, 130); // 10 * 13
        assert_eq!(params.num_shortened, 9); // 130 - 121
        assert_eq!(params.target_n, 256);
        assert_eq!(params.target_k, 121);
    }

    #[test]
    fn test_rate_matched_bg2_256_49_exact_dimensions() {
        let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
        assert_eq!(code.n(), 256, "n mismatch");
        assert_eq!(code.k(), 49, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 5);
        assert_eq!(params.full_k, 50); // 10 * 5
        assert_eq!(params.num_shortened, 1); // 50 - 49
    }

    #[test]
    fn test_rate_matched_bg2_625_225_exact_dimensions() {
        let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 625, 225);
        assert_eq!(code.n(), 625, "n mismatch");
        assert_eq!(code.k(), 225, "k mismatch");
        assert_eq!(params.base_graph, 2);
        // ceil(225/10) = 23, smallest Z >= 23 is 24
        assert_eq!(params.lifting_factor, 24);
        assert_eq!(params.full_k, 240);
        assert_eq!(params.num_shortened, 15);
    }

    #[test]
    fn test_rate_matched_bg2_1024_441_exact_dimensions() {
        let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
        assert_eq!(code.n(), 1024, "n mismatch");
        assert_eq!(code.k(), 441, "k mismatch");
        assert_eq!(params.base_graph, 2);
        // ceil(441/10) = 45, smallest Z >= 45 is 48
        assert_eq!(params.lifting_factor, 48);
        assert_eq!(params.full_k, 480);
        assert_eq!(params.num_shortened, 39);
    }

    #[test]
    fn test_rate_matched_bg1_1024_640_exact_dimensions() {
        let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 640);
        assert_eq!(code.n(), 1024, "n mismatch");
        assert_eq!(code.k(), 640, "k mismatch");
        assert_eq!(params.base_graph, 1);
        // ceil(640/22) = 30, smallest Z >= 30 is 30
        assert_eq!(params.lifting_factor, 30);
        assert_eq!(params.full_k, 660); // 22 * 30
        assert_eq!(params.num_shortened, 20);
    }

    #[test]
    fn test_rate_matched_bg1_4096_3249_exact_dimensions() {
        let (code, params) = QuasiCyclicLdpc::nr_5g_rate_matched(1, 4096, 3249);
        assert_eq!(code.n(), 4096, "n mismatch");
        assert_eq!(code.k(), 3249, "k mismatch");
        assert_eq!(params.base_graph, 1);
        // ceil(3249/22) = 148, smallest Z >= 148 is 160
        assert_eq!(params.lifting_factor, 160);
        assert_eq!(params.full_k, 3520); // 22 * 160
        assert_eq!(params.num_shortened, 271);
    }

    // ========================================================================
    // Rate matching: BP decoding convergence on all-zero codeword
    // ========================================================================

    /// Helper: verify BP decoding converges on the zero codeword for a
    /// rate-matched code. Feeds high-confidence LLRs (+10 for each bit)
    /// and checks that the decoder converges within 50 iterations.
    fn assert_bp_converges(code: &LdpcCode, label: &str) {
        use crate::llr::Llr;
        use crate::traits::IterativeSoftDecoder;

        let mut decoder = crate::ldpc::LdpcDecoder::new(code.clone());
        // All-zero codeword: positive LLR means "likely 0"
        let llrs: Vec<Llr> = vec![Llr::new(10.0); code.n()];
        let result = decoder.decode_iterative(&llrs, 50);
        assert!(
            result.converged,
            "{label}: BP did not converge in 50 iterations"
        );
        assert!(
            result.syndrome_check_passed,
            "{label}: syndrome check failed after convergence"
        );
    }

    #[test]
    fn test_rate_matched_bg2_256_121_bp_converges() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        assert_bp_converges(&code, "BG2 (256,121)");
    }

    #[test]
    fn test_rate_matched_bg2_256_49_bp_converges() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
        assert_bp_converges(&code, "BG2 (256,49)");
    }

    #[test]
    fn test_rate_matched_bg2_625_225_bp_converges() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 625, 225);
        assert_bp_converges(&code, "BG2 (625,225)");
    }

    #[test]
    fn test_rate_matched_bg2_1024_441_bp_converges() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
        assert_bp_converges(&code, "BG2 (1024,441)");
    }

    #[test]
    fn test_rate_matched_bg1_1024_640_bp_converges() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 640);
        assert_bp_converges(&code, "BG1 (1024,640)");
    }

    #[test]
    fn test_rate_matched_bg1_4096_3249_bp_converges() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(1, 4096, 3249);
        assert_bp_converges(&code, "BG1 (4096,3249)");
    }

    // ========================================================================
    // Rate matching: zero codeword validity
    // ========================================================================

    #[test]
    fn test_rate_matched_bg2_256_121_zero_codeword() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    #[test]
    fn test_rate_matched_bg2_256_49_zero_codeword() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    #[test]
    fn test_rate_matched_bg1_1024_640_zero_codeword() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 640);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    #[test]
    fn test_rate_matched_bg1_4096_3249_zero_codeword() {
        let (code, _) = QuasiCyclicLdpc::nr_5g_rate_matched(1, 4096, 3249);
        let zero = gf2_core::BitVec::zeros(code.n());
        assert!(code.is_valid_codeword(&zero));
    }

    // ========================================================================
    // Rate matching: effective rate
    // ========================================================================

    #[test]
    fn test_rate_matched_effective_rate() {
        let (_, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let rate = params.effective_rate();
        assert!(
            (rate - 121.0 / 256.0).abs() < 1e-6,
            "Expected rate ~0.473, got {rate}"
        );
    }

    // ========================================================================
    // Panics
    // ========================================================================

    #[test]
    #[should_panic(expected = "base_graph must be 1 or 2")]
    fn test_rate_matched_invalid_bg() {
        QuasiCyclicLdpc::nr_5g_rate_matched(3, 256, 121);
    }

    #[test]
    #[should_panic(expected = "target_n")]
    fn test_rate_matched_n_le_k() {
        QuasiCyclicLdpc::nr_5g_rate_matched(2, 100, 200);
    }

    // ========================================================================
    // Spot-check specific shift values
    // ========================================================================

    #[test]
    fn test_bg2_specific_shift_values() {
        let matrix = bg2::bg2_base_matrix(384);
        assert_eq!(matrix[0][0], 38);
        assert_eq!(matrix[0][1], 52);
        assert_eq!(matrix[0][9], 103);
        assert_eq!(matrix[1][13], 1);
        assert_eq!(matrix[2][14], 0);
        assert_eq!(matrix[3][15], 0);
        assert_eq!(matrix[4][16], 0);
        assert_eq!(matrix[41][6], 37);
        assert_eq!(matrix[41][7], 80);
    }

    #[test]
    fn test_bg1_specific_shift_values() {
        let matrix = bg1::bg1_base_matrix(384);
        assert_eq!(matrix[0][0], 250);
        assert_eq!(matrix[0][22], 56);
        assert_eq!(matrix[1][23], 5);
        assert_eq!(matrix[2][24], 0);
        assert_eq!(matrix[45][67], 0);
    }

    // ========================================================================
    // Modular reduction test
    // ========================================================================

    #[test]
    fn test_bg2_shifts_mod_z() {
        let matrix = bg2::bg2_base_matrix(2);
        for row in &matrix {
            for &val in row {
                if val >= 0 {
                    assert!(val == 0 || val == 1, "Shift {val} not in {{0,1}} for Z=2");
                }
            }
        }
    }

    #[test]
    fn test_bg1_shifts_mod_z() {
        let matrix = bg1::bg1_base_matrix(2);
        for row in &matrix {
            for &val in row {
                if val >= 0 {
                    assert!(val == 0 || val == 1, "Shift {val} not in {{0,1}} for Z=2");
                }
            }
        }
    }

    // ========================================================================
    // Edge count sanity checks
    // ========================================================================

    #[test]
    fn test_bg2_edge_count_z2() {
        let qc = QuasiCyclicLdpc::nr_5g(2, 2);
        let edges = qc.to_edges();
        let matrix = bg2::bg2_base_matrix(2);
        let non_neg_count: usize = matrix
            .iter()
            .map(|row| row.iter().filter(|&&v| v >= 0).count())
            .sum();
        assert_eq!(edges.len(), non_neg_count * 2);
    }

    // ========================================================================
    // Rows 40-41 (punctured rows) are valid
    // ========================================================================

    #[test]
    fn test_bg2_rows_40_41_have_entries() {
        let matrix = bg2::bg2_base_matrix(384);
        let row40_nneg: usize = matrix[40].iter().filter(|&&v| v >= 0).count();
        let row41_nneg: usize = matrix[41].iter().filter(|&&v| v >= 0).count();
        assert!(row40_nneg >= 2, "Row 40 should have >= 2 non-neg entries");
        assert!(row41_nneg >= 2, "Row 41 should have >= 2 non-neg entries");
    }
}
