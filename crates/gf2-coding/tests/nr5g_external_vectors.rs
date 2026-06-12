//! External reference-vector validation for the 5G NR LDPC base graphs.
//!
//! Validates the compiled-in 3GPP TS 38.212 base-graph shift tables (BG1
//! Table 5.3.2-2, BG2 Table 5.3.2-3, all 8 lifting sets `i_LS` = 0..7)
//! bit-exactly against an EXTERNAL reference: the NVIDIA Sionna base-graph
//! CSV tables committed under `data/ldpc/nr_5g/` (Apache-2.0; provenance,
//! upstream commit pin, and format description in
//! `data/ldpc/nr_5g/PROVENANCE.md`). The CSV parser lives in this test —
//! the compiled-in Rust constants remain the production SSOT.
//!
//! Coverage:
//!
//! - One regression per (BG, `i_LS`) pair (16 total) asserting
//!   (a) the RAW shift table is bit-exact vs the parsed external reference,
//!   and (b) the production constructor path `QuasiCyclicLdpc::nr_5g(bg, z)`
//!   produces the external reference reduced `V mod Z` for EVERY valid Z in
//!   the set (all 51 lifting sizes are swept — stronger than a single
//!   representative Z per set, at negligible cost since `nr_5g` only builds
//!   the base matrix).
//! - The wrong-`i_LS` trap guard (the ~2 dB BLER trap,
//!   `feedback_ldpc_shift_tables`): at Z=208 (`i_LS`=6), substituting any of
//!   the 7 wrong per-set tables yields a matrix that DIFFERS from the
//!   external reference, and all 8 raw tables are pairwise distinct — so
//!   collapsing the per-`i_LS` tables into one fails loudly.
//! - Rate coverage {1/3, 1/2, 2/3, 5/6} through the public
//!   `QuasiCyclicLdpc::nr_5g_rate_matched` constructor surface, pinning the
//!   selected (Z, `i_LS`) per tuple, re-asserting the mother base matrix
//!   against the external reference, and a bit-exact noiseless
//!   encode/decode roundtrip. BG1 carries all four rates; BG2 carries
//!   {1/3, 1/2, 2/3} only — TS 38.212 clause 7.2.2 selects BG2 only for
//!   rates R <= 0.67 (5/6 on BG2 is outside the standard's operating
//!   region), so the in-scope tuples are 4 (BG1) + 3 (BG2).

use gf2_coding::ldpc::nr_5g::lifting::LIFTING_SIZE_SETS;
use gf2_coding::ldpc::nr_5g::{lifting_set_index, shift_table, Nr5gRateMatchedDecoder};
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::BitVec;
use std::path::PathBuf;

/// Parses a Sionna base-graph CSV into 8 dense per-`i_LS` matrices.
///
/// Format (see `data/ldpc/nr_5g/PROVENANCE.md`): two header lines, then one
/// semicolon-delimited line per base-graph edge carrying the row index
/// (blank = same as previous line), the column index, and the 8 shift
/// values for `i_LS` = 0..7. Entries absent from the file are -1.
fn load_reference(bg: u8) -> [Vec<Vec<i16>>; 8] {
    let (rows, cols, file, expected_edges) = match bg {
        1 => (46, 68, "5G_bg1.csv", 316),
        2 => (42, 52, "5G_bg2.csv", 197),
        _ => panic!("base graph must be 1 or 2, got {bg}"),
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/ldpc/nr_5g")
        .join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read reference file {}: {e}", path.display()));

    let mut mats: [Vec<Vec<i16>>; 8] = std::array::from_fn(|_| vec![vec![-1i16; cols]; rows]);
    let mut cur_row: Option<usize> = None;
    let mut edges = 0usize;
    for line in text.lines().skip(2) {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(';').collect();
        assert!(parts.len() >= 10, "malformed reference line: {line:?}");
        if !parts[0].trim().is_empty() {
            cur_row = Some(parts[0].trim().parse().expect("row index"));
        }
        let r = cur_row.expect("column entry before any row index");
        let c: usize = parts[1].trim().parse().expect("column index");
        assert!(r < rows && c < cols, "edge ({r},{c}) out of bounds");
        for (i_ls, mat) in mats.iter_mut().enumerate() {
            let v: i16 = parts[2 + i_ls].trim().parse().expect("shift value");
            assert!(v >= 0, "reference stores only connected edges, got {v}");
            assert_eq!(mat[r][c], -1, "duplicate edge ({r},{c})");
            mat[r][c] = v;
        }
        edges += 1;
    }
    assert_eq!(edges, expected_edges, "BG{bg} reference edge count");
    mats
}

/// Applies `V mod Z` to a raw shift table, preserving -1 (no connection).
fn reduce_mod_z(table: &[Vec<i16>], z: usize) -> Vec<Vec<i32>> {
    table
        .iter()
        .map(|row| {
            row.iter()
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

/// One regression per (BG, `i_LS`): raw table bit-exact vs the external
/// reference, plus the production constructor path at every Z in the set.
fn check_bg_ils_against_reference(bg: u8, i_ls: usize) {
    let reference = load_reference(bg);

    // (a) Raw per-i_LS shift table, bit-exact (values AND -1 pattern).
    assert_eq!(
        shift_table(bg, i_ls),
        reference[i_ls],
        "BG{bg} i_LS={i_ls}: raw shift table differs from the Sionna reference"
    );

    // (b) Production constructor path for every valid Z in this set.
    for &z in LIFTING_SIZE_SETS[i_ls] {
        let z = z as usize;
        let qc = QuasiCyclicLdpc::nr_5g(bg, z);
        let expected = reduce_mod_z(&reference[i_ls], z);
        assert_eq!(
            qc.base_matrix(),
            &expected[..],
            "BG{bg} Z={z} (i_LS={i_ls}): constructor base matrix differs from \
             the Sionna reference reduced mod Z"
        );
    }
}

// ===========================================================================
// Per-(BG, i_LS) table regressions vs the external reference
// ===========================================================================

#[test]
fn test_bg1_ils0_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 0);
}

#[test]
fn test_bg1_ils1_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 1);
}

#[test]
fn test_bg1_ils2_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 2);
}

#[test]
fn test_bg1_ils3_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 3);
}

#[test]
fn test_bg1_ils4_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 4);
}

#[test]
fn test_bg1_ils5_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 5);
}

#[test]
fn test_bg1_ils6_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 6);
}

#[test]
fn test_bg1_ils7_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 7);
}

#[test]
fn test_bg2_ils0_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 0);
}

#[test]
fn test_bg2_ils1_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 1);
}

#[test]
fn test_bg2_ils2_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 2);
}

#[test]
fn test_bg2_ils3_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 3);
}

#[test]
fn test_bg2_ils4_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 4);
}

#[test]
fn test_bg2_ils5_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 5);
}

#[test]
fn test_bg2_ils6_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 6);
}

#[test]
fn test_bg2_ils7_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 7);
}

// ===========================================================================
// Wrong-i_LS trap guard (feedback_ldpc_shift_tables: the ~2 dB BLER trap)
// ===========================================================================

/// At Z=208 (`i_LS`=6 for both BGs), substituting any WRONG per-set table
/// yields a base matrix that differs from the external reference, while the
/// correct table matches. If the 8 per-`i_LS` tables were ever collapsed
/// into one, at least 7 of these inequality assertions would see identical
/// matrices and fail loudly.
fn check_wrong_ils_guard_z208(bg: u8) {
    let z = 208usize;
    assert_eq!(lifting_set_index(z as u16), Some(6), "Z=208 must be i_LS=6");

    let reference = load_reference(bg);
    let correct = reduce_mod_z(&reference[6], z);

    // The production constructor (which derives i_LS=6 from Z=208 itself)
    // matches the external reference...
    let qc = QuasiCyclicLdpc::nr_5g(bg, z);
    assert_eq!(
        qc.base_matrix(),
        &correct[..],
        "BG{bg} Z=208: correct i_LS=6 table must match the external reference"
    );

    // ...and every wrong per-set table, reduced mod the same Z, differs.
    for wrong_ils in (0..8).filter(|&w| w != 6) {
        let wrong = reduce_mod_z(&shift_table(bg, wrong_ils), z);
        assert_ne!(
            wrong, correct,
            "BG{bg} Z=208: the WRONG i_LS={wrong_ils} table must differ from \
             the i_LS=6 external reference — per-i_LS tables may have been \
             collapsed (the ~2 dB BLER trap)"
        );
    }
}

#[test]
fn test_bg1_wrong_ils_table_for_z208_differs_from_reference() {
    check_wrong_ils_guard_z208(1);
}

#[test]
fn test_bg2_wrong_ils_table_for_z208_differs_from_reference() {
    check_wrong_ils_guard_z208(2);
}

/// All 8 raw per-`i_LS` tables are pairwise distinct for both BGs: direct
/// collapse detection independent of any particular Z.
#[test]
fn test_per_ils_tables_pairwise_distinct() {
    for bg in [1u8, 2u8] {
        for i in 0..8 {
            for j in (i + 1)..8 {
                assert_ne!(
                    shift_table(bg, i),
                    shift_table(bg, j),
                    "BG{bg}: raw shift tables for i_LS={i} and i_LS={j} must \
                     be distinct — per-i_LS tables may have been collapsed"
                );
            }
        }
    }
}

// ===========================================================================
// Rate coverage {1/3, 1/2, 2/3, 5/6} through the public constructor surface
// ===========================================================================

/// Deterministic message: bits set at positions that are multiples of 3.
fn deterministic_message(k: usize) -> BitVec {
    let mut msg = BitVec::zeros(k);
    for i in (0..k).step_by(3) {
        msg.set(i, true);
    }
    msg
}

/// One regression per in-scope (BG, rate) tuple: pins the (Z, `i_LS`) the
/// public `nr_5g_rate_matched` surface selects, re-asserts the mother base
/// matrix against the external reference at that Z, and runs a bit-exact
/// noiseless encode/decode roundtrip.
fn check_rate_tuple(bg: u8, target_n: usize, target_k: usize, expect_z: usize, expect_ils: usize) {
    let rm = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);
    let params = rm.params().clone();
    assert_eq!(params.base_graph, bg);
    assert_eq!(
        params.lifting_factor, expect_z,
        "BG{bg} (n={target_n}, k={target_k}): expected lifting size Z={expect_z}"
    );
    assert_eq!(
        lifting_set_index(expect_z as u16),
        Some(expect_ils),
        "Z={expect_z} must belong to lifting set i_LS={expect_ils}"
    );
    assert_eq!(rm.mother_code().n(), params.nb * expect_z);

    // The mother code's base matrix (the same `nr_5g` path the rate-matched
    // constructor builds on) matches the external reference at this Z.
    let reference = load_reference(bg);
    let expected = reduce_mod_z(&reference[expect_ils], expect_z);
    let qc = QuasiCyclicLdpc::nr_5g(bg, expect_z);
    assert_eq!(
        qc.base_matrix(),
        &expected[..],
        "BG{bg} Z={expect_z} (i_LS={expect_ils}): mother base matrix differs \
         from the Sionna reference reduced mod Z"
    );

    // Bit-exact noiseless roundtrip through the rate-matched surface.
    let msg = deterministic_message(target_k);
    let codeword = rm.encode(&msg);
    assert_eq!(codeword.len(), target_n);
    let llrs: Vec<Llr> = (0..target_n)
        .map(|i| Llr::new(if codeword.get(i) { -4.0 } else { 4.0 }))
        .collect();
    let mut decoder = Nr5gRateMatchedDecoder::new(rm);
    let result = decoder.decode_iterative(&llrs, 50);
    assert!(
        result.syndrome_check_passed,
        "BG{bg} (n={target_n}, k={target_k}): noiseless decode must converge"
    );
    assert_eq!(
        result.decoded_bits, msg,
        "BG{bg} (n={target_n}, k={target_k}): noiseless roundtrip must be bit-exact"
    );
}

#[test]
fn test_rate_1_3_bg1_z24_ils1() {
    check_rate_tuple(1, 1584, 528, 24, 1);
}

#[test]
fn test_rate_1_2_bg1_z20_ils2() {
    check_rate_tuple(1, 880, 440, 20, 2);
}

#[test]
fn test_rate_2_3_bg1_z28_ils3() {
    check_rate_tuple(1, 924, 616, 28, 3);
}

#[test]
fn test_rate_5_6_bg1_z30_ils7() {
    check_rate_tuple(1, 792, 660, 30, 7);
}

#[test]
fn test_rate_1_3_bg2_z26_ils6() {
    check_rate_tuple(2, 600, 200, 26, 6);
}

#[test]
fn test_rate_1_2_bg2_z44_ils5() {
    check_rate_tuple(2, 704, 352, 44, 5);
}

#[test]
fn test_rate_2_3_bg2_z48_ils1() {
    check_rate_tuple(2, 540, 360, 48, 1);
}
