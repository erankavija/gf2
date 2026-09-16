//! Deterministic untimed correctness oracle for every measured route.
//!
//! The oracle checks every output word, source immutability, canonical
//! LSB-first bit indexing, logical length and zero tail padding, over the
//! frozen bit-length boundaries, all seven word counts, both address layouts,
//! both matrix shapes and the four bidirectional row-pair groups. It emits no
//! timing sample and cannot serve as a pilot.

use crate::cells::{
    Cache, Layout, RowShape, ALL_WORDS, BOUNDARY_BITS, CAMPAIGN_SEED, NR_TARGETS, ROW_MATRIX_ROWS,
    ROW_PAIRS,
};
use crate::fixture::{RowBanks, XorBanks, SLAB_ALIGN};
use crate::routes::{nr_construct, public_row_xor, public_xor, resolved_xor, verify_nr};
use gf2_core::BitMatrix;
use tuning_campaign_support::abtest::SplitMix64;

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

/// Expands a seed into `words` canonical little-endian words, masking the
/// partial final word for a logical length of `bits`.
fn seeded(seed: u64, bits: usize) -> Vec<u64> {
    let words = bits.div_ceil(64);
    let mut mixer = SplitMix64::new(seed);
    let mut buffer: Vec<u64> = (0..words).map(|_| mixer.next_u64()).collect();
    let tail = bits % 64;
    if tail != 0 {
        buffer[words - 1] &= (1_u64 << tail) - 1;
    }
    buffer
}

/// Checks the frozen logical bit-length boundaries.
fn bit_length_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    for bits in BOUNDARY_BITS {
        let name = format!("xor-bits-{bits}");
        let source = seeded(CAMPAIGN_SEED ^ bits as u64, bits);
        let original = seeded(CAMPAIGN_SEED.rotate_left(17) ^ bits as u64, bits);
        let model: Vec<u64> = original
            .iter()
            .zip(source.iter())
            .map(|(d, s)| d ^ s)
            .collect();
        let mut destination = original.clone();
        public_xor(&mut destination, &source);
        let mut checks = 0;
        if destination != model {
            return Err(fail(
                &name,
                "an output word differs from the wordwise model",
            ));
        }
        checks += destination.len();
        for index in 0..bits {
            let observed = (destination[index >> 6] >> (index & 63)) & 1;
            let expected = ((original[index >> 6] >> (index & 63)) & 1)
                ^ ((source[index >> 6] >> (index & 63)) & 1);
            if observed != expected {
                return Err(fail(&name, format!("bit {index} is not LSB-first XOR")));
            }
            checks += 1;
        }
        let tail = bits % 64;
        if tail != 0 {
            let padding = destination[destination.len() - 1] >> tail;
            if padding != 0 {
                return Err(fail(&name, "the partial final word carries tail padding"));
            }
            checks += 1;
        }
        if source != seeded(CAMPAIGN_SEED ^ bits as u64, bits) {
            return Err(fail(&name, "the source changed"));
        }
        checks += 1;
        if destination.len() != bits.div_ceil(64) {
            return Err(fail(&name, "the logical length changed"));
        }
        checks += 1;
        report.push(OracleCase { name, checks });
    }
    Ok(())
}

/// Checks every word count in both address layouts.
fn isolated_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    for words in ALL_WORDS {
        for layout in [Layout::A64, Layout::O8] {
            let name = format!("xor-{words}w-{}", layout.id());
            let seed = CAMPAIGN_SEED ^ ((words as u64) << 8) ^ layout.offset_bytes() as u64;
            let mut banks = XorBanks::build(words, layout, Cache::Warm, seed);
            let mut checks = 0;
            let (source_addr, destination_addr) = banks.addresses_mod_64(0, 0);
            let (source_base, destination_base) = banks.bases_mod_64(0);
            let wanted = layout.offset_bytes() % SLAB_ALIGN;
            if (source_addr, destination_addr) != (wanted, wanted) {
                return Err(fail(
                    &name,
                    format!("views begin at ({source_addr}, {destination_addr}) mod 64"),
                ));
            }
            if (source_base, destination_base) != (0, 0) {
                return Err(fail(&name, "a slab base is not 64-byte aligned"));
            }
            checks += 4;

            let (destination, source) = banks.pair(0, 0);
            let original = destination.to_vec();
            let source_snapshot = source.to_vec();
            let model: Vec<u64> = original
                .iter()
                .zip(source_snapshot.iter())
                .map(|(d, s)| d ^ s)
                .collect();
            public_xor(destination, source);
            if destination != model.as_slice() {
                return Err(fail(
                    &name,
                    "an output word differs from the wordwise model",
                ));
            }
            checks += destination.len();
            for index in 0..words * 64 {
                let observed = (destination[index >> 6] >> (index & 63)) & 1;
                let expected = ((original[index >> 6] >> (index & 63)) & 1)
                    ^ ((source_snapshot[index >> 6] >> (index & 63)) & 1);
                if observed != expected {
                    return Err(fail(&name, format!("bit {index} is not LSB-first XOR")));
                }
                checks += 1;
            }
            // Call parity: a second application restores the destination, so a
            // timed window needs no reset inside its measured region.
            public_xor(destination, source);
            if destination != original.as_slice() {
                return Err(fail(
                    &name,
                    "two applications did not restore the destination",
                ));
            }
            checks += 1;
            if source != source_snapshot.as_slice() {
                return Err(fail(&name, "the source changed"));
            }
            checks += 1;

            // The hoisted attribution route observes the same bytes.
            let resolved = resolved_xor(words);
            resolved(destination, source);
            if destination != model.as_slice() {
                return Err(fail(
                    &name,
                    "the resolved route differs from the public route",
                ));
            }
            checks += 1;
            report.push(OracleCase { name, checks });
        }
    }
    Ok(())
}

/// Checks every word count in both matrix shapes over the frozen row cycle.
fn row_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    for words in ALL_WORDS {
        for shape in [RowShape::Full, RowShape::Tail63] {
            let name = format!("row-xor-{words}w-{}", shape.id());
            let seed = CAMPAIGN_SEED ^ ((words as u64) << 16) ^ shape.columns(words) as u64;
            let mut banks = RowBanks::build(words, shape, Cache::Warm, seed);
            let columns = shape.columns(words);
            let mut checks = 0;
            let mut model: Vec<Vec<u64>> = (0..ROW_MATRIX_ROWS)
                .map(|row| banks.matrix(0, 0).row_words(row).to_vec())
                .collect();
            for (index, &(dst, src)) in ROW_PAIRS.iter().enumerate() {
                public_row_xor(banks.matrix_mut(0, 0), index);
                let source = model[src].clone();
                for (target, value) in model[dst].iter_mut().zip(source.iter()) {
                    *target ^= value;
                }
            }
            let matrix = banks.matrix(0, 0);
            for (row, expected) in model.iter().enumerate() {
                if matrix.row_words(row) != expected.as_slice() {
                    return Err(fail(&name, format!("row {row} differs from the model")));
                }
                checks += 1;
            }
            checks += check_indexing(&name, matrix, columns)?;
            let tail = columns % 64;
            if tail != 0 {
                for row in 0..ROW_MATRIX_ROWS {
                    let last = matrix.row_words(row);
                    if last[last.len() - 1] >> tail != 0 {
                        return Err(fail(&name, format!("row {row} carries tail padding")));
                    }
                    checks += 1;
                }
            }
            if matrix.stride_words() != words {
                return Err(fail(
                    &name,
                    format!("stride {} is not {words}", matrix.stride_words()),
                ));
            }
            checks += 1;
            report.push(OracleCase { name, checks });
        }
    }
    Ok(())
}

fn check_indexing(name: &str, matrix: &BitMatrix, columns: usize) -> Result<usize, String> {
    let mut checks = 0;
    for row in 0..ROW_MATRIX_ROWS {
        let words = matrix.row_words(row);
        for column in 0..columns {
            let packed = (words[column >> 6] >> (column & 63)) & 1 == 1;
            if matrix.get(row, column) != packed {
                return Err(fail(
                    name,
                    format!("({row}, {column}) is not canonical LSB-first"),
                ));
            }
            checks += 1;
        }
    }
    Ok(checks)
}

/// Checks every selected NR route against its frozen declaration.
fn nr_cases(report: &mut Vec<OracleCase>) -> Result<(), String> {
    for target in NR_TARGETS {
        let name = format!("nr-construct-{}", target.suffix);
        let code = nr_construct(&target);
        let facts = verify_nr(&code, &target).map_err(|error| fail(&name, error))?;
        let again = nr_construct(&target);
        let repeated = verify_nr(&again, &target).map_err(|error| fail(&name, error))?;
        if facts.structure_digest != repeated.structure_digest {
            return Err(fail(&name, "the sparse structure digest is not stable"));
        }
        report.push(OracleCase { name, checks: 6 });
    }
    Ok(())
}

/// Runs every gf2-side oracle case.
pub fn run() -> Result<Vec<OracleCase>, String> {
    let mut report = Vec::new();
    bit_length_cases(&mut report)?;
    isolated_cases(&mut report)?;
    row_cases(&mut report)?;
    nr_cases(&mut report)?;
    Ok(report)
}
