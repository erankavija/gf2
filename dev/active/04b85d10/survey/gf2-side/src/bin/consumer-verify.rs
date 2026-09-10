//! Correctness evidence for the bit-storage consumer profile (jit:04b85d10).
//!
//! Every cell of this profile compares two production routes to one consumer
//! result. A timing comparison is meaningful only where the routes agree, so
//! this binary establishes that agreement first, on the exact code the arms
//! call, and prints one JSON record per check. It exits non-zero on the first
//! disagreement.
//!
//! The checks cover the word-boundary cases the repository requires of
//! bit-packed behaviour — 0, 1, 63, 64 and 65 bits — alongside the sizes the
//! profile measures.

use std::process::ExitCode;

use gf2_coding::bch::encode::{EncodeFamily, SystematicLayout};
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_coding::bch::CodeRate;
use gf2_coding::ldpc::LdpcCode;
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::kernels::ops::{popcount, resolve_xor_inplace, xor_inplace};
use gf2_core::kernels::scalar::ScalarBackend;
use gf2_core::kernels::Backend;
use gf2_core::{BitMatrix, BitVec};
use gf2_kernels_simd::transpose;

/// The bit lengths every packed check runs at.
///
/// The five boundary values are the repository's required word-boundary
/// cases; the remainder are the ladder the profile measures.
const BIT_LENGTHS: &[usize] = &[0, 1, 63, 64, 65, 512, 4096, 32768, 524_288];

/// One completed check.
struct Check {
    name: &'static str,
    detail: String,
    passed: bool,
}

fn record(name: &'static str, detail: String, passed: bool) -> Check {
    Check {
        name,
        detail,
        passed,
    }
}

/// Every logical XOR route writes the same words.
fn check_row_xor() -> Check {
    let simd = gf2_core::kernels::simd::maybe_simd();
    let mut mismatches = Vec::new();
    for &bits in BIT_LENGTHS {
        let words = bits.div_ceil(64);
        if words == 0 {
            continue;
        }
        let left = BitVec::random_seeded(words * 64, 0x51D0).words().to_vec();
        let right = BitVec::random_seeded(words * 64, 0x9C4E).words().to_vec();

        let mut dispatched = left.clone();
        xor_inplace(&mut dispatched, &right);

        let mut resolved = left.clone();
        (resolve_xor_inplace(words))(&mut resolved, &right);

        let mut scalar = left.clone();
        ScalarBackend.xor(&mut scalar, &right);

        if dispatched != resolved || dispatched != scalar {
            mismatches.push(format!(
                "{bits} bits: dispatched, resolved and scalar differ"
            ));
        }
        if let Some(backend) = simd {
            let mut simd_words = left.clone();
            backend.xor(&mut simd_words, &right);
            if simd_words != scalar {
                mismatches.push(format!("{bits} bits: simd differs from scalar"));
            }
        }
    }
    record(
        "row-xor-routes-agree",
        if mismatches.is_empty() {
            format!("{} bit lengths agree", BIT_LENGTHS.len())
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

/// Every population-count route returns the same count.
fn check_popcount() -> Check {
    let simd = gf2_core::kernels::simd::maybe_simd();
    let mut mismatches = Vec::new();
    for &bits in BIT_LENGTHS {
        let words = bits.div_ceil(64);
        let buffer = BitVec::random_seeded(words * 64, 0xC0FF).words().to_vec();
        let reference: u64 = buffer.iter().map(|word| u64::from(word.count_ones())).sum();
        if popcount(&buffer) != reference || ScalarBackend.popcount(&buffer) != reference {
            mismatches.push(format!("{bits} bits: dispatched or scalar count differs"));
        }
        if let Some(backend) = simd {
            if backend.popcount(&buffer) != reference {
                mismatches.push(format!("{bits} bits: simd count differs"));
            }
        }
    }
    record(
        "popcount-routes-agree",
        if mismatches.is_empty() {
            format!("{} bit lengths agree", BIT_LENGTHS.len())
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

/// The full count and the first-set-bit search answer the same zero question.
fn check_zero_test() -> Check {
    let mut mismatches = Vec::new();
    for &bits in BIT_LENGTHS {
        if bits == 0 {
            let empty = BitVec::zeros(0);
            if (empty.count_ones() == 0) != empty.find_first_one().is_none() {
                mismatches.push("0 bits: the two spellings disagree".to_owned());
            }
            continue;
        }
        // The all-zero case, the first bit, the last bit and one interior bit.
        let positions = [None, Some(0), Some(bits - 1), Some(bits / 2)];
        for position in positions {
            let mut vector = BitVec::zeros(bits);
            if let Some(index) = position {
                vector.set(index, true);
            }
            if (vector.count_ones() == 0) != vector.find_first_one().is_none() {
                mismatches.push(format!("{bits} bits, set {position:?}: spellings disagree"));
            }
        }
    }
    record(
        "zero-test-spellings-agree",
        if mismatches.is_empty() {
            format!(
                "{} bit lengths agree over four bit patterns",
                BIT_LENGTHS.len()
            )
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

/// The two 64x64 block-transpose lanes write the same words, and the
/// transpose is an involution.
fn check_transpose() -> Check {
    let mut mismatches = Vec::new();
    let vector = BitVec::random_seeded(64 * 64, 0x7A11);
    let mut input = [0u64; 64];
    input.copy_from_slice(vector.words());

    let mut scalar_out = [0u64; 64];
    transpose::transpose_64x64_scalar(&input, &mut scalar_out);

    match transpose::detect() {
        Some(fns) => {
            let mut detected_out = [0u64; 64];
            (fns.transpose_64x64)(&input, &mut detected_out);
            if detected_out != scalar_out {
                mismatches.push(format!(
                    "the {} lane differs from the portable one",
                    fns.name
                ));
            }
        }
        None => mismatches.push("this host publishes no block-transpose bundle".to_owned()),
    }

    let mut round_trip = [0u64; 64];
    transpose::transpose_64x64_scalar(&scalar_out, &mut round_trip);
    if round_trip != input {
        mismatches.push("the portable lane is not an involution".to_owned());
    }
    record(
        "transpose-lanes-agree",
        if mismatches.is_empty() {
            "both lanes agree and the transpose is an involution".to_owned()
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

/// The dense transpose of a random matrix restores itself.
fn check_dense_transpose() -> Check {
    let mut mismatches = Vec::new();
    for &(rows, cols) in &[(64usize, 64usize), (65, 63), (513, 257), (1024, 1024)] {
        let matrix = BitMatrix::random_seeded(rows, cols, 0x2C3F);
        let round_trip = matrix.transpose().transpose();
        if round_trip.rows() != rows || round_trip.cols() != cols {
            mismatches.push(format!("{rows}x{cols}: the round trip changed the shape"));
            continue;
        }
        let mut equal = true;
        for row in 0..rows {
            if matrix.row_words(row) != round_trip.row_words(row) {
                equal = false;
                break;
            }
        }
        if !equal {
            mismatches.push(format!("{rows}x{cols}: the round trip changed the words"));
        }
    }
    record(
        "dense-transpose-round-trips",
        if mismatches.is_empty() {
            "four shapes round-trip identically".to_owned()
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

/// The dense matrix-vector product matches a naive row-parity reference.
fn check_dense_matvec() -> Check {
    let mut mismatches = Vec::new();
    for &(rows, cols) in &[(64usize, 64usize), (65, 65), (256, 1024), (1024, 4096)] {
        let matrix = BitMatrix::random_seeded(rows, cols, 0x4B71);
        let x = BitVec::random_seeded(cols, 0x8D22);
        let y = matrix.matvec(&x);
        for row in 0..rows {
            let parity = matrix
                .row_words(row)
                .iter()
                .zip(x.words())
                .fold(0u64, |accumulator, (left, right)| {
                    accumulator ^ (left & right)
                })
                .count_ones()
                & 1
                == 1;
            if y.get(row) != parity {
                mismatches.push(format!(
                    "{rows}x{cols}: row {row} differs from the reference"
                ));
                break;
            }
        }
    }
    record(
        "dense-matvec-matches-reference",
        if mismatches.is_empty() {
            "four shapes match the naive row-parity reference".to_owned()
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

/// The two spellings of the LDPC zero-syndrome check agree, on a codeword and
/// on a corrupted word.
fn check_ldpc_codeword() -> Check {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let mut mismatches = Vec::new();

    let zero = BitVec::zeros(code.n());
    if !code.is_valid_codeword(&zero) || code.syndrome(&zero).find_first_one().is_some() {
        mismatches.push("the all-zero word is not accepted by both spellings".to_owned());
    }

    let mut corrupted = BitVec::zeros(code.n());
    corrupted.set(7, true);
    let counted = code.is_valid_codeword(&corrupted);
    let searched = code.syndrome(&corrupted).find_first_one().is_none();
    if counted != searched {
        mismatches.push("a weight-one word splits the two spellings".to_owned());
    }
    if counted {
        mismatches.push("a weight-one word is accepted as a codeword".to_owned());
    }

    let random = BitVec::random_seeded(code.n(), 0x1D0C);
    if code.is_valid_codeword(&random) != code.syndrome(&random).find_first_one().is_none() {
        mismatches.push("a random word splits the two spellings".to_owned());
    }
    record(
        "ldpc-zero-syndrome-spellings-agree",
        if mismatches.is_empty() {
            "the two spellings agree on the zero, weight-one and random words".to_owned()
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

/// Builds one declared binary BCH row.
fn build(degree: usize, modulus: u64, designed_distance: u64) -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(degree, modulus))
        .expect("the declared polynomial is primitive");
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(designed_distance)
            .expect("a positive designed distance"),
    })
    .expect("a narrow-sense construction over the declared mother field")
}

/// Every available encoding family writes the same codewords as the reference
/// family, and the allocating entry point writes those same codewords.
fn check_bch_families() -> Check {
    let mut mismatches = Vec::new();
    let layout = SystematicLayout::default();
    for &(degree, modulus, distance) in &[
        (8usize, 0b1_0001_1101u64, 9u64),
        (14, 0b100_0000_0010_1011, 25),
        (16, 0b1_0000_0000_0010_1101, 25),
    ] {
        let code = build(degree, modulus, distance);
        for &batch in &[1usize, 16, 64, 65, 256] {
            let messages: Vec<BitVec> = (0..batch)
                .map(|index| BitVec::random_seeded(code.k(), index as u64 + 1))
                .collect();
            let mut reference = vec![BitVec::zeros(code.n()); batch];
            let mut workspace = code.encode_workspace();
            code.encode_batch_family_into(
                EncodeFamily::REFERENCE,
                &messages,
                layout,
                &mut workspace,
                &mut reference,
            )
            .expect("the reference family encodes a validated batch");

            for &family in EncodeFamily::REGISTERED {
                if !code.encode_family_available(family, layout) {
                    continue;
                }
                let mut codewords = vec![BitVec::zeros(code.n()); batch];
                let mut workspace = code.encode_workspace();
                code.encode_batch_family_into(
                    family,
                    &messages,
                    layout,
                    &mut workspace,
                    &mut codewords,
                )
                .expect("an available family encodes a validated batch");
                if codewords != reference {
                    mismatches.push(format!(
                        "m={degree} batch={batch}: {family:?} differs from the reference"
                    ));
                }
            }

            let allocated = code
                .encode_batch(&messages, layout)
                .expect("the selected family encodes a validated batch");
            if allocated != reference {
                mismatches.push(format!(
                    "m={degree} batch={batch}: the allocating entry point differs"
                ));
            }

            // The caller-buffer entry point is the candidate arm of the
            // allocation cell; it selects the same family as `encode_batch`.
            let mut selected = vec![BitVec::zeros(code.n()); batch];
            let mut workspace = code.encode_workspace();
            code.encode_batch_into(&messages, layout, &mut workspace, &mut selected)
                .expect("the selected family encodes a validated batch");
            if selected != reference {
                mismatches.push(format!(
                    "m={degree} batch={batch}: the caller-buffer entry point differs"
                ));
            }
        }
    }
    record(
        "bch-encoding-families-agree",
        if mismatches.is_empty() {
            "three rows, five batch lengths, every available family and both batch entry points bit-identical".to_owned()
        } else {
            mismatches.join("; ")
        },
        mismatches.is_empty(),
    )
}

fn main() -> ExitCode {
    let checks = vec![
        check_row_xor(),
        check_popcount(),
        check_zero_test(),
        check_transpose(),
        check_dense_transpose(),
        check_dense_matvec(),
        check_ldpc_codeword(),
        check_bch_families(),
    ];
    let mut failed = 0usize;
    for check in &checks {
        if !check.passed {
            failed += 1;
        }
        println!(
            "{}",
            serde_json::json!({
                "schema": "consumer-verify-record-v1",
                "check": check.name,
                "passed": check.passed,
                "detail": check.detail,
            })
        );
    }
    println!(
        "{}",
        serde_json::json!({
            "schema": "consumer-verify-summary-v1",
            "checks": checks.len(),
            "failed": failed,
        })
    );
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
