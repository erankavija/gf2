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
//! The mother code (full base graph) can be shortened and punctured to achieve
//! target (n, k) dimensions. Shortening removes systematic columns (setting
//! them to known zeros) and puncturing removes parity columns from transmission.
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

use super::QuasiCyclicLdpc;

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

    /// Creates a 5G NR LDPC code with rate matching applied.
    ///
    /// Starts from the full mother code (BG1 or BG2 expanded with Z) and applies
    /// shortening and puncturing to achieve target dimensions. The returned code
    /// uses only the active rows and columns after rate matching.
    ///
    /// # Rate Matching Algorithm
    ///
    /// 1. **Select Z**: The smallest valid lifting size Z such that `K_b * Z >= k`
    ///    (where K_b is the number of systematic columns in the base graph).
    /// 2. **Shortening**: If `K_b * Z > k`, the first `K_b * Z - k` systematic bits
    ///    are set to zero (shortened). These columns are removed from the code.
    /// 3. **Puncturing**: The first 2*Z parity bits are always punctured (not transmitted).
    ///    Additional parity columns may be punctured to reach the target n.
    ///
    /// # Arguments
    ///
    /// * `base_graph` - Base graph number: 1 or 2
    /// * `target_n` - Target codeword length
    /// * `target_k` - Target message length
    ///
    /// # Returns
    ///
    /// A tuple of `(QuasiCyclicLdpc, NrRateMatchParams)` containing the QC structure
    /// and the rate matching parameters used.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `base_graph` is not 1 or 2
    /// - No valid lifting size exists for the given k
    /// - Target dimensions are incompatible with the base graph
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let (qc, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(params.target_n, 256);
    /// assert_eq!(params.target_k, 121);
    /// assert!(params.lifting_factor > 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(mb * nb) where mb x nb is the base matrix size.
    pub fn nr_5g_rate_matched(
        base_graph: u8,
        target_n: usize,
        target_k: usize,
    ) -> (Self, NrRateMatchParams) {
        assert!(
            base_graph == 1 || base_graph == 2,
            "base_graph must be 1 or 2, got {base_graph}"
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

        // Find the smallest valid Z such that K_b * Z >= target_k
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

        // Compute shortening: number of systematic bits set to zero
        let full_k = kb * z;
        let num_shortened = full_k - target_k;

        // Compute the number of transmitted parity bits needed
        // Total transmitted bits = target_n
        // Transmitted systematic bits = target_k (after shortening)
        // Transmitted parity bits = target_n - target_k
        let num_parity_transmitted = target_n - target_k;

        // Full parity bits available = mb * Z
        // The first 2*Z parity bits are always punctured per 3GPP TS 38.212
        let full_parity = mb * z;
        let available_parity = full_parity.saturating_sub(2 * z);
        let num_parity_punctured = available_parity.saturating_sub(num_parity_transmitted);

        // Build the rate-matched base matrix by:
        // 1. Remove shortened systematic columns
        // 2. Keep all parity columns (puncturing is handled at the encoder/decoder level,
        //    not by removing columns from H)
        // For the QC-LDPC construction, we build the full code and let the
        // encoder/decoder handle shortening and puncturing.
        let base_matrix = match base_graph {
            1 => bg1::bg1_base_matrix(z),
            2 => bg2::bg2_base_matrix(z),
            _ => unreachable!(),
        };

        // For rate matching, we construct the full code. The rate matching parameters
        // describe how to interpret the code for encoding/decoding.
        // Shortened bits are set to zero and not transmitted.
        // Punctured parity bits are not transmitted but still part of the code.

        // Determine how many base graph columns to actually use:
        // Active systematic columns = kb - ceil(num_shortened / z) (fully active columns)
        // We keep all columns in the QC structure and let the rate matching params
        // guide the encoder/decoder.
        let active_sys_cols = if num_shortened == 0 {
            kb
        } else {
            // Number of fully shortened columns (entire Z-block is zero)
            let fully_shortened_cols = num_shortened / z;
            // Remaining partial shortening within the next column
            let _partial_shortening = num_shortened % z;
            kb - fully_shortened_cols
        };

        // Number of parity columns actually needed for transmission + punctured
        let total_parity_cols_needed = (num_parity_transmitted + 2 * z).div_ceil(z);
        let active_parity_cols = total_parity_cols_needed.min(mb);

        // Build a trimmed base matrix with only active columns
        let active_cols = active_sys_cols + active_parity_cols;
        let trimmed_base_matrix: Vec<Vec<i32>> = base_matrix
            .iter()
            .take(active_parity_cols)
            .map(|row| {
                let mut trimmed_row = Vec::with_capacity(active_cols);
                // Systematic columns (skip fully shortened ones from the beginning)
                let skip_sys = kb - active_sys_cols;
                trimmed_row.extend_from_slice(&row[skip_sys..kb]);
                // Parity columns
                trimmed_row.extend_from_slice(&row[kb..(kb + active_parity_cols)]);
                trimmed_row
            })
            .collect();

        let qc = Self::new(trimmed_base_matrix, z);

        let params = NrRateMatchParams {
            base_graph,
            lifting_factor: z,
            target_n,
            target_k,
            full_k,
            full_n: nb * z,
            num_shortened,
            num_parity_punctured,
            kb,
            active_sys_cols,
            active_parity_cols,
        };

        (qc, params)
    }
}

/// Parameters describing the rate matching applied to a 5G NR LDPC code.
///
/// These parameters are needed by the encoder and decoder to correctly
/// handle shortened and punctured bits.
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
    /// Number of shortened systematic bits.
    pub num_shortened: usize,
    /// Number of punctured parity bits.
    pub num_parity_punctured: usize,
    /// K_b: number of systematic base columns.
    pub kb: usize,
    /// Number of active (non-shortened) systematic base columns.
    pub active_sys_cols: usize,
    /// Number of active parity base columns.
    pub active_parity_cols: usize,
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
    /// assert!(rate > 0.0 && rate < 1.0);
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
        // Verify BG2 construction succeeds for all valid Z
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
        // Verify BG1 construction succeeds for all valid Z
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
        // For any Z, all shift values after mod must be in [0, Z)
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
        // First 4 rows of BG2 should have high connectivity to systematic columns
        let matrix = bg2::bg2_base_matrix(384);
        for (r, row) in matrix.iter().enumerate().take(4) {
            let non_neg_count = row.iter().filter(|&&v| v >= 0).count();
            // Core rows should have many non-negative entries (> 5)
            assert!(
                non_neg_count > 5,
                "BG2 core row {r} has only {non_neg_count} non-negative entries"
            );
        }
    }

    #[test]
    fn test_bg2_extension_has_identity_diagonal() {
        // Extension rows 4..39 have a 0-shift identity entry on their diagonal
        // in the parity part. Rows 40 and 41 are special "punctured" rows
        // that only connect to systematic columns (no identity on diagonal).
        let matrix = bg2::bg2_base_matrix(384);
        for (r, row) in matrix.iter().enumerate().take(40).skip(4) {
            // Check there's a 0 entry somewhere in the parity part
            let has_identity = row[bg2::BG2_KB..].contains(&0);
            assert!(
                has_identity,
                "BG2 extension row {r} missing identity entry in parity part"
            );
        }
        // Rows 40-41 should NOT have identity entries in parity part
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
    // Rate matching tests
    // ========================================================================

    #[test]
    fn test_rate_matched_bg2_256_121() {
        let (qc, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        assert_eq!(params.target_n, 256);
        assert_eq!(params.target_k, 121);
        assert_eq!(params.base_graph, 2);
        assert!(params.lifting_factor >= 13); // ceil(121/10) = 13
        assert!(params.full_k >= 121);
        assert!(params.effective_rate() > 0.0);
        assert!(params.effective_rate() < 1.0);
        // Verify QC structure is valid
        assert_eq!(qc.expansion_factor(), params.lifting_factor);
    }

    #[test]
    fn test_rate_matched_bg2_256_49() {
        let (_, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
        assert_eq!(params.target_n, 256);
        assert_eq!(params.target_k, 49);
        assert!(params.lifting_factor >= 5); // ceil(49/10) = 5
    }

    #[test]
    fn test_rate_matched_bg2_625_225() {
        let (_, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 625, 225);
        assert_eq!(params.target_n, 625);
        assert_eq!(params.target_k, 225);
    }

    #[test]
    fn test_rate_matched_bg2_1024_441() {
        let (_, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
        assert_eq!(params.target_n, 1024);
        assert_eq!(params.target_k, 441);
    }

    #[test]
    fn test_rate_matched_effective_rate() {
        let (_, params) = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let rate = params.effective_rate();
        // 121/256 ~ 0.473
        assert!(
            (rate - 0.473).abs() < 0.01,
            "Expected rate ~0.473, got {rate}"
        );
    }

    // ========================================================================
    // Spot-check specific shift values for BG2
    // ========================================================================

    #[test]
    fn test_bg2_specific_shift_values() {
        // Verify a few known values from the BG2 table
        let matrix = bg2::bg2_base_matrix(384); // Z=384 > max shift, so V mod Z = V

        // Row 0, col 0 should be 38
        assert_eq!(matrix[0][0], 38);
        // Row 0, col 1 should be 52
        assert_eq!(matrix[0][1], 52);
        // Row 0, col 9 should be 103
        assert_eq!(matrix[0][9], 103);
        // Row 1, col 13 should be 1
        assert_eq!(matrix[1][13], 1);
        // Row 2, col 14 should be 0
        assert_eq!(matrix[2][14], 0);
        // Row 3, col 15 should be 0
        assert_eq!(matrix[3][15], 0);
        // Row 4, col 16 should be 0
        assert_eq!(matrix[4][16], 0);
        // Row 41, col 6 should be 37
        assert_eq!(matrix[41][6], 37);
        // Row 41, col 7 should be 80
        assert_eq!(matrix[41][7], 80);
    }

    #[test]
    fn test_bg1_specific_shift_values() {
        let matrix = bg1::bg1_base_matrix(384);

        // Row 0, col 0 should be 250
        assert_eq!(matrix[0][0], 250);
        // Row 0, col 22 should be 56
        assert_eq!(matrix[0][22], 56);
        // Row 1, col 23 should be 5
        assert_eq!(matrix[1][23], 5);
        // Row 2, col 24 should be 0
        assert_eq!(matrix[2][24], 0);
        // Row 45, col 67 should be 0
        assert_eq!(matrix[45][67], 0);
    }

    // ========================================================================
    // Modular reduction test
    // ========================================================================

    #[test]
    fn test_bg2_shifts_mod_z() {
        // With Z=2, all shifts should be 0 or 1
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
        // Each non-negative entry contributes Z edges
        let matrix = bg2::bg2_base_matrix(2);
        let non_neg_count: usize = matrix
            .iter()
            .map(|row| row.iter().filter(|&&v| v >= 0).count())
            .sum();
        assert_eq!(edges.len(), non_neg_count * 2); // Z=2
    }

    // ========================================================================
    // Rows 40-41 (punctured rows) are valid
    // ========================================================================

    #[test]
    fn test_bg2_rows_40_41_have_entries() {
        // Rows 40 and 41 should have non-negative entries (they participate in H)
        let matrix = bg2::bg2_base_matrix(384);
        let row40_nneg: usize = matrix[40].iter().filter(|&&v| v >= 0).count();
        let row41_nneg: usize = matrix[41].iter().filter(|&&v| v >= 0).count();
        assert!(row40_nneg >= 2, "Row 40 should have >= 2 non-neg entries");
        assert!(row41_nneg >= 2, "Row 41 should have >= 2 non-neg entries");
    }
}
