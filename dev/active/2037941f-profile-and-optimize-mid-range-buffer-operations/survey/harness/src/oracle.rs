//! Deterministic untimed correctness oracle for every measured route.
//!
//! The oracle checks every output word, source immutability, canonical
//! LSB-first bit indexing, logical length and zero tail padding, over the
//! frozen bit-length boundaries, all seven word counts, both address layouts,
//! both matrix shapes and the four bidirectional row-pair groups. It emits no
//! timing sample and cannot serve as a pilot.

use crate::cells::{
    Cache, Layout, RowShape, ALL_WORDS, BOUNDARY_BITS, CAMPAIGN_SEED, MID_RANGE_MAX_WORDS,
    MID_RANGE_MIN_WORDS, NR_TARGETS, ROW_MATRIX_ROWS, ROW_PAIRS,
};
use crate::fixture::{RowBanks, XorBanks, SLAB_ALIGN};
use crate::routes::{
    canonical_structure_digest, nr_construct, public_row_xor, public_xor, resolved_xor, verify_nr,
};
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
use gf2_coding::traits::BlockEncoder;
use gf2_core::{BitMatrix, BitVec};
use tuning_campaign_support::abtest::SplitMix64;

/// One oracle case and the number of checks it performed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleCase {
    /// Case name, as printed in the validation record.
    pub name: String,
    /// Checks the case performed.
    pub checks: usize,
    /// Observed facts the case establishes, rendered after the check count.
    pub detail: String,
}

impl OracleCase {
    /// A case whose name and check count say everything it establishes.
    pub fn plain(name: String, checks: usize) -> Self {
        Self {
            name,
            checks,
            detail: String::new(),
        }
    }
}

impl std::fmt::Display for OracleCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "PASS {}: {} checks", self.name, self.checks)?;
        if self.detail.is_empty() {
            return Ok(());
        }
        write!(formatter, " [{}]", self.detail)
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
        report.push(OracleCase::plain(name, checks));
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
            report.push(OracleCase::plain(name, checks));
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
            // The offsets are the unmodified production allocation's: every
            // declared row sits at its own stride from the base.
            let base = banks.base_mod_64(0, 0);
            let stride_bytes = matrix.stride_words() * 8;
            let observed = banks.pair_addresses_mod_64(0, 0);
            let offsets = observed
                .iter()
                .map(|(dst, src)| format!("{dst}:{src}"))
                .collect::<Vec<_>>()
                .join(",");
            for (&(dst, src), &(dst_addr, src_addr)) in ROW_PAIRS.iter().zip(observed.iter()) {
                let wanted = (
                    (base + dst * stride_bytes) % 64,
                    (base + src * stride_bytes) % 64,
                );
                if (dst_addr, src_addr) != wanted {
                    return Err(fail(
                        &name,
                        format!(
                            "pair ({dst}, {src}) sits at {:?} rather than {wanted:?} mod 64",
                            (dst_addr, src_addr)
                        ),
                    ));
                }
                checks += 2;
            }
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
            report.push(OracleCase {
                name,
                checks,
                detail: format!(
                    "stride={}w columns={columns} base%64={base} dst:src%64={offsets}",
                    matrix.stride_words()
                ),
            });
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

/// Seeded boundary messages of one route, in the order the oracle encodes them.
///
/// The all-zero word anchors linearity, the two unit vectors reach the ends of
/// the systematic section, the all-ones word saturates it, and the seeded word
/// is the campaign stream's own draw for this route.
fn boundary_messages(k: usize, seed: u64) -> Vec<BitVec> {
    let mut unit_low = BitVec::zeros(k);
    unit_low.set(0, true);
    let mut unit_high = BitVec::zeros(k);
    unit_high.set(k - 1, true);
    let mut ones = BitVec::zeros(k);
    let mut seeded_word = BitVec::zeros(k);
    let mut mixer = SplitMix64::new(seed);
    let mut draw = mixer.next_u64();
    for index in 0..k {
        ones.set(index, true);
        if index % 64 == 0 && index > 0 {
            draw = mixer.next_u64();
        }
        seeded_word.set(index, (draw >> (index % 64)) & 1 == 1);
    }
    vec![BitVec::zeros(k), unit_low, unit_high, ones, seeded_word]
}

/// Bitwise XOR of two equally long words.
fn xor_bits(left: &BitVec, right: &BitVec) -> BitVec {
    let mut combined = BitVec::zeros(left.len());
    for index in 0..left.len() {
        combined.set(index, left.get(index) ^ right.get(index));
    }
    combined
}

/// Checks the seeded encodings of one route against the canonical linear model.
///
/// A linear block code answers the sum of two messages with the sum of their
/// codewords, and the zero message with the zero codeword. Two independently
/// constructed objects answer identically, which is the determinism the
/// measured configuration relies on.
fn nr_encode_checks(
    name: &str,
    code: &Nr5gRateMatchedCode,
    again: &Nr5gRateMatchedCode,
    k: usize,
    n: usize,
    seed: u64,
) -> Result<usize, String> {
    let messages = boundary_messages(k, seed);
    let codewords: Vec<BitVec> = messages
        .iter()
        .map(|message| code.encode(message))
        .collect();
    let mut checks = 0;
    for (message, codeword) in messages.iter().zip(codewords.iter()) {
        if codeword.len() != n {
            return Err(fail(
                name,
                format!("a codeword is {} bits, not {n}", codeword.len()),
            ));
        }
        let repeated = again.encode(message);
        if (0..n).any(|index| repeated.get(index) != codeword.get(index)) {
            return Err(fail(
                name,
                "two constructions of the route encode differently",
            ));
        }
        checks += 2;
    }
    if (0..n).any(|index| codewords[0].get(index)) {
        return Err(fail(
            name,
            "the zero message does not encode to the zero codeword",
        ));
    }
    checks += 1;
    for left in 1..messages.len() {
        for right in (left + 1)..messages.len() {
            let combined = code.encode(&xor_bits(&messages[left], &messages[right]));
            if (0..n).any(|index| {
                combined.get(index) != codewords[left].get(index) ^ codewords[right].get(index)
            }) {
                return Err(fail(
                    name,
                    "the encoding is not linear over the boundary messages",
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
        // `verify_nr` reads the lifting factor, both dense dimensions, the
        // stride, its mid-range band and the public (n, k) from the returned
        // object: seven of this case's checks.
        let facts = verify_nr(&code, &target).map_err(|error| fail(&name, error))?;
        let mut checks = 7;
        let canonical = canonical_structure_digest(target.base_graph, facts.lifting_factor)
            .map_err(|error| fail(&name, error))?;
        if facts.structure_digest != canonical {
            return Err(fail(
                &name,
                "the parity-check structure differs from the canonical 3GPP expansion",
            ));
        }
        checks += 1;
        let again = nr_construct(&target);
        let repeated = verify_nr(&again, &target).map_err(|error| fail(&name, error))?;
        if facts != repeated {
            return Err(fail(&name, "two constructions observe different facts"));
        }
        checks += 1;
        let seed = CAMPAIGN_SEED ^ ((target.target_n as u64) << 32) ^ target.target_k as u64;
        checks += nr_encode_checks(&name, &code, &again, target.target_k, target.target_n, seed)?;
        report.push(OracleCase {
            name,
            checks,
            detail: format!(
                "Z={} dense={}x{} stride={}w band={MID_RANGE_MIN_WORDS}-{MID_RANGE_MAX_WORDS}w nnz={} h-sha256={}",
                facts.lifting_factor,
                facts.dense_rows,
                facts.dense_cols,
                facts.stride_words,
                facts.nnz,
                facts.structure_digest
            ),
        });
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
