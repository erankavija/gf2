//! 5G NR LDPC code construction from `@/citation/ThreeGpp2017` Section 5.3.2.
//!
//! [`QuasiCyclicLdpc::nr_5g`] expands base graph BG1 (46x68, K_b = 22) or BG2
//! (42x52, K_b = 10) by a lifting size Z from Table 5.3.2-1: each base entry V
//! becomes a Z x Z circulant with shift `V mod Z`, and -1 a zero block.
//! [`QuasiCyclicLdpc::nr_5g_rate_matched`] adds rate matching, which
//! [`Nr5gRateMatchedCode`] applies through LLR initialization on the full
//! mother code, keeping every column of H.

pub(crate) mod bg1;
pub(crate) mod bg2;
pub mod interleaver;
pub mod lifting;

pub use lifting::{all_lifting_sizes, is_valid_lifting_size, lifting_set_index};

/// Returns the raw per-`i_LS` shift table for a base graph.
///
/// Entries are the verbatim `V` values from `@/citation/ThreeGpp2017` Table 5.3.2-2
/// (BG1) or Table 5.3.2-3 (BG2) for the given lifting set index `i_ls`:
/// `-1` means "no connection" (zero circulant block); non-negative values
/// are the base shifts **before** the `V mod Z` reduction that
/// [`QuasiCyclicLdpc::nr_5g`] applies for a concrete lifting size.
///
/// External reference data (e.g. the committed `@/citation/Sionna2026` tables
/// under `data/ldpc/nr_5g/`) can be compared bit-exactly against it.
///
/// # Panics
///
/// Panics if `base_graph` is not 1 or 2, or if `i_ls >= 8`.
pub fn shift_table(base_graph: u8, i_ls: usize) -> Vec<Vec<i16>> {
    assert!(
        base_graph == 1 || base_graph == 2,
        "base_graph must be 1 or 2, got {base_graph}"
    );
    assert!(i_ls < 8, "i_ls must be in 0..8, got {i_ls}");
    match base_graph {
        1 => bg1::bg1_raw_shift_table(i_ls)
            .iter()
            .map(|row| row.to_vec())
            .collect(),
        2 => bg2::bg2_raw_shift_table(i_ls)
            .iter()
            .map(|row| row.to_vec())
            .collect(),
        _ => unreachable!(),
    }
}

/// Returns the lifting set index for Z, panicking if Z is invalid.
pub(crate) fn require_lifting_set_index(z: usize) -> usize {
    lifting::lifting_set_index(z as u16)
        .unwrap_or_else(|| panic!("Z={z} is not a valid 5G NR lifting size"))
}

/// Applies `V mod Z` to each entry, preserving -1 (no connection).
pub(crate) fn reduce_shifts(table: &[impl AsRef<[i16]>], z: usize) -> Vec<Vec<i32>> {
    table
        .iter()
        .map(|row| {
            row.as_ref()
                .iter()
                .map(|&v| {
                    if v < 0 {
                        -1i32
                    } else {
                        (v as i32) % (z as i32)
                    }
                })
                .collect()
        })
        .collect()
}

/// Returns the `K_b` value used for lifting-size selection per
/// `@/citation/ThreeGpp2017` Section 5.2.2.
///
/// For BG1, `K_b = 22` always. For BG2, `K_b` depends on the input block size
/// `B` (= `target_k` here): `10` for `B > 640`, `9` for `560 < B <= 640`,
/// `8` for `192 < B <= 560`, and `6` for `B <= 192`. The standard selects a
/// larger lifting size (and thus a larger mother code) for small information
/// blocks.
///
/// # Panics
///
/// Panics if `base_graph` is not 1 or 2.
pub fn kb_for_z_selection(base_graph: u8, target_k: usize) -> usize {
    assert!(
        base_graph == 1 || base_graph == 2,
        "base_graph must be 1 or 2, got {base_graph}"
    );
    match base_graph {
        1 => bg1::BG1_KB,
        2 => {
            if target_k > 640 {
                10
            } else if target_k > 560 {
                9
            } else if target_k > 192 {
                8
            } else {
                6
            }
        }
        _ => unreachable!(),
    }
}

/// Returns the largest message length `target_k` for which
/// [`QuasiCyclicLdpc::nr_5g_rate_matched`] selects **exactly** the lifting
/// size `z`.
///
/// The `@/citation/ThreeGpp2017` §5.2.2 Z-selection picks the smallest valid `Z` with
/// `K_b' * Z >= target_k`, where `K_b' = `[`kb_for_z_selection`]`(bg, target_k)`
/// depends on `target_k` itself for BG2. Consequently the naive full payload
/// `K_b * Z` does **not** always select `Z` back (e.g. BG2 with `Z = 52`:
/// `target_k = 520 <= 640` gives `K_b' = 9`, whose smallest satisfying lifting
/// size is 72, not 52). This function resolves that fixed point: it returns
/// `K_b' * z` for the largest `K_b'` candidate that is self-consistent — i.e.
/// `kb_for_z_selection(bg, K_b' * z) == K_b'` — so a rate-matched code built
/// with this payload uses exactly the requested `z`. A self-consistent
/// candidate exists for every valid lifting size of both base graphs (BG1 is
/// trivially `22 * z`; BG2 resolves to one of `{10, 9, 8, 6} * z`).
///
/// The exact-`z` guarantee additionally requires `target_n` to satisfy the
/// Z-selection's transmission-budget criterion
/// `target_k + (N_b - K_b - 2) * z >= target_n` (else a larger lifting size is
/// selected). Every code rate `>= 1/3` satisfies it for both base graphs, so
/// the guarantee holds for those rates.
///
/// # Panics
///
/// Panics if `base_graph` is not 1 or 2, or if `z` is not a valid 5G NR
/// lifting size.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::max_payload_for_lifting;
/// use gf2_coding::ldpc::QuasiCyclicLdpc;
///
/// // BG1: always the full 22 * Z systematic payload.
/// assert_eq!(max_payload_for_lifting(1, 384), 22 * 384);
///
/// // BG2, Z = 52: the full 10 * 52 = 520 payload would select Z = 72
/// // (kb_for_z = 9 for 520 <= 640); the largest self-consistent payload
/// // is 8 * 52 = 416.
/// assert_eq!(max_payload_for_lifting(2, 52), 416);
/// let rm = QuasiCyclicLdpc::nr_5g_rate_matched(2, 832, 416);
/// assert_eq!(rm.params().lifting_factor, 52);
/// ```
pub fn max_payload_for_lifting(base_graph: u8, z: usize) -> usize {
    assert!(
        base_graph == 1 || base_graph == 2,
        "base_graph must be 1 or 2, got {base_graph}"
    );
    assert!(
        is_valid_lifting_size(z as u16),
        "Z={z} is not a valid 5G NR lifting size"
    );
    // Candidate K_b' values in descending order: BG1 has the constant 22; BG2
    // has the §5.2.2 bands {10, 9, 8, 6}. The first self-consistent candidate
    // yields the largest payload (payload is monotone in K_b').
    let candidates: &[usize] = match base_graph {
        1 => &[bg1::BG1_KB],
        2 => &[10, 9, 8, 6],
        _ => unreachable!(),
    };
    candidates
        .iter()
        .copied()
        // Self-consistency: the §5.2.2 band the payload `kb * z` lands in must
        // be the very K_b' that produced it. Then the Z-selection's constraint
        // `K_b' * Z >= target_k` is tight at Z = z (and fails for every smaller
        // valid lifting size), so the selection lands on exactly z.
        .find(|&kb| kb_for_z_selection(base_graph, kb * z) == kb)
        .map(|kb| kb * z)
        .unwrap_or_else(|| {
            // Unreachable for any valid lifting size: the BG2 bands cover
            // every Z in Table 5.3.2-1 (kb=10 for Z >= 72, kb=9 for Z = 64,
            // kb=8 for 26 <= Z <= 60, kb=6 for Z <= 24); BG1 is constant.
            panic!("no self-consistent K_b for BG{base_graph} Z={z}")
        })
}

use super::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder, QuasiCyclicLdpc};
use crate::llr::Llr;
use crate::traits::{DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::BitVec;

impl QuasiCyclicLdpc {
    /// Creates the 5G NR quasi-cyclic structure of base graph `base_graph`
    /// (1 or 2) with lifting size `lifting_factor`: each base entry V becomes
    /// a Z x Z circulant with shift `V mod Z`, or a zero block if V = -1.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `base_graph` is not 1 or 2
    /// - `lifting_factor` is not a valid 5G NR lifting size
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
    /// wraps it in an [`Nr5gRateMatchedCode`], which documents the rate
    /// matching.
    ///
    /// Z is the smallest valid lifting size with `K_b' * Z >= target_k`, for
    /// the `K_b'` of [`kb_for_z_selection`], and
    /// `target_k + (N_b - K_b - 2) * Z >= target_n` (enough transmitted bits
    /// after mandatory puncturing of the first 2*Z systematic columns).
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `base_graph` is not 1 or 2
    /// - `target_n <= target_k`
    /// - No valid lifting size exists for the given (n, k)
    /// - The mother code has insufficient bits for the target n
    ///
    /// # Complexity
    ///
    /// O(m * n * min(m, n)) for Gaussian elimination on the m × n mother
    /// parity-check matrix.
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

        // K_b for code dimension: always the maximum for the base graph.
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

        // `@/citation/ThreeGpp2017` Section 5.2.2: K_b for Z selection depends
        // on B (the input block size) for BG2.
        let kb_for_z = kb_for_z_selection(base_graph, target_k);

        // The smallest valid Z such that:
        //   (a) kb_for_z * Z >= target_k  (§5.2.2 Z selection criterion)
        //   (b) target_k + (N_b - K_b - 2) * Z >= target_n  (enough transmitted bits
        //       after mandatory 2*Z systematic puncturing)
        let all_z = all_lifting_sizes();
        let z = all_z
            .iter()
            .copied()
            .find(|&z| {
                let z_us = z as usize;
                kb_for_z * z_us >= target_k && target_k + (nb - kb - 2) * z_us >= target_n
            })
            .unwrap_or_else(|| {
                panic!(
                    "No valid lifting size for BG{} with (n={}, k={}): \
                     kb_for_z={}, max possible k = {} * {} = {}",
                    base_graph,
                    target_n,
                    target_k,
                    kb_for_z,
                    kb_for_z,
                    all_z.last().unwrap(),
                    kb_for_z as u16 * all_z.last().unwrap()
                )
            });
        let z = z as usize;

        let full_k = kb * z;
        let full_n = nb * z;

        let num_filler = full_k - target_k;

        // Mandatory systematic puncturing — first 2*Z columns
        let num_punct_sys = 2 * z;

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

        let qc = Self::nr_5g(base_graph, z);
        let mother_code = LdpcCode::from_quasi_cyclic(&qc);

        let encoding = compute_mother_encoding(&mother_code, &params);

        Nr5gRateMatchedCode {
            mother_code,
            encoding,
            params,
        }
    }
}

/// Parameters of the `@/citation/ThreeGpp2017` rate matching applied to a 5G NR LDPC code.
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
    /// These are the first 2*Z columns, per `@/citation/ThreeGpp2017` Section 5.3.2.
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
    pub fn effective_rate(&self) -> f64 {
        self.target_k as f64 / self.target_n as f64
    }

    /// Returns the number of active (non-shortened, non-punctured) systematic
    /// columns retained in the rate-matched code.
    pub fn active_systematic_bits(&self) -> usize {
        self.full_k - self.num_shortened - self.num_punctured_systematic
    }

    /// Returns the number of transmitted parity bits.
    pub fn transmitted_parity_bits(&self) -> usize {
        self.target_n - self.active_systematic_bits()
    }
}

/// Filler-position prior; [`Nr5gRateMatchedCode::prepare_llrs`] documents the
/// value and why it is finite.
const FILLER_LLR: f32 = 15.0;

/// Encoding data for the mother code with right-pivot column mapping.
///
/// Contains the parity matrix and the systematic/parity column indices
/// computed via RREF with right-to-left pivoting on the parity-check matrix H.
#[derive(Clone)]
struct MotherEncoding {
    /// Parity matrix P (k × m): for systematic codes, G = [I at sys_cols | P at par_cols]
    parity_matrix: gf2_core::BitMatrix,
    /// Sorted systematic (information) column indices in the codeword
    systematic_cols: Vec<usize>,
    /// Sorted parity column indices in the codeword
    parity_cols: Vec<usize>,
    /// Ordered list of full-codeword column indices that are transmitted after
    /// rate matching. Length = target_n. Respects the RREF column assignment so
    /// that punctured, filler, and truncated positions are correctly excluded.
    transmitted_cols: Vec<usize>,
    /// Set of filler (shortened) column positions in the full codeword.
    filler_col_set: std::collections::HashSet<usize>,
}

/// Computes the encoding data for an LDPC code using RREF from the right.
///
/// Performs Gaussian elimination with right-to-left pivoting to identify
/// m pivot columns (parity positions) and k non-pivot columns (systematic
/// positions). Then computes the parity matrix P such that for each
/// systematic basis vector e_i, the codeword places 1 at systematic_cols[i]
/// and the corresponding parity bits at parity_cols.
///
/// Also computes the `transmitted_cols` list — the ordered set of full-codeword
/// column indices that survive rate matching. This accounts for the fact that
/// the RREF may assign some columns in the natural systematic range (0..K_b*Z)
/// as parity pivots and vice-versa, which affects which columns carry filler
/// zeros and which carry actual information.
///
/// # Panics
///
/// Panics if H is not full rank.
///
/// # Complexity
///
/// O(m * n * min(m, n)) for Gaussian elimination.
fn compute_mother_encoding(code: &LdpcCode, params: &NrRateMatchParams) -> MotherEncoding {
    use gf2_core::BitMatrix;

    let n = code.n();
    let m = code.m();
    let k = n - m;
    let h = code.parity_check_matrix();

    let mut work = BitMatrix::zeros(m, n);
    for row in 0..m {
        for col in h.row_iter(row) {
            work.set(row, col, true);
        }
    }

    // RREF with parity-column preference: find pivots preferring columns >= full_k
    // (the natural parity region) over columns < full_k (natural systematic region).
    //
    // This ensures systematic_cols == [0, 1, ..., full_k-1] whenever the H matrix
    // structure allows it. For 5G NR BG2, row 41's expanded block only has entries
    // in systematic columns (base cols 6, 7), forcing Z pivot columns into the
    // systematic range. The two-pass approach minimizes such "swapped" columns.
    let full_k = params.full_k;
    let mut pivot_cols = Vec::with_capacity(m);
    let mut current_row = 0;

    for col in (full_k..n).rev() {
        if current_row >= m {
            break;
        }

        let mut pivot_row = None;
        for row in current_row..m {
            if work.get(row, col) {
                pivot_row = Some(row);
                break;
            }
        }

        let Some(pr) = pivot_row else {
            continue;
        };

        if pr != current_row {
            work.swap_rows(pr, current_row);
        }

        for row in 0..m {
            if row != current_row && work.get(row, col) {
                work.row_xor(row, current_row);
            }
        }

        pivot_cols.push(col);
        current_row += 1;
    }

    // Systematic columns, right to left, for the pivots the parity columns
    // did not supply (e.g., BG2 row 41, which only touches systematic
    // columns). Right-to-left prefers higher-numbered systematic columns as
    // parity pivots, which are closer to the filler/shortened region.
    for col in (0..full_k).rev() {
        if current_row >= m {
            break;
        }

        let mut pivot_row = None;
        for row in current_row..m {
            if work.get(row, col) {
                pivot_row = Some(row);
                break;
            }
        }

        let Some(pr) = pivot_row else {
            continue;
        };

        if pr != current_row {
            work.swap_rows(pr, current_row);
        }

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

    pivot_cols.sort_unstable();

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

    // P[i, j] = work[row_for_pivot[j], systematic_cols[i]]
    let mut parity_matrix = BitMatrix::zeros(k, m);
    for (i, &sys_col) in systematic_cols.iter().enumerate() {
        for (j, &pivot_row) in row_for_pivot.iter().enumerate() {
            if work.get(pivot_row, sys_col) {
                parity_matrix.set(i, j, true);
            }
        }
    }

    // Compute filler column positions: systematic_cols[target_k..full_k] are
    // known-zero (filler) bits that should not be transmitted.
    let filler_col_set: std::collections::HashSet<usize> = (params.target_k..params.full_k)
        .map(|i| systematic_cols[i])
        .collect();

    // Compute the set of parity columns to keep. We keep the first parity_kept
    // parity columns (sorted ascending) and exclude the rest.
    let parity_kept_set: std::collections::HashSet<usize> = pivot_cols
        .iter()
        .take(params.parity_kept)
        .copied()
        .collect();

    // Build the transmitted column list: walk through all columns 0..full_n
    // in natural order, keeping those that are:
    //   (a) NOT in the first 2*Z columns (always punctured)
    //   (b) NOT a filler position
    //   (c) NOT a truncated parity column (parity col not in parity_kept_set)
    let punctured_end = params.num_punctured_systematic; // = 2*Z
    let mut transmitted_cols = Vec::with_capacity(params.target_n);
    for col in 0..params.full_n {
        if col < punctured_end {
            continue;
        }
        if filler_col_set.contains(&col) {
            continue;
        }
        if pivot_set.contains(&col) && !parity_kept_set.contains(&col) {
            continue;
        }
        transmitted_cols.push(col);
    }

    debug_assert_eq!(
        transmitted_cols.len(),
        params.target_n,
        "transmitted_cols length {} != target_n {}",
        transmitted_cols.len(),
        params.target_n
    );

    MotherEncoding {
        parity_matrix,
        systematic_cols,
        parity_cols: pivot_cols,
        transmitted_cols,
        filler_col_set,
    }
}

/// A 5G NR LDPC code with `@/citation/ThreeGpp2017` rate matching.
///
/// Wraps the full mother code and handles rate matching through LLR
/// initialization rather than H column removal. This preserves the full
/// Tanner graph for proper BP convergence on both BG1 and BG2 codes.
///
/// # Encoding
///
/// 1. Pad the target_k message with filler zeros to reach full_k = K_b * Z.
/// 2. Encode with the full mother code to get full_n = N_b * Z coded bits.
/// 3. Extract target_n transmitted bits using `transmitted_cols` (the
///    precomputed list of non-punctured, non-filler, non-truncated columns).
///
/// # Decoding
///
/// 1. Receive target_n channel LLRs.
/// 2. Map to full_n LLR vector using `transmitted_cols`:
///    - Transmitted positions: channel LLR
///    - Filler positions: the finite prior of [`Self::prepare_llrs`] (known zero)
///    - All other positions: LLR = 0 (no channel info)
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
    pub fn n(&self) -> usize {
        self.params.target_n
    }

    /// Returns the target message length.
    pub fn k(&self) -> usize {
        self.params.target_k
    }

    /// Returns a reference to the rate matching parameters.
    pub fn params(&self) -> &NrRateMatchParams {
        &self.params
    }

    /// Returns a reference to the full mother code.
    ///
    /// The mother code has n = N_b * Z columns and m = M_b * Z rows.
    /// BP decoding operates on this full code.
    pub fn mother_code(&self) -> &LdpcCode {
        &self.mother_code
    }

    /// Encodes a target_k message into a target_n transmitted codeword: pads
    /// with filler zeros to full_k, encodes with the mother code through the
    /// RREF-derived column mapping, and extracts the transmitted bits.
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != target_k`.
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

        let mut padded = BitVec::zeros(p.full_k);
        for i in 0..p.target_k {
            padded.set(i, message.get(i));
        }

        let parity = enc.parity_matrix.matvec_transpose(&padded);

        let mut codeword = BitVec::zeros(p.full_n);
        for (i, &col) in enc.systematic_cols.iter().enumerate() {
            codeword.set(col, padded.get(i));
        }
        for (j, &col) in enc.parity_cols.iter().enumerate() {
            codeword.set(col, parity.get(j));
        }

        // The precomputed column list covers the case where RREF assigns some
        // natural "systematic" columns as parity pivots (e.g., BG2 row 41).
        let mut output = BitVec::with_capacity(p.target_n);
        for &col in &enc.transmitted_cols {
            output.push_bit(codeword.get(col));
        }

        debug_assert_eq!(output.len(), p.target_n);
        output
    }

    /// Constructs the full-length LLR vector from target_n channel LLRs.
    ///
    /// Maps channel LLRs back to the full mother code positions using the
    /// precomputed `transmitted_cols` list. This ensures consistency with
    /// the encoder's column assignment, even when the RREF places some
    /// natural-systematic columns as parity pivots (e.g., BG2 row 41).
    ///
    /// - Transmitted positions (from `transmitted_cols`): channel LLRs
    /// - Filler positions: LLR = 15.0 (known to be zero)
    /// - Punctured & untransmitted positions: LLR = 0 (no info)
    ///
    /// The filler prior is finite because `tanh(15.0 / 2)` stays below 1 in
    /// `f32`, whereas the exact prior $+\infty$ gives `tanh` = 1, the argument
    /// at which the sum-product check-node `atanh` diverges.
    ///
    /// # Panics
    ///
    /// Panics if `channel_llrs.len() != target_n`.
    pub fn prepare_llrs(&self, channel_llrs: &[Llr]) -> Vec<Llr> {
        assert_eq!(
            channel_llrs.len(),
            self.params.target_n,
            "Channel LLR length {} must equal target_n = {}",
            channel_llrs.len(),
            self.params.target_n
        );

        let p = &self.params;
        let enc = &self.encoding;
        let mut full_llrs = vec![Llr::zero(); p.full_n];

        for (ch_idx, &col) in enc.transmitted_cols.iter().enumerate() {
            full_llrs[col] = channel_llrs[ch_idx];
        }

        // Filler positions are systematic_cols[target_k..full_k]; none is in
        // transmitted_cols, so nothing is overwritten.
        for &col in &enc.filler_col_set {
            full_llrs[col] = Llr::new(FILLER_LLR);
        }

        full_llrs
    }

    /// Extracts target_k message bits from a full decoded codeword.
    ///
    /// The decoded codeword has full_n bits indexed by H column position.
    /// Message bit `i` is at position `systematic_cols[i]` in the codeword.
    /// This method extracts the first target_k message bits, discarding filler.
    ///
    /// # Panics
    ///
    /// Panics if `decoded_codeword.len() < full_n` (the codeword is shorter than
    /// the full mother code length, so systematic column indices may be out of bounds).
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

/// Default normalized min-sum scaling factor for check-to-variable messages.
const DEFAULT_NMS_SCALE: f32 = 0.75;

/// Iterative BP decoder for rate-matched 5G NR LDPC codes.
///
/// Wraps an [`LdpcDecoder`] operating on the full mother code, with
/// rate-matching preprocessing (LLR mapping) and postprocessing (message
/// extraction).
pub struct Nr5gRateMatchedDecoder {
    /// The rate-matched code (owns mother code, encoder, params).
    rm_code: Nr5gRateMatchedCode,
    /// Inner LDPC decoder operating on the full mother code.
    inner: LdpcDecoder,
}

impl Nr5gRateMatchedDecoder {
    /// Creates a new rate-matched decoder with default normalized min-sum (α=0.75).
    ///
    /// # Complexity
    ///
    /// O(nnz(H)) for building the inner decoder's Tanner graph adjacency.
    pub fn new(rm_code: Nr5gRateMatchedCode) -> Self {
        Self::with_scale(rm_code, DEFAULT_NMS_SCALE)
    }

    /// Creates a decoder with a custom min-sum scaling factor; `scale = 1.0`
    /// selects plain min-sum.
    ///
    /// # Panics
    ///
    /// Panics if `scale` is not finite or lies outside `(0.0, 1.0]`.
    pub fn with_scale(rm_code: Nr5gRateMatchedCode, scale: f32) -> Self {
        let algorithm = if (scale - 1.0).abs() < f32::EPSILON {
            DecoderAlgorithm::MinSum
        } else {
            DecoderAlgorithm::NormalizedMinSum(scale)
        };
        Self::with_algorithm(rm_code, algorithm)
    }

    /// Creates a decoder with a specific BP algorithm variant.
    ///
    /// # Panics
    ///
    /// Panics if `algorithm` carries a parameter [`DecoderConfig::new`] rejects.
    pub fn with_algorithm(rm_code: Nr5gRateMatchedCode, algorithm: DecoderAlgorithm) -> Self {
        let config = DecoderConfig::new(algorithm, true);
        let inner = LdpcDecoder::with_config(rm_code.mother_code.clone(), config);
        Self { rm_code, inner }
    }

    /// Returns the target codeword length.
    pub fn n(&self) -> usize {
        self.rm_code.n()
    }

    /// Returns the target message length.
    pub fn k(&self) -> usize {
        self.rm_code.k()
    }

    /// Returns a reference to the rate matching parameters.
    pub fn params(&self) -> &NrRateMatchParams {
        self.rm_code.params()
    }

    /// Returns a reference to the underlying rate-matched code.
    pub fn code(&self) -> &Nr5gRateMatchedCode {
        &self.rm_code
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

        let full_llrs = self.rm_code.prepare_llrs(llrs);

        // decode_to_codeword returns all N bits: the message extraction uses
        // natural column ordering (positions 0..target_k), not the
        // RREF-determined systematic positions of LdpcDecoder::decode_iterative.
        let mother_result = self.inner.decode_to_codeword(&full_llrs, max_iterations);

        let target_k = self.rm_code.params.target_k;
        let mut message = BitVec::with_capacity(target_k);
        for i in 0..target_k {
            message.push_bit(mother_result.decoded_bits.get(i));
        }

        DecoderResult::new(
            message,
            mother_result.iterations,
            mother_result.converged,
            mother_result.syndrome_check_passed,
        )
    }

    fn last_iteration_count(&self) -> usize {
        self.inner.last_iteration_count()
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::LdpcCode;

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
        for z in [2u16, 52, 384] {
            let matrix = bg2::bg2_base_matrix(z as usize);
            // Extension rows 4..39 should have at least one parity entry
            for (r, row) in matrix.iter().enumerate().take(40).skip(4) {
                let has_parity = row[bg2::BG2_KB..].iter().any(|&v| v >= 0);
                assert!(
                    has_parity,
                    "BG2 Z={z} extension row {r} missing parity entry"
                );
            }
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

    #[test]
    fn test_rate_matched_bg2_256_121_exact_dimensions() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 256, "n mismatch");
        assert_eq!(rm_code.k(), 121, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 22);
        assert_eq!(params.full_k, 220); // 10 * 22
        assert_eq!(params.full_n, 1144); // 52 * 22
        assert_eq!(params.num_shortened, 99); // 220 - 121
        assert_eq!(params.num_punctured_systematic, 44); // 2 * 22
        assert_eq!(params.num_punctured_parity, 745); // 924 - 179
        assert_eq!(params.target_n, 256);
        assert_eq!(params.target_k, 121);
        assert_eq!(params.active_systematic_bits(), 77); // 220 - 99 - 44
        assert_eq!(params.transmitted_parity_bits(), 179);
    }

    #[test]
    fn test_rate_matched_bg2_256_49_exact_dimensions() {
        // By the §5.2.2 K_b rules, target_k=49 <= 192 so K_b_for_z=6,
        // smallest Z with 6*Z >= 49 is Z=9.
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 256, "n mismatch");
        assert_eq!(rm_code.k(), 49, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 9); // Z=9 via K_b_for_z=6
        assert_eq!(params.full_k, 90); // 10 * 9
        assert_eq!(params.full_n, 468); // 52 * 9
        assert_eq!(params.num_shortened, 41); // 90 - 49
        assert_eq!(params.num_punctured_systematic, 18); // 2 * 9
        assert_eq!(params.active_systematic_bits(), 31); // 90 - 41 - 18
        assert_eq!(params.transmitted_parity_bits(), 225);
    }

    #[test]
    fn test_rate_matched_bg2_625_225_exact_dimensions() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 625, 225);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 625, "n mismatch");
        assert_eq!(rm_code.k(), 225, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 30);
        assert_eq!(params.full_k, 300);
        assert_eq!(params.num_shortened, 75);
        assert_eq!(params.num_punctured_systematic, 60); // 2 * 30
        assert_eq!(params.active_systematic_bits(), 165); // 300 - 75 - 60
        assert_eq!(params.transmitted_parity_bits(), 460);
    }

    #[test]
    fn test_rate_matched_bg2_1024_441_exact_dimensions() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
        let params = rm_code.params();
        assert_eq!(rm_code.n(), 1024, "n mismatch");
        assert_eq!(rm_code.k(), 441, "k mismatch");
        assert_eq!(params.base_graph, 2);
        assert_eq!(params.lifting_factor, 56);
        assert_eq!(params.full_k, 560);
        assert_eq!(params.num_shortened, 119);
        assert_eq!(params.num_punctured_systematic, 112); // 2 * 56
        assert_eq!(params.active_systematic_bits(), 329); // 560 - 119 - 112
        assert_eq!(params.transmitted_parity_bits(), 695);
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

    #[test]
    fn test_kb_for_z_selection_bands() {
        // BG1: constant 22 regardless of block size.
        for k in [1usize, 192, 193, 560, 561, 640, 641, 8448] {
            assert_eq!(kb_for_z_selection(1, k), 22, "BG1 K_b' at k={k}");
        }
        // BG2: the §5.2.2 bands with exact boundary semantics (>, not >=).
        assert_eq!(kb_for_z_selection(2, 641), 10);
        assert_eq!(kb_for_z_selection(2, 640), 9);
        assert_eq!(kb_for_z_selection(2, 561), 9);
        assert_eq!(kb_for_z_selection(2, 560), 8);
        assert_eq!(kb_for_z_selection(2, 193), 8);
        assert_eq!(kb_for_z_selection(2, 192), 6);
        assert_eq!(kb_for_z_selection(2, 1), 6);
    }

    #[test]
    fn test_max_payload_self_consistent_for_all_z_both_bgs() {
        // For every valid lifting size and both base graphs, the returned
        // payload's own K_b' band must reproduce the K_b' that built it (the
        // fixed point that makes the Z-selection land on exactly z).
        for bg in [1u8, 2] {
            for z in all_lifting_sizes() {
                let z = z as usize;
                let payload = max_payload_for_lifting(bg, z);
                let kb = payload / z;
                assert_eq!(payload % z, 0, "BG{bg} Z={z}: payload {payload} not kb*z");
                assert_eq!(
                    kb_for_z_selection(bg, payload),
                    kb,
                    "BG{bg} Z={z}: payload {payload} is not self-consistent"
                );
            }
        }
    }

    /// Small Z keeps construction fast-tier; the all-Z sweep is the slow-tier
    /// test below.
    #[test]
    fn test_max_payload_realizes_exact_z_representatives() {
        let cases: &[(u8, usize)] = &[
            (1, 2),
            (1, 16),
            (1, 52),
            (2, 2),
            (2, 15),
            (2, 24),  // kb=6 upper boundary
            (2, 26),  // kb=8 lower boundary
            (2, 52),  // the band-mismatch example from the docs
            (2, 60),  // kb=8 upper boundary
            (2, 64),  // the sole kb=9 size
            (2, 72),  // kb=10 lower boundary
            (2, 104), // kb=10 interior
        ];
        for &(bg, z) in cases {
            let k = max_payload_for_lifting(bg, z);
            let rm = QuasiCyclicLdpc::nr_5g_rate_matched(bg, 2 * k, k);
            assert_eq!(
                rm.params().lifting_factor,
                z,
                "BG{bg} Z={z}: rate-1/2 code with payload {k} must realize exactly Z"
            );
        }
    }

    #[test]
    #[ignore = "slow: constructs rate-matched codes for all 51 lifting sizes x both BGs"]
    fn test_max_payload_realizes_exact_z_all() {
        for bg in [1u8, 2] {
            for z in all_lifting_sizes() {
                let z = z as usize;
                let k = max_payload_for_lifting(bg, z);
                let rm = QuasiCyclicLdpc::nr_5g_rate_matched(bg, 2 * k, k);
                assert_eq!(
                    rm.params().lifting_factor,
                    z,
                    "BG{bg} Z={z}: payload {k} must realize exactly Z"
                );
            }
        }
    }

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

    #[test]
    fn test_rate_matched_effective_rate() {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let rate = rm_code.params().effective_rate();
        assert!(
            (rate - 121.0 / 256.0).abs() < 1e-6,
            "Expected rate ~0.473, got {rate}"
        );
    }

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

    #[test]
    fn test_bg2_specific_shift_values() {
        // Z=384 belongs to lifting set 1 (384 = 3 * 2^7, base factor 3)
        let matrix = bg2::bg2_base_matrix(384);
        assert_eq!(matrix[0][0], 174);
        assert_eq!(matrix[0][1], 97);
        assert_eq!(matrix[0][9], 172);

        // Also verify i_LS=0 (Z=256 = 2 * 2^7, base factor 2)
        let matrix0 = bg2::bg2_base_matrix(256);
        assert_eq!(matrix0[0][0], 9);
        assert_eq!(matrix0[0][1], 117);
    }

    #[test]
    fn test_bg1_specific_shift_values() {
        // Z=384 belongs to lifting set 1 (384 = 3 * 2^7, base factor 3)
        let matrix = bg1::bg1_base_matrix(384);
        assert_eq!(matrix[0][0], 307);
        assert_eq!(matrix[0][22], 1);
        assert_eq!(matrix[1][23], 0);
        assert_eq!(matrix[2][24], 0);
        assert_eq!(matrix[45][67], 0);

        // Also verify i_LS=0 (Z=256 = 2 * 2^7, base factor 2)
        let matrix0 = bg1::bg1_base_matrix(256);
        assert_eq!(matrix0[0][0], 250);
    }

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

    #[test]
    fn test_bg2_rows_40_41_have_entries() {
        let matrix = bg2::bg2_base_matrix(384);
        let row40_nneg: usize = matrix[40].iter().filter(|&&v| v >= 0).count();
        let row41_nneg: usize = matrix[41].iter().filter(|&&v| v >= 0).count();
        assert!(row40_nneg >= 2, "Row 40 should have >= 2 non-neg entries");
        assert!(row41_nneg >= 2, "Row 41 should have >= 2 non-neg entries");
    }

    #[test]
    fn test_rate_matched_params_consistency() {
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

    fn ber_acceptance(
        bg: u8,
        target_n: usize,
        target_k: usize,
        eb_n0_db: f64,
        max_ber: f64,
        num_frames: usize,
        label: &str,
    ) {
        use crate::simulation::{BpskAwgnChannel, ChannelModel};
        use crate::traits::BlockEncoder;
        use rand::Rng;

        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);
        let mut decoder = Nr5gRateMatchedDecoder::new(QuasiCyclicLdpc::nr_5g_rate_matched(
            bg, target_n, target_k,
        ));

        let rate = rm_code.params().effective_rate();
        let channel = BpskAwgnChannel;
        let mut rng = rand::thread_rng();

        let mut total_bits = 0usize;
        let mut bit_errors = 0usize;
        let mut frame_errors = 0usize;
        let mut frames_decoded = 0usize;

        for _ in 0..num_frames {
            let mut msg = BitVec::zeros(target_k);
            for i in 0..target_k {
                if rng.gen_bool(0.5) {
                    msg.set(i, true);
                }
            }

            let codeword = rm_code.encode(&msg);
            assert_eq!(codeword.len(), target_n);

            let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);

            let result = decoder.decode_iterative(&llrs, 50);

            if result.converged {
                frames_decoded += 1;
            }

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
        assert!(
            bler < 1.0,
            "{label}: BLER = 1.0 — no frame decoded correctly at {eb_n0_db} dB"
        );
    }

    #[test]
    fn test_ber_bg2_256_121_6db() {
        ber_acceptance(2, 256, 121, 6.0, 1e-3, 20, "BG2 (256,121) @ 6dB");
    }

    #[test]
    fn test_ber_bg2_256_49_6db() {
        ber_acceptance(2, 256, 49, 6.0, 1e-3, 20, "BG2 (256,49) @ 6dB");
    }

    #[test]
    fn test_ber_bg2_1024_441_6db() {
        ber_acceptance(2, 1024, 441, 6.0, 1e-3, 10, "BG2 (1024,441) @ 6dB");
    }

    #[test]
    fn test_ber_bg2_625_225_6db() {
        ber_acceptance(2, 625, 225, 6.0, 1e-3, 10, "BG2 (625,225) @ 6dB");
    }

    #[test]
    fn test_ber_bg1_1024_640_8db() {
        ber_acceptance(1, 1024, 640, 8.0, 1e-3, 10, "BG1 (1024,640) @ 8dB");
    }

    #[test]
    #[ignore = "slow: BER simulation BG1(4096,3249) at 8dB; large code exceeds 5s"]
    fn test_ber_bg1_4096_3249_8db() {
        ber_acceptance(1, 4096, 3249, 8.0, 1e-2, 5, "BG1 (4096,3249) @ 8dB");
    }

    #[test]
    fn test_soft_decoder_trait_roundtrip() {
        use crate::traits::{BlockEncoder, SoftDecoder};

        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let msg = gf2_core::BitVec::zeros(121);
        let cw = rm_code.encode(&msg);

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
        #[ignore = "slow: proptest NR 5G LDPC encoding; BG1(1024,640) construction dominates"]
        fn prop_encode_produces_correct_length(bg in 1u8..=2, seed in 0u64..100) {
            // Use a fixed target per BG to keep the test fast
            let (target_n, target_k) = if bg == 1 { (1024, 640) } else { (256, 121) };
            let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);

            use rand::rngs::StdRng;
            use rand::SeedableRng;
            let mut rng = StdRng::seed_from_u64(seed);
            let msg = gf2_core::BitVec::random(target_k, &mut rng);

            let cw = rm_code.encode(&msg);
            prop_assert_eq!(cw.len(), target_n, "codeword length must be target_n");
        }

        #[test]
        #[ignore = "slow: proptest NR 5G LDPC LLR prep; BG1(1024,640) construction dominates"]
        fn prop_prepare_llrs_correct_length(bg in 1u8..=2) {
            let (target_n, target_k) = if bg == 1 { (1024, 640) } else { (256, 121) };
            let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);

            let channel_llrs: Vec<Llr> = vec![Llr::new(1.0); target_n];
            let full_llrs = rm_code.prepare_llrs(&channel_llrs);
            prop_assert_eq!(full_llrs.len(), rm_code.mother_code().n());
        }
    }
}
