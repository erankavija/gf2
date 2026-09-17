//! Deterministic untimed correctness oracle for every measured route.
//!
//! The oracle recomputes each row parity from the public `get` accessors of the
//! matrix and the vector, so it shares no code with the measured route. It
//! checks every output bit, the output length, canonical LSB-first bit
//! indexing, zero tail padding in the output `BitVec`, and immutability of the
//! matrix and the vector, over the frozen logical boundaries 0, 1, 63, 64 and
//! 65, all seven word counts and both column shapes. It emits no timing sample
//! and cannot serve as a pilot.

use crate::cells::{
    Cache, MatvecShape, ALL_WORDS, ANCHOR_WORDS, BOUNDARY_BITS, CAMPAIGN_SEED, MATVEC_ROWS,
};
use crate::fixture::MatvecBanks;
use crate::routes::{fused_and_popcnt, fused_bundle, public_matvec, verify_lane, verify_shape, Route};
use gf2_core::{BitMatrix, BitVec};

/// Rows of the oracle's word-count matrices; 65 exercises a partial output word.
const ORACLE_ROWS: usize = 65;

/// One oracle case and the number of checks it performed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleCase {
    /// Case name, as printed in the validation record.
    pub name: String,
    /// Checks the case performed.
    pub checks: usize,
}

impl std::fmt::Display for OracleCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "PASS {}: {} checks", self.name, self.checks)
    }
}

fn fail(case: &str, detail: impl std::fmt::Display) -> String {
    format!("FAIL {case}: {detail}")
}

/// Row parity recomputed bit by bit through the public accessors.
fn oracle_parity(matrix: &BitMatrix, vector: &BitVec, row: usize) -> bool {
    (0..matrix.cols()).fold(false, |parity, column| {
        parity ^ (matrix.get(row, column) && vector.get(column))
    })
}

/// Checks one constructed shape end to end and returns the checks performed.
fn check_product(name: &str, banks: &MatvecBanks) -> Result<usize, String> {
    let item = banks.item(0, 0);
    let (rows, columns) = (banks.rows(), banks.columns());
    let facts = verify_shape(&item.matrix, &item.vector, rows, columns)
        .map_err(|error| fail(name, error))?;
    let mut checks = 1;

    let output = public_matvec(&item.matrix, &item.vector);
    if output.len() != rows {
        return Err(fail(name, format!("output length {} is not {rows}", output.len())));
    }
    checks += 1;

    for row in 0..rows {
        if output.get(row) != oracle_parity(&item.matrix, &item.vector, row) {
            return Err(fail(name, format!("row {row} differs from the parity oracle")));
        }
        checks += 1;
    }

    let words = output.words();
    if words.len() != rows.div_ceil(64) {
        return Err(fail(name, "the output word count is not the logical length"));
    }
    checks += 1;
    for index in 0..rows {
        if output.get(index) != ((words[index >> 6] >> (index & 63)) & 1 == 1) {
            return Err(fail(name, format!("output bit {index} is not canonical LSB-first")));
        }
        checks += 1;
    }
    let tail = rows % 64;
    if tail != 0 && words[words.len() - 1] >> tail != 0 {
        return Err(fail(name, "the output carries tail padding"));
    }
    checks += 1;

    // Immutability: the timed call reads the fixture only, so a rebuild at the
    // same seed reproduces every input word.
    if item.matrix.stride_words() != facts.stride_words {
        return Err(fail(name, "the matrix stride changed"));
    }
    checks += 1;
    Ok(checks)
}

fn seeded_banks(rows: usize, columns: usize, salt: u64) -> MatvecBanks {
    MatvecBanks::build(rows, columns, Cache::Warm, CAMPAIGN_SEED ^ salt)
}

/// Checks the frozen logical boundaries on both the columns and the output.
fn boundary_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    for columns in BOUNDARY_BITS {
        for rows in BOUNDARY_BITS {
            let name = format!("matvec-cols-{columns}-rows-{rows}");
            let salt = ((columns as u64) << 32) ^ rows as u64;
            let banks = seeded_banks(rows, columns, salt);
            let mut checks = check_product(&name, &banks)?;

            let item = banks.item(0, 0);
            let rebuilt = seeded_banks(rows, columns, salt);
            let fresh = rebuilt.item(0, 0);
            for row in 0..rows {
                if item.matrix.row_words(row) != fresh.matrix.row_words(row) {
                    return Err(fail(&name, format!("row {row} of the matrix changed")));
                }
                checks += 1;
            }
            if item.vector.words() != fresh.vector.words() {
                return Err(fail(&name, "the vector changed"));
            }
            checks += 1;
            report.push(OracleCase { name, checks });
        }
    }
    Ok(())
}

/// Checks every word count in both column shapes.
fn shape_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    for words in ALL_WORDS {
        for shape in [MatvecShape::Full, MatvecShape::Tail1] {
            let columns = shape.columns(words);
            let name = format!("matvec-{words}w-{}", shape.id());
            let banks = seeded_banks(ORACLE_ROWS, columns, (words as u64) << 8);
            let mut checks = check_product(&name, &banks)?;

            // A `tail1` fixture carries a zero padding bit before and after the
            // measured call, so no cell needs a reset inside a timed window.
            let item = banks.item(0, 0);
            let tail = columns % 64;
            if tail != 0 {
                for row in 0..ORACLE_ROWS {
                    let row_words = item.matrix.row_words(row);
                    if row_words[row_words.len() - 1] >> tail != 0 {
                        return Err(fail(&name, format!("row {row} carries tail padding")));
                    }
                    checks += 1;
                }
                let vector_words = item.vector.words();
                if vector_words[vector_words.len() - 1] >> tail != 0 {
                    return Err(fail(&name, "the vector carries tail padding"));
                }
                checks += 1;
            }
            report.push(OracleCase { name, checks });
        }
    }
    Ok(())
}

/// Checks the frozen allocated geometry and the lane it resolves.
fn anchor_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    let route = if cfg!(feature = "simd") { Route::MatvecA } else { Route::MatvecScalarReference };
    for words in ANCHOR_WORDS {
        let columns = MatvecShape::Full.columns(words);
        let name = format!("matvec-anchor-{words}w");
        let banks = MatvecBanks::allocated(words, MatvecShape::Full, Cache::Warm, CAMPAIGN_SEED);
        let item = banks.item(0, 0);
        let facts = verify_shape(&item.matrix, &item.vector, MATVEC_ROWS, columns)
            .map_err(|error| fail(&name, error))?;
        verify_lane(facts, route).map_err(|error| fail(&name, error))?;
        let output = public_matvec(&item.matrix, &item.vector);
        if output.len() != MATVEC_ROWS {
            return Err(fail(&name, "the output length is not the frozen row count"));
        }
        for row in 0..MATVEC_ROWS {
            if output.get(row) != oracle_parity(&item.matrix, &item.vector, row) {
                return Err(fail(&name, format!("row {row} differs from the parity oracle")));
            }
        }
        report.push(OracleCase { name, checks: MATVEC_ROWS + 3 });
    }
    Ok(())
}

/// Checks the isolated bundle entry against an independent count.
fn isolated_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    let bundle = fused_bundle()
        .ok_or_else(|| fail("and-popcnt", "the host detected no kernel bundle"))?;
    for words in ALL_WORDS {
        let name = format!("and-popcnt-{words}w");
        let columns = 64 * words;
        let banks = seeded_banks(1, columns, (words as u64) << 16);
        let item = banks.item(0, 0);
        let row = item.matrix.row_words(0);
        let vector = item.vector.words();
        let observed = fused_and_popcnt(&bundle, row, vector);
        let expected: u64 = row
            .iter()
            .zip(vector)
            .map(|(left, right)| u64::from((left & right).count_ones()))
            .sum();
        if observed != expected {
            return Err(fail(&name, format!("count {observed} differs from {expected}")));
        }
        let parity = oracle_parity(&item.matrix, &item.vector, 0);
        if (observed & 1 == 1) != parity {
            return Err(fail(&name, "the low bit of the count is not the row parity"));
        }
        if fused_and_popcnt(&bundle, row, vector) != observed {
            return Err(fail(&name, "the entry is not a pure function of its operands"));
        }
        report.push(OracleCase { name, checks: 3 });
    }
    Ok(())
}

/// Runs every gf2-side oracle case.
pub fn run() -> Result<Vec<OracleCase>, String> {
    let mut report = Vec::new();
    boundary_cases(&mut report)?;
    shape_cases(&mut report)?;
    anchor_cases(&mut report)?;
    isolated_cases(&mut report)?;
    Ok(report)
}
