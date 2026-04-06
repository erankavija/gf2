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
//! # 3GPP Rate Matching (TS 38.212 Section 5.3.2)
//!
//! Rate matching works through LLR initialization on the FULL mother code,
//! NOT by removing columns from H. This preserves the Tanner graph structure
//! needed for proper BP convergence (especially for BG1 codes).
//!
//! ## Encoder side
//!
//! 1. **Select Z**: smallest valid Z such that `K_b * Z >= target_k` AND
//!    enough transmitted bits remain after mandatory puncturing.
//! 2. **Pad message**: append `num_filler = K_b * Z - target_k` zero bits.
//! 3. **Encode**: with full mother code to get N = N_b * Z coded bits.
//! 4. **Puncture**: the first `2 * Z` coded output bits are always
//!    punctured (not transmitted).
//! 5. **Rate match**: skip filler positions and transmit E = target_n bits.
//!
//! ## Decoder side
//!
//! 1. Receive target_n LLRs from the channel.
//! 2. Construct full-length N LLR vector:
//!    - First 2*Z positions: LLR = 0 (no channel information)
//!    - Filler bit positions: LLR = +inf (known to be zero)
//!    - Transmitted positions: LLR from channel
//!    - Remaining parity positions: LLR = 0 (punctured parity)
//! 3. Decode with BP on the FULL mother code H.
//! 4. Extract target_k message bits from the decoded output.
//!
//! # Target Code Construction Parameters
//!
//! The following table documents the exact parameters for each of the 6
//! downstream target codes with 3GPP-conformant rate matching.
//!
//! | Target (n, k) | Rate  | BG  | Z   | Mother (N, K)  | Filler | 2*Z punct. | Parity kept | Parity removed |
//! |---------------|-------|-----|-----|----------------|--------|------------|-------------|----------------|
//! | (256, 121)    | 0.473 | BG2 | 13  | (676, 130)     | 9      | 26         | 161         | 385            |
//! | (256, 49)     | 0.191 | BG2 | 6   | (312, 60)      | 11     | 12         | 219         | 33             |
//! | (625, 225)    | 0.360 | BG2 | 24  | (1248, 240)    | 15     | 48         | 448         | 560            |
//! | (1024, 441)   | 0.431 | BG2 | 48  | (2496, 480)    | 39     | 96         | 679         | 1337           |
//! | (1024, 640)   | 0.625 | BG1 | 30  | (2040, 660)    | 20     | 60         | 444         | 936            |
//! | (4096, 3249)  | 0.793 | BG1 | 160 | (10880, 3520)  | 271    | 320        | 1167        | 6193           |
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
use crate::llr::Llr;
use crate::traits::{DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::BitVec;

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
    /// wraps it in an [`Nr5gRateMatchedCode`] that handles 3GPP TS 38.212
    /// rate matching via LLR initialization on the full Tanner graph.
    ///
    /// # 3GPP Rate Matching Algorithm (TS 38.212 Section 5.3.2)
    ///
    /// 1. **Select Z**: The smallest valid lifting size Z such that
    ///    `K_b * Z >= target_k` and enough transmitted bits remain after
    ///    mandatory puncturing of the first 2*Z systematic columns.
    /// 2. **Shortening (filler bits)**: `K_b * Z - target_k` positions at the
    ///    end of the systematic section are forced to zero.
    /// 3. **Mandatory systematic puncturing**: The first `2 * Z` coded bits
    ///    are always punctured (not transmitted).
    /// 4. **Parity truncation**: Excess parity columns are not transmitted.
    ///
    /// Unlike column-removal approaches, this preserves the full mother code
    /// H for BP decoding. Rate matching is handled via LLR initialization:
    /// punctured positions get LLR=0, filler positions get LLR=+inf.
    ///
    /// # Arguments
    ///
    /// * `base_graph` - Base graph number: 1 or 2
    /// * `target_n` - Target codeword length (must satisfy `target_n > target_k`)
    /// * `target_k` - Target message length
    ///
    /// # Returns
    ///
    /// An [`Nr5gRateMatchedCode`] that implements [`BlockEncoder`](crate::traits::BlockEncoder),
    /// [`SoftDecoder`], and [`IterativeSoftDecoder`] with the target (n, k) dimensions.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `base_graph` is not 1 or 2
    /// - `target_n <= target_k`
    /// - No valid lifting size exists for the given (n, k)
    /// - The mother code has insufficient bits for the target n
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(rm_code.n(), 256);
    /// assert_eq!(rm_code.k(), 121);
    /// assert_eq!(rm_code.params().target_n, 256);
    /// assert_eq!(rm_code.params().target_k, 121);
    /// assert_eq!(rm_code.params().num_punctured_systematic, 26); // 2 * Z = 2 * 13
    /// ```
    ///
    /// # Complexity
    ///
    /// O(mb * nb * Z) for expanding the mother code parity-check matrix.
    pub fn nr_5g_rate_matched(
        base_graph: u8,
        target_n: usize,
        target_k: usize,
    ) -> Nr5gRateMatchedCode {
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

        // Step 1: Find the smallest valid Z such that:
        //   (a) K_b * Z >= target_k  (enough information capacity)
        //   (b) target_k + (N_b - K_b - 2) * Z >= target_n  (enough transmitted bits
        //       after mandatory 2*Z systematic puncturing)
        let all_z = all_lifting_sizes();
        let z = all_z
            .iter()
            .copied()
            .find(|&z| {
                let z_us = z as usize;
                kb * z_us >= target_k && target_k + (nb - kb - 2) * z_us >= target_n
            })
            .unwrap_or_else(|| {
                panic!(
                    "No valid lifting size for BG{} with (n={}, k={}): \
                     max possible k = {} * {} = {}",
                    base_graph,
                    target_n,
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

        // Step 2: Compute filler (shortening) count
        let num_filler = full_k - target_k;

        // Step 3: 3GPP mandatory systematic puncturing — first 2*Z columns
        let num_punct_sys = 2 * z;

        // Step 4: Compute parity truncation
        let total_parity = full_n - full_k;
        let remaining_sys = full_k - num_filler - num_punct_sys;
        let available_total = remaining_sys + total_parity;

        assert!(
            available_total >= target_n,
            "After removing {} filler and {} punctured-systematic columns, \
             only {} columns remain but need {} (BG{} Z={})",
            num_filler,
            num_punct_sys,
            available_total,
            target_n,
            base_graph,
            z
        );

        let num_parity_removed = available_total - target_n;
        let parity_kept = total_parity - num_parity_removed;

        let params = NrRateMatchParams {
            base_graph,
            lifting_factor: z,
            target_n,
            target_k,
            full_k,
            full_n,
            num_shortened: num_filler,
            num_punctured_systematic: num_punct_sys,
            num_punctured_parity: num_parity_removed,
            parity_kept,
            kb,
            nb,
        };

        // Build the full mother code
        let qc = Self::nr_5g(base_graph, z);
        let mother_code = LdpcCode::from_quasi_cyclic(&qc);

        // Compute encoding data with column mapping
        let encoding = compute_mother_encoding(&mother_code);

        Nr5gRateMatchedCode {
            mother_code,
            encoding,
            params,
        }
    }
}

/// Parameters describing the 3GPP-conformant rate matching applied to a 5G NR LDPC code.
///
/// Documents the exact construction choices (Z_c, shortening count, puncturing
/// counts) so that encoders and decoders can correctly interpret the code.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::QuasiCyclicLdpc;
///
/// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
/// let params = rm_code.params();
/// assert_eq!(params.base_graph, 2);
/// assert_eq!(params.lifting_factor, 13);
/// assert_eq!(params.num_shortened, 9);
/// assert_eq!(params.num_punctured_systematic, 26); // 2 * Z
/// assert_eq!(params.target_n, 256);
/// assert_eq!(params.target_k, 121);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NrRateMatchParams {
    /// Base graph number (1 or 2).
    pub base_graph: u8,
    /// Lifting factor Z_c used for QC expansion.
    pub lifting_factor: usize,
    /// Target codeword length after rate matching.
    pub target_n: usize,
    /// Target message length.
    pub target_k: usize,
    /// Full message length before shortening (K_b * Z).
    pub full_k: usize,
    /// Full codeword length before puncturing (N_b * Z).
    pub full_n: usize,
    /// Number of shortened (filler) systematic columns removed from the end
    /// of the systematic section (= K_b * Z - target_k).
    pub num_shortened: usize,
    /// Number of mandatory punctured systematic columns (always 2 * Z).
    /// These are the first 2*Z columns, per 3GPP TS 38.212 Section 5.3.2.
    pub num_punctured_systematic: usize,
    /// Number of parity columns removed from the end of the parity section.
    pub num_punctured_parity: usize,
    /// Number of parity columns kept (transmitted).
    pub parity_kept: usize,
    /// K_b: number of systematic base columns in the base graph.
    pub kb: usize,
    /// N_b: total number of base columns in the base graph.
    pub nb: usize,
}

impl NrRateMatchParams {
    /// Returns the effective code rate after rate matching.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let rate = rm_code.params().effective_rate();
    /// assert!((rate - 121.0 / 256.0).abs() < 1e-6);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn effective_rate(&self) -> f64 {
        self.target_k as f64 / self.target_n as f64
    }

    /// Returns the number of active (non-shortened, non-punctured) systematic
    /// columns retained in the rate-matched code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(rm_code.params().active_systematic_bits(), 130 - 9 - 26);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn active_systematic_bits(&self) -> usize {
        self.full_k - self.num_shortened - self.num_punctured_systematic
    }

    /// Returns the number of transmitted parity bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(rm_code.params().transmitted_parity_bits(), 256 - 95);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn transmitted_parity_bits(&self) -> usize {
        self.target_n - self.active_systematic_bits()
    }
}

/// LLR value used for filler (shortened) bit positions.
///
/// Filler bits are known to be zero, so we use a large positive LLR
/// to represent high confidence in bit=0.
const FILLER_LLR: f32 = 1000.0;

/// Encoding data for the mother code with right-pivot column mapping.
///
/// Contains the parity matrix and the systematic/parity column indices
/// computed via RREF with right-to-left pivoting on the parity-check matrix H.
/// This matches the encoder's internal column ordering, ensuring that the
/// codeword positions are consistent with H's column layout for BP decoding.
#[derive(Clone)]
struct MotherEncoding {
    /// Parity matrix P (k × m): for systematic codes, G = [I at sys_cols | P at par_cols]
    parity_matrix: gf2_core::BitMatrix,
    /// Sorted systematic (information) column indices in the codeword
    systematic_cols: Vec<usize>,
    /// Sorted parity column indices in the codeword
    parity_cols: Vec<usize>,
}

/// Computes the encoding data for an LDPC code using RREF from the right.
///
/// Performs Gaussian elimination with right-to-left pivoting to identify
/// m pivot columns (parity positions) and k non-pivot columns (systematic
/// positions). Then computes the parity matrix P such that for each
/// systematic basis vector e_i, the codeword places 1 at systematic_cols[i]
/// and the corresponding parity bits at parity_cols.
///
/// # Arguments
///
/// * `code` - The LDPC code with parity-check matrix H
///
/// # Returns
///
/// A `MotherEncoding` containing the parity matrix and column mappings.
///
/// # Panics
///
/// Panics if H is not full rank.
///
/// # Complexity
///
/// O(m * n * min(m, n)) for Gaussian elimination.
fn compute_mother_encoding(code: &LdpcCode) -> MotherEncoding {
    use gf2_core::BitMatrix;

    let n = code.n();
    let m = code.m();
    let k = n - m;
    let h = code.parity_check_matrix();

    // Convert sparse H to dense for RREF
    let mut work = BitMatrix::zeros(m, n);
    for row in 0..m {
        for col in h.row_iter(row) {
            work.set(row, col, true);
        }
    }

    // RREF from right: find pivots starting from rightmost column
    let mut pivot_cols = Vec::with_capacity(m);
    let mut current_row = 0;
    let mut col_idx = n;

    while current_row < m && col_idx > 0 {
        col_idx -= 1;
        let col = col_idx;

        // Find pivot row in current_row..m
        let mut pivot_row = None;
        for row in current_row..m {
            if work.get(row, col) {
                pivot_row = Some(row);
                break;
            }
        }

        let Some(pivot_row) = pivot_row else {
            continue;
        };

        // Swap with current_row
        if pivot_row != current_row {
            work.swap_rows(current_row, pivot_row);
        }

        // Eliminate all other rows
        for row in 0..m {
            if row != current_row && work.get(row, col) {
                work.row_xor(row, current_row);
            }
        }

        pivot_cols.push(col);
        current_row += 1;
    }

    assert_eq!(
        pivot_cols.len(),
        m,
        "H matrix is rank-deficient: rank {} < m {}",
        pivot_cols.len(),
        m
    );

    // Sort pivot_cols for consistent ordering
    pivot_cols.sort_unstable();

    // Systematic columns are non-pivot columns (sorted)
    let pivot_set: std::collections::HashSet<usize> = pivot_cols.iter().copied().collect();
    let systematic_cols: Vec<usize> = (0..n).filter(|c| !pivot_set.contains(c)).collect();
    assert_eq!(systematic_cols.len(), k);

    // Reorder rows so that row i has its pivot at parity_cols[i]
    // After RREF, each row has exactly one pivot column with a 1
    let mut row_for_pivot = vec![0usize; m];
    for row in 0..m {
        for (pi, &pcol) in pivot_cols.iter().enumerate() {
            if work.get(row, pcol) {
                row_for_pivot[pi] = row;
                break;
            }
        }
    }

    // Build parity matrix P (k × m)
    // P[i, j] = work[row_for_pivot[j], systematic_cols[i]]
    let mut parity_matrix = BitMatrix::zeros(k, m);
    for (i, &sys_col) in systematic_cols.iter().enumerate() {
        for (j, &pivot_row) in row_for_pivot.iter().enumerate() {
            if work.get(pivot_row, sys_col) {
                parity_matrix.set(i, j, true);
            }
        }
    }

    MotherEncoding {
        parity_matrix,
        systematic_cols,
        parity_cols: pivot_cols,
    }
}

/// A 5G NR LDPC code with 3GPP-conformant rate matching.
///
/// Wraps the full mother code and handles rate matching through LLR
/// initialization rather than H column removal. This preserves the full
/// Tanner graph for proper BP convergence on both BG1 and BG2 codes.
///
/// # Encoding
///
/// 1. Pad the target_k message with filler zeros to reach full_k = K_b * Z.
/// 2. Encode with the full mother code to get full_n = N_b * Z coded bits.
/// 3. Extract target_n transmitted bits (skip punctured/filler positions).
///
/// # Decoding
///
/// 1. Receive target_n channel LLRs.
/// 2. Map to full_n LLR vector with proper initialization:
///    - Punctured systematic (first 2*Z): LLR = 0
///    - Filler positions: LLR = +1000 (known zero)
///    - Transmitted positions: channel LLR
///    - Untransmitted parity: LLR = 0
/// 3. BP decode on the full mother code H.
/// 4. Extract target_k message bits.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::QuasiCyclicLdpc;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_core::BitVec;
///
/// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
/// assert_eq!(rm_code.n(), 256);
/// assert_eq!(rm_code.k(), 121);
///
/// let msg = BitVec::zeros(121);
/// let codeword = rm_code.encode(&msg);
/// assert_eq!(codeword.len(), 256);
/// ```
#[derive(Clone)]
pub struct Nr5gRateMatchedCode {
    /// Full mother code (N_b * Z columns, M_b * Z rows).
    mother_code: LdpcCode,
    /// Encoding data: parity matrix + systematic/parity column mapping.
    encoding: MotherEncoding,
    /// Rate matching parameters.
    params: NrRateMatchParams,
}

impl Nr5gRateMatchedCode {
    /// Returns the target codeword length (transmitted bits).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(rm_code.n(), 256);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn n(&self) -> usize {
        self.params.target_n
    }

    /// Returns the target message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(rm_code.k(), 121);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn k(&self) -> usize {
        self.params.target_k
    }

    /// Returns a reference to the rate matching parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// assert_eq!(rm_code.params().lifting_factor, 13);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn params(&self) -> &NrRateMatchParams {
        &self.params
    }

    /// Returns a reference to the full mother code.
    ///
    /// The mother code has n = N_b * Z columns and m = M_b * Z rows.
    /// BP decoding operates on this full code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let mother = rm_code.mother_code();
    /// assert_eq!(mother.n(), 52 * 13); // BG2 N_b=52, Z=13
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn mother_code(&self) -> &LdpcCode {
        &self.mother_code
    }

    /// Encodes a target_k message into a target_n transmitted codeword.
    ///
    /// 1. Pads with filler zeros to reach full_k.
    /// 2. Encodes with full mother code to get full_n bits.
    /// 3. Extracts target_n transmitted bits.
    ///
    /// # Arguments
    ///
    /// * `message` - A bit vector of length target_k
    ///
    /// # Returns
    ///
    /// A bit vector of length target_n
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != target_k`.
    /// Encodes using the mother code and applies rate matching.
    ///
    /// The encoding uses the RREF-derived column mapping to place
    /// message and parity bits at the correct codeword positions.
    fn encode_rate_matched(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.params.target_k,
            "Message length {} must equal target_k = {}",
            message.len(),
            self.params.target_k
        );

        let p = &self.params;
        let enc = &self.encoding;

        // Step 1: Pad message with filler zeros to full_k
        let mut padded = BitVec::zeros(p.full_k);
        for i in 0..p.target_k {
            padded.set(i, message.get(i));
        }

        // Step 2: Compute parity bits: parity = P^T * padded_message
        let parity = enc.parity_matrix.matvec_transpose(&padded);

        // Step 3: Build full codeword using the column mapping
        let mut codeword = BitVec::zeros(p.full_n);
        for (i, &col) in enc.systematic_cols.iter().enumerate() {
            codeword.set(col, padded.get(i));
        }
        for (j, &col) in enc.parity_cols.iter().enumerate() {
            codeword.set(col, parity.get(j));
        }

        // Step 4: Extract transmitted bits
        // Transmitted positions in codeword (same positions as in H):
        //   - Skip first 2*Z positions (always punctured)
        //   - Skip filler positions (full_k - num_shortened .. full_k - 1)
        //   - Take parity_kept parity positions starting from full_k
        let mut output = BitVec::with_capacity(p.target_n);

        // Transmitted systematic: positions [2*Z .. full_k - num_shortened)
        for i in p.num_punctured_systematic..(p.full_k - p.num_shortened) {
            output.push_bit(codeword.get(i));
        }

        // Transmitted parity: positions [full_k .. full_k + parity_kept)
        for i in p.full_k..(p.full_k + p.parity_kept) {
            output.push_bit(codeword.get(i));
        }

        debug_assert_eq!(output.len(), p.target_n);
        output
    }

    /// Constructs the full-length LLR vector from target_n channel LLRs.
    ///
    /// Maps channel LLRs back to the full mother code positions:
    /// - First 2*Z positions: LLR = 0 (punctured systematic, no info)
    /// - Active systematic positions: channel LLRs
    /// - Filler positions: LLR = +1000 (known to be zero)
    /// - Transmitted parity positions: channel LLRs
    /// - Untransmitted parity positions: LLR = 0 (punctured)
    ///
    /// # Arguments
    ///
    /// * `channel_llrs` - LLR values for target_n received positions
    ///
    /// # Returns
    ///
    /// Full-length (full_n) LLR vector for BP decoding.
    ///
    /// # Panics
    ///
    /// Panics if `channel_llrs.len() != target_n`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    /// use gf2_coding::llr::Llr;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let target_n = rm_code.params().target_n;
    /// let full_n = rm_code.params().full_n;
    ///
    /// // Simulate target_n channel LLRs (all strongly positive = likely zero bits)
    /// let channel_llrs: Vec<Llr> = (0..target_n).map(|_| Llr::new(3.0)).collect();
    /// let full_llrs = rm_code.prepare_llrs(&channel_llrs);
    ///
    /// assert_eq!(full_llrs.len(), full_n);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(full_n) where full_n = N_b * Z.
    pub fn prepare_llrs(&self, channel_llrs: &[Llr]) -> Vec<Llr> {
        assert_eq!(
            channel_llrs.len(),
            self.params.target_n,
            "Channel LLR length {} must equal target_n = {}",
            channel_llrs.len(),
            self.params.target_n
        );

        let p = &self.params;
        let mut full_llrs = vec![Llr::zero(); p.full_n];

        // Track position in channel_llrs via an iterator
        let mut ch_iter = channel_llrs.iter();

        // Map channel LLRs to the same codeword positions used during encoding:
        // Transmitted positions = [2*Z .. full_k - num_shortened) ∪ [full_k .. full_k + parity_kept)
        for slot in &mut full_llrs[p.num_punctured_systematic..(p.full_k - p.num_shortened)] {
            *slot = *ch_iter.next().unwrap();
        }
        for slot in &mut full_llrs[p.full_k..(p.full_k + p.parity_kept)] {
            *slot = *ch_iter.next().unwrap();
        }

        debug_assert_eq!(ch_iter.len(), 0, "All channel LLRs should be consumed");

        // Set filler LLRs: filler message indices are target_k..full_k-1.
        // These message bits were forced to zero during encoding.
        // Their codeword positions are systematic_cols[target_k..full_k-1].
        for i in p.target_k..p.full_k {
            let cw_pos = self.encoding.systematic_cols[i];
            full_llrs[cw_pos] = Llr::new(FILLER_LLR);
        }

        // All other positions (punctured systematic 0..2*Z, untransmitted parity,
        // and any non-transmitted positions) remain at LLR=0.

        full_llrs
    }

    /// Extracts target_k message bits from a full decoded codeword.
    ///
    /// The decoded codeword has full_n bits indexed by H column position.
    /// Message bit `i` is at position `systematic_cols[i]` in the codeword.
    /// This method extracts the first target_k message bits, discarding filler.
    ///
    /// # Arguments
    ///
    /// * `decoded_codeword` - Decoded codeword from mother code (length full_n)
    ///
    /// # Returns
    ///
    /// Extracted message bits of length target_k.
    ///
    /// # Panics
    ///
    /// Panics if `decoded_codeword.len() < full_n` (the codeword is shorter than
    /// the full mother code length, so systematic column indices may be out of bounds).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    /// use gf2_core::BitVec;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let full_n = rm_code.params().full_n;
    /// let target_k = rm_code.params().target_k;
    ///
    /// // Simulate a decoded all-zero codeword of full mother-code length
    /// let decoded = BitVec::zeros(full_n);
    /// let message = rm_code.extract_message(&decoded);
    ///
    /// assert_eq!(message.len(), target_k);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(target_k).
    pub fn extract_message(&self, decoded_codeword: &BitVec) -> BitVec {
        let mut msg = BitVec::with_capacity(self.params.target_k);
        for i in 0..self.params.target_k {
            msg.push_bit(decoded_codeword.get(self.encoding.systematic_cols[i]));
        }
        msg
    }
}

impl crate::traits::BlockEncoder for Nr5gRateMatchedCode {
    fn k(&self) -> usize {
        self.params.target_k
    }

    fn n(&self) -> usize {
        self.params.target_n
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        self.encode_rate_matched(message)
    }
}

impl SoftDecoder for Nr5gRateMatchedCode {
    fn k(&self) -> usize {
        self.params.target_k
    }

    fn n(&self) -> usize {
        self.params.target_n
    }

    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        assert_eq!(
            llrs.len(),
            self.params.target_n,
            "LLR length must equal target_n = {}",
            self.params.target_n
        );
        // Use the full BP decoder — decode_iterative handles prepare_llrs internally
        let mut decoder = Nr5gRateMatchedDecoder::new((*self).clone());
        decoder.decode_iterative(llrs, 50).decoded_bits
    }
}

/// Normalized min-sum scaling factor for check-to-variable messages.
///
/// The standard min-sum algorithm overestimates check-to-variable messages.
/// Multiplying by alpha ≈ 0.75 corrects this and is critical for convergence
/// when many variable nodes are punctured (LLR=0), as in rate-matched codes.
const MINSUM_SCALE: f32 = 0.75;

/// Iterative BP decoder for rate-matched 5G NR LDPC codes.
///
/// Implements normalized min-sum belief propagation on the FULL mother code.
/// The normalization factor [`MINSUM_SCALE`] is essential for convergence when
/// the LLR vector contains many punctured positions (LLR=0).
///
/// # Examples
///
/// ```ignore
/// use gf2_coding::ldpc::QuasiCyclicLdpc;
/// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
/// use gf2_coding::traits::IterativeSoftDecoder;
///
/// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
/// let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
/// let result = decoder.decode_iterative(&channel_llrs, 50);
/// ```
pub struct Nr5gRateMatchedDecoder {
    /// The rate-matched code (owns mother code, encoder, params).
    rm_code: Nr5gRateMatchedCode,
    /// Cached check node neighbors: check_neighbors[check] = [var1, var2, ...]
    check_neighbors: Vec<Vec<usize>>,
    /// Cached variable node neighbors: var_neighbors[var] = [check1, check2, ...]
    var_neighbors: Vec<Vec<usize>>,
    /// Current variable node beliefs (posterior LLRs)
    beliefs: Vec<f32>,
    /// Check-to-variable messages: c2v[check][pos] for neighbors[pos]
    c2v: Vec<Vec<f32>>,
    /// Variable-to-check messages: v2c[var][pos] for neighbors[pos]
    v2c: Vec<Vec<f32>>,
    /// Last iteration count
    last_iterations: usize,
}

impl Nr5gRateMatchedDecoder {
    /// Creates a new rate-matched decoder.
    ///
    /// Pre-computes the Tanner graph adjacency from the mother code's H matrix.
    ///
    /// # Arguments
    ///
    /// * `rm_code` - The rate-matched code to decode
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let decoder = Nr5gRateMatchedDecoder::new(rm_code);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n).
    pub fn new(rm_code: Nr5gRateMatchedCode) -> Self {
        let h = rm_code.mother_code.parity_check_matrix();
        let n = rm_code.mother_code.n();
        let m = rm_code.mother_code.m();

        let check_neighbors: Vec<Vec<usize>> = (0..m).map(|r| h.row_iter(r).collect()).collect();
        let var_neighbors: Vec<Vec<usize>> = (0..n).map(|c| h.col_iter(c).collect()).collect();

        let c2v: Vec<Vec<f32>> = check_neighbors
            .iter()
            .map(|nb| vec![0.0; nb.len()])
            .collect();
        let v2c: Vec<Vec<f32>> = var_neighbors.iter().map(|nb| vec![0.0; nb.len()]).collect();

        Self {
            rm_code,
            check_neighbors,
            var_neighbors,
            beliefs: vec![0.0; n],
            c2v,
            v2c,
            last_iterations: 0,
        }
    }

    /// Returns the target codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let decoder = Nr5gRateMatchedDecoder::new(rm_code);
    /// assert_eq!(decoder.n(), 256);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn n(&self) -> usize {
        self.rm_code.n()
    }

    /// Returns the target message length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let decoder = Nr5gRateMatchedDecoder::new(rm_code);
    /// assert_eq!(decoder.k(), 121);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn k(&self) -> usize {
        self.rm_code.k()
    }

    /// Returns a reference to the rate matching parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let decoder = Nr5gRateMatchedDecoder::new(rm_code);
    /// assert_eq!(decoder.params().target_n, 256);
    /// assert_eq!(decoder.params().target_k, 121);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn params(&self) -> &NrRateMatchParams {
        self.rm_code.params()
    }

    /// Returns a reference to the underlying rate-matched code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    /// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    ///
    /// let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    /// let decoder = Nr5gRateMatchedDecoder::new(rm_code);
    /// assert_eq!(decoder.code().n(), 256);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn code(&self) -> &Nr5gRateMatchedCode {
        &self.rm_code
    }

    /// Runs normalized min-sum BP on the full mother code.
    fn bp_decode(&mut self, channel_llrs: &[f32], max_iterations: usize) -> DecoderResult {
        let n = self.rm_code.mother_code.n();

        // Reset check-to-variable messages
        for msgs in &mut self.c2v {
            for m in msgs.iter_mut() {
                *m = 0.0;
            }
        }

        // Initialize variable-to-check messages with channel LLRs
        for (var, ch_llr) in channel_llrs.iter().enumerate().take(n) {
            for slot in &mut self.v2c[var] {
                *slot = *ch_llr;
            }
        }

        let mut iterations = 0;
        let mut converged = false;

        for iter in 0..max_iterations {
            iterations = iter + 1;

            // === Check node update (normalized min-sum) ===
            for (check, neighbors) in self.check_neighbors.iter().enumerate() {
                let deg = neighbors.len();
                for pos in 0..deg {
                    // Compute extrinsic min-sum: product of signs, minimum magnitude
                    let mut sign = 1i8;
                    let mut min_abs = f32::MAX;
                    for (other_pos, &other_var) in neighbors.iter().enumerate() {
                        if other_pos != pos {
                            let var_check_pos = self.var_neighbors[other_var]
                                .iter()
                                .position(|&c| c == check)
                                .unwrap();
                            let msg = self.v2c[other_var][var_check_pos];
                            if msg < 0.0 {
                                sign = -sign;
                            }
                            let abs = msg.abs();
                            if abs < min_abs {
                                min_abs = abs;
                            }
                        }
                    }
                    // Apply normalization scaling
                    self.c2v[check][pos] = sign as f32 * min_abs * MINSUM_SCALE;
                }
            }

            // === Variable node update ===
            for (var, ch_llr) in channel_llrs.iter().enumerate().take(n) {
                let neighbors = &self.var_neighbors[var];
                // Total belief = channel + sum of incoming check messages
                let mut belief = *ch_llr;
                for &check in neighbors {
                    let check_pos = self.check_neighbors[check]
                        .iter()
                        .position(|&v| v == var)
                        .unwrap();
                    belief += self.c2v[check][check_pos];
                }
                self.beliefs[var] = belief;

                // Extrinsic variable-to-check messages
                for (vpos, &check) in neighbors.iter().enumerate() {
                    let check_pos = self.check_neighbors[check]
                        .iter()
                        .position(|&v| v == var)
                        .unwrap();
                    self.v2c[var][vpos] = belief - self.c2v[check][check_pos];
                }
            }

            // === Syndrome check ===
            let mut decoded = BitVec::with_capacity(n);
            for &b in &self.beliefs {
                decoded.push_bit(b < 0.0);
            }
            if self.rm_code.mother_code.is_valid_codeword(&decoded) {
                converged = true;
                break;
            }
        }

        self.last_iterations = iterations;

        // Hard decode the full codeword
        let mut decoded_cw = BitVec::with_capacity(n);
        for &b in &self.beliefs {
            decoded_cw.push_bit(b < 0.0);
        }
        let syndrome_passed = self.rm_code.mother_code.is_valid_codeword(&decoded_cw);

        // Return the full decoded codeword (extract_message will pick out
        // the message bits using the systematic column mapping)
        DecoderResult::new(decoded_cw, iterations, converged, syndrome_passed)
    }
}

impl SoftDecoder for Nr5gRateMatchedDecoder {
    fn k(&self) -> usize {
        self.rm_code.k()
    }

    fn n(&self) -> usize {
        self.rm_code.n()
    }

    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        self.rm_code.decode_soft(llrs)
    }
}

impl IterativeSoftDecoder for Nr5gRateMatchedDecoder {
    fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
        assert_eq!(
            llrs.len(),
            self.rm_code.params.target_n,
            "LLR length {} must equal target_n = {}",
            llrs.len(),
            self.rm_code.params.target_n
        );

        // Map channel LLRs to full mother code LLR vector
        let full_llrs = self.rm_code.prepare_llrs(llrs);

        // Convert to f32 for BP
        let llr_f32: Vec<f32> = full_llrs.iter().map(|l| l.value()).collect();

        // Run normalized min-sum BP on full mother code
        let mother_result = self.bp_decode(&llr_f32, max_iterations);

        // Extract target_k message bits from decoded mother code output
        let message = self.rm_code.extract_message(&mother_result.decoded_bits);

        DecoderResult::new(
            message,
            mother_result.iterations,
            mother_result.converged,
            mother_result.syndrome_check_passed,
        )
    }

    fn last_iteration_count(&self) -> usize {
        self.last_iterations
    }

    fn reset(&mut self) {
        for msgs in &mut self.c2v {
            for m in msgs.iter_mut() {
                *m = 0.0;
            }
        }
        for msgs in &mut self.v2c {
            for m in msgs.iter_mut() {
                *m = 0.0;
            }
        }
        for b in &mut self.beliefs {
            *b = 0.0;
        }
        self.last_iterations = 0;
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
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 256, "n mismatch");
        assert_eq!(rm_code.k(), 121, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 13);
        assert_eq!(params.full_k, 130); // 10 * 13
        assert_eq!(params.full_n, 676); // 52 * 13
        assert_eq!(params.num_shortened, 9); // 130 - 121
        assert_eq!(params.num_punctured_systematic, 26); // 2 * 13
        assert_eq!(params.num_punctured_parity, 385); // 546 - 161
        assert_eq!(params.target_n, 256);
        assert_eq!(params.target_k, 121);
        assert_eq!(params.active_systematic_bits(), 95); // 130 - 9 - 26
        assert_eq!(params.transmitted_parity_bits(), 161);
    }

    #[test]
    fn test_rate_matched_bg2_256_49_exact_dimensions() {
        // With 3GPP puncturing (2*Z mandatory), Z=5 is too small: only 249 bits
        // available. Needs Z=6 (set 1: a=3, j=1).
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 256, "n mismatch");
        assert_eq!(rm_code.k(), 49, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 6); // Z=6 (not 5) due to 2*Z puncturing
        assert_eq!(params.full_k, 60); // 10 * 6
        assert_eq!(params.full_n, 312); // 52 * 6
        assert_eq!(params.num_shortened, 11); // 60 - 49
        assert_eq!(params.num_punctured_systematic, 12); // 2 * 6
        assert_eq!(params.active_systematic_bits(), 37); // 60 - 11 - 12
        assert_eq!(params.transmitted_parity_bits(), 219);
    }

    #[test]
    fn test_rate_matched_bg2_625_225_exact_dimensions() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 625, 225);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 625, "n mismatch");
        assert_eq!(rm_code.k(), 225, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 24);
        assert_eq!(params.full_k, 240);
        assert_eq!(params.num_shortened, 15);
        assert_eq!(params.num_punctured_systematic, 48); // 2 * 24
        assert_eq!(params.active_systematic_bits(), 177); // 240 - 15 - 48
        assert_eq!(params.transmitted_parity_bits(), 448);
    }

    #[test]
    fn test_rate_matched_bg2_1024_441_exact_dimensions() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 1024, "n mismatch");
        assert_eq!(rm_code.k(), 441, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 48);
        assert_eq!(params.full_k, 480);
        assert_eq!(params.num_shortened, 39);
        assert_eq!(params.num_punctured_systematic, 96); // 2 * 48
        assert_eq!(params.active_systematic_bits(), 345); // 480 - 39 - 96
        assert_eq!(params.transmitted_parity_bits(), 679);
    }

    #[test]
    fn test_rate_matched_bg1_1024_640_exact_dimensions() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 640);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 1024, "n mismatch");
        assert_eq!(rm_code.k(), 640, "k mismatch");
        assert_eq!(params.base_graph, 1);
        assert_eq!(params.lifting_factor, 30);
        assert_eq!(params.full_k, 660); // 22 * 30
        assert_eq!(params.num_shortened, 20);
        assert_eq!(params.num_punctured_systematic, 60); // 2 * 30
        assert_eq!(params.active_systematic_bits(), 580); // 660 - 20 - 60
        assert_eq!(params.transmitted_parity_bits(), 444);
    }

    #[test]
    fn test_rate_matched_bg1_4096_3249_exact_dimensions() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(1, 4096, 3249);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 4096, "n mismatch");
        assert_eq!(rm_code.k(), 3249, "k mismatch");
        assert_eq!(params.base_graph, 1);
        assert_eq!(params.lifting_factor, 160);
        assert_eq!(params.full_k, 3520); // 22 * 160
        assert_eq!(params.num_shortened, 271);
        assert_eq!(params.num_punctured_systematic, 320); // 2 * 160
        assert_eq!(params.active_systematic_bits(), 2929); // 3520 - 271 - 320
        assert_eq!(params.transmitted_parity_bits(), 1167);
    }

    // ========================================================================
    // Rate matching: BP decoding convergence on all-zero codeword via LLR
    // ========================================================================

    /// Helper: verify BP decoding converges on the zero codeword for a
    /// rate-matched code. Uses the full mother code with LLR mapping.
    fn assert_bp_converges_rate_matched(bg: u8, target_n: usize, target_k: usize, label: &str) {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);
        let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
        // All-zero codeword: positive LLR means "likely 0"
        let llrs: Vec<Llr> = vec![Llr::new(10.0); target_n];
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
        assert_bp_converges_rate_matched(2, 256, 121, "BG2 (256,121)");
    }

    #[test]
    fn test_rate_matched_bg2_256_49_bp_converges() {
        assert_bp_converges_rate_matched(2, 256, 49, "BG2 (256,49)");
    }

    #[test]
    fn test_rate_matched_bg2_625_225_bp_converges() {
        assert_bp_converges_rate_matched(2, 625, 225, "BG2 (625,225)");
    }

    #[test]
    fn test_rate_matched_bg2_1024_441_bp_converges() {
        assert_bp_converges_rate_matched(2, 1024, 441, "BG2 (1024,441)");
    }

    #[test]
    fn test_rate_matched_bg1_1024_640_bp_converges() {
        assert_bp_converges_rate_matched(1, 1024, 640, "BG1 (1024,640)");
    }

    #[test]
    fn test_rate_matched_bg1_4096_3249_bp_converges() {
        assert_bp_converges_rate_matched(1, 4096, 3249, "BG1 (4096,3249)");
    }

    // ========================================================================
    // Rate matching: zero codeword encode roundtrip
    // ========================================================================

    #[test]
    fn test_rate_matched_bg2_256_121_zero_encode() {
        use crate::traits::BlockEncoder;
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let msg = BitVec::zeros(121);
        let cw = rm_code.encode(&msg);
        assert_eq!(cw.len(), 256);
        // All-zero message should produce all-zero codeword (linear code)
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_rate_matched_bg1_1024_640_zero_encode() {
        use crate::traits::BlockEncoder;
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 640);
        let msg = BitVec::zeros(640);
        let cw = rm_code.encode(&msg);
        assert_eq!(cw.len(), 1024);
        assert_eq!(cw.count_ones(), 0);
    }

    // ========================================================================
    // Rate matching: effective rate
    // ========================================================================

    #[test]
    fn test_rate_matched_effective_rate() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let rate = rm_code.params().effective_rate();
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

    // ========================================================================
    // Rate matching: 3GPP params consistency
    // ========================================================================

    #[test]
    fn test_rate_matched_params_consistency() {
        // Verify that active_systematic + transmitted_parity == target_n
        // for all 6 target codes.
        let cases: &[(u8, usize, usize)] = &[
            (2, 256, 121),
            (2, 256, 49),
            (2, 625, 225),
            (2, 1024, 441),
            (1, 1024, 640),
            (1, 4096, 3249),
        ];
        for &(bg, n, k) in cases {
            let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k);
            let params = rm_code.params();
            assert_eq!(rm_code.n(), n);
            assert_eq!(rm_code.k(), k);
            assert_eq!(
                params.active_systematic_bits() + params.transmitted_parity_bits(),
                n,
                "BG{bg} ({n},{k}): active_sys + parity != target_n"
            );
            assert_eq!(
                params.num_punctured_systematic,
                2 * params.lifting_factor,
                "BG{bg} ({n},{k}): punctured_systematic != 2*Z"
            );
            assert_eq!(
                params.num_shortened,
                params.full_k - k,
                "BG{bg} ({n},{k}): shortened != full_k - target_k"
            );
        }
    }

    // ========================================================================
    // BER acceptance test: encode, BPSK+AWGN, BP decode (full mother code)
    // ========================================================================

    /// BER acceptance test: encode random messages through a rate-matched
    /// 5G NR LDPC code, transmit over BPSK+AWGN at the given Eb/N0, BP
    /// decode on the FULL mother code with LLR mapping, and verify BER
    /// is below the threshold.
    fn ber_acceptance(
        bg: u8,
        target_n: usize,
        target_k: usize,
        eb_n0_db: f64,
        max_ber: f64,
        num_frames: usize,
        label: &str,
    ) {
        use crate::channel::{AwgnChannel, BpskModulator};
        use crate::traits::BlockEncoder;
        use rand::Rng;

        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);
        let mut decoder = Nr5gRateMatchedDecoder::new(QuasiCyclicLdpc::nr_5g_rate_matched(
            bg, target_n, target_k,
        ));

        let rate = rm_code.params().effective_rate();
        let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, rate);
        let sigma_sq = channel.variance();
        let mut rng = rand::thread_rng();

        let mut total_bits = 0usize;
        let mut bit_errors = 0usize;
        let mut frame_errors = 0usize;
        let mut frames_decoded = 0usize;

        for _ in 0..num_frames {
            // Generate random message
            let mut msg = BitVec::zeros(target_k);
            for i in 0..target_k {
                if rng.gen_bool(0.5) {
                    msg.set(i, true);
                }
            }

            // Encode with rate matching
            let codeword = rm_code.encode(&msg);
            assert_eq!(codeword.len(), target_n);

            // BPSK modulate
            let bits: Vec<bool> = (0..target_n).map(|i| codeword.get(i)).collect();
            let symbols = BpskModulator::modulate_bits(&bits);

            // Transmit through AWGN channel
            let received = channel.transmit_symbols(&symbols, &mut rng);

            // Convert to LLRs
            let llrs: Vec<Llr> = received
                .iter()
                .map(|&r| BpskModulator::to_llr(r, sigma_sq))
                .collect();

            // BP decode on full mother code via rate-matched decoder
            let result = decoder.decode_iterative(&llrs, 50);

            if result.converged {
                frames_decoded += 1;
            }

            // Compare decoded message with original
            let mut frame_has_error = false;
            for i in 0..target_k {
                if result.decoded_bits.get(i) != msg.get(i) {
                    bit_errors += 1;
                    frame_has_error = true;
                }
            }
            if frame_has_error {
                frame_errors += 1;
            }
            total_bits += target_k;
        }

        let ber = bit_errors as f64 / total_bits as f64;
        let bler = frame_errors as f64 / num_frames as f64;
        assert!(
            ber < max_ber,
            "{label}: BER = {ber:.2e} exceeds threshold {max_ber:.2e} \
             ({bit_errors}/{total_bits} errors, {num_frames} frames, \
             {frames_decoded}/{num_frames} converged, \
             BLER = {bler:.2e}, Eb/N0 = {eb_n0_db} dB)"
        );
        // BLER must also be reasonable (at least some frames decoded correctly)
        assert!(
            bler < 1.0,
            "{label}: BLER = 1.0 — no frame decoded correctly at {eb_n0_db} dB"
        );
    }

    #[test]
    fn test_ber_bg2_256_121_6db() {
        // BG2 (256, 121) at 6 dB: low-rate code, expect near-zero BER
        ber_acceptance(2, 256, 121, 6.0, 1e-3, 20, "BG2 (256,121) @ 6dB");
    }

    #[test]
    fn test_ber_bg2_256_49_6db() {
        // BG2 (256, 49) at 6 dB: very low rate, strong protection
        ber_acceptance(2, 256, 49, 6.0, 1e-3, 20, "BG2 (256,49) @ 6dB");
    }

    #[test]
    fn test_ber_bg2_1024_441_6db() {
        // BG2 (1024, 441) at 6 dB: moderate rate 0.43 with larger block
        ber_acceptance(2, 1024, 441, 6.0, 1e-3, 10, "BG2 (1024,441) @ 6dB");
    }

    #[test]
    fn test_ber_bg2_625_225_6db() {
        // BG2 (625, 225) at 6 dB: rate 0.36
        ber_acceptance(2, 625, 225, 6.0, 1e-3, 10, "BG2 (625,225) @ 6dB");
    }

    #[test]
    fn test_ber_bg1_1024_640_8db() {
        // BG1 (1024, 640) at 8 dB: previously broken with column-removal approach
        ber_acceptance(1, 1024, 640, 8.0, 1e-3, 10, "BG1 (1024,640) @ 8dB");
    }

    #[test]
    fn test_ber_bg1_4096_3249_8db() {
        // BG1 (4096, 3249) at 8 dB: high rate 0.793
        ber_acceptance(1, 4096, 3249, 8.0, 1e-2, 5, "BG1 (4096,3249) @ 8dB");
    }

    // -----------------------------------------------------------------------
    // SoftDecoder regression test
    // -----------------------------------------------------------------------

    #[test]
    fn test_soft_decoder_trait_roundtrip() {
        use crate::traits::{BlockEncoder, SoftDecoder};

        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let msg = gf2_core::BitVec::zeros(121);
        let cw = rm_code.encode(&msg);

        // Create noiseless LLRs: bit 0 → +5.0, bit 1 → -5.0
        let llrs: Vec<Llr> = (0..256)
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();

        let decoded = rm_code.decode_soft(&llrs);
        assert_eq!(decoded.len(), 121);
        for i in 0..121 {
            assert_eq!(decoded.get(i), msg.get(i), "bit {} mismatch", i);
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
        fn prop_encode_produces_correct_length(bg in 1u8..=2, seed in 0u64..100) {
            // Use a fixed target per BG to keep the test fast
            let (target_n, target_k) = if bg == 1 { (1024, 640) } else { (256, 121) };
            let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);

            // Generate a deterministic random message
            use rand::rngs::StdRng;
            use rand::SeedableRng;
            let mut rng = StdRng::seed_from_u64(seed);
            let msg = gf2_core::BitVec::random(target_k, &mut rng);

            let cw = rm_code.encode(&msg);
            prop_assert_eq!(cw.len(), target_n, "codeword length must be target_n");
        }

        #[test]
        fn prop_prepare_llrs_correct_length(bg in 1u8..=2) {
            let (target_n, target_k) = if bg == 1 { (1024, 640) } else { (256, 121) };
            let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);

            let channel_llrs: Vec<Llr> = vec![Llr::new(1.0); target_n];
            let full_llrs = rm_code.prepare_llrs(&channel_llrs);
            prop_assert_eq!(full_llrs.len(), rm_code.mother_code().n());
        }
    }
}
