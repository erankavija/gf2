//! Differential evidence for the batch-encoding family dispatch seam.
//!
//! The rows are the predeclared corpus of the workload-selection contract
//! (`dev/active/4e732b56/workload-selection.md` § 2), and the batch ladder is
//! its § 3. Every registered family is checked bit-identical against
//! [`EncodeFamily::REFERENCE`] on each row, at every declared layout, and
//! across worker counts.

use std::num::NonZeroUsize;

use gf2_coding::bch::encode::{max_parallel_batch_workers, EncodeFamily, SystematicLayout};
use gf2_coding::bch::error::BchError;
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::BitVec;

/// One corpus row: the code a cell encodes, and the batch lengths it is
/// encoded at.
struct Row {
    name: &'static str,
    /// Extension degree of the mother field.
    degree: usize,
    /// Primitive polynomial of the mother field, matching the contract's
    /// `prim` column.
    modulus: u64,
    /// Designed distance $\delta$ of the contract row.
    designed_distance: u64,
    /// Batch lengths from the contract's ladder that this row is encoded at.
    batches: &'static [usize],
}

const LAYOUTS: &[SystematicLayout] = &[
    SystematicLayout::MessageParityAscending,
    SystematicLayout::MessageParityDescending,
];

/// The contract's binary conformance rows carry their exact lengths; the two
/// DVB-T2 rows carry their generator polynomials at the mother length
/// $2^m - 1$, which is where the canonical construction model reaches them
/// until the shortened presentations migrate (`97410c80`). See the
/// 2026-09-01 amendment in the workload-selection contract.
const ROWS: &[Row] = &[
    Row {
        name: "B1",
        degree: 4,
        modulus: 0b1_0011,
        designed_distance: 7,
        batches: &[1, 16, 256, 4096],
    },
    Row {
        name: "B2",
        degree: 7,
        modulus: 0b1000_0011,
        designed_distance: 21,
        batches: &[1, 16, 256, 4096],
    },
    Row {
        name: "B3",
        degree: 8,
        modulus: 0b1_0001_1101,
        designed_distance: 9,
        batches: &[1, 16, 256, 4096],
    },
    Row {
        name: "T2S",
        degree: 14,
        modulus: 0b100_0000_0010_1011,
        designed_distance: 25,
        batches: &[1, 16],
    },
    Row {
        name: "T2N",
        degree: 16,
        modulus: 0b1_0000_0000_0010_1101,
        designed_distance: 25,
        batches: &[1, 16],
    },
];

/// Codes whose redundancy sits on a packed word boundary, so a reduction
/// that consumes whole blocks meets the padding cases 63, 64, and 65.
const WORD_BOUNDARY_ROWS: &[Row] = &[
    // r = 45 with k = 18: every message degree runs through the leading
    // partial block, so a whole-block step never executes.
    Row {
        name: "r45-k18",
        degree: 6,
        modulus: 0b100_0011,
        designed_distance: 17,
        batches: &[1, 8],
    },
    // r = 27 with k = 36: one whole block and a four-degree lead, below the
    // block width the table family needs.
    Row {
        name: "r27-k36",
        degree: 6,
        modulus: 0b100_0011,
        designed_distance: 11,
        batches: &[1, 8],
    },
    // r = 64: exactly one packed word.
    Row {
        name: "r64",
        degree: 8,
        modulus: 0b1_0001_1101,
        designed_distance: 17,
        batches: &[1, 8],
    },
    // r = 65: one packed word plus one coefficient.
    Row {
        name: "r65",
        degree: 13,
        modulus: 0b10_0000_0001_1011,
        designed_distance: 11,
        batches: &[1, 8],
    },
];

fn build(row: &Row) -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(row.degree, row.modulus))
        .expect("the contract's prim column is a primitive polynomial");
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(row.designed_distance)
            .expect("a positive designed distance"),
    })
    .expect("a narrow-sense construction over the contract's mother field")
}

fn messages(code: &BinaryBchCode, count: usize) -> Vec<BitVec> {
    (0..count)
        .map(|index| BitVec::random_seeded(code.k(), index as u64 + 1))
        .collect()
}

/// Encodes `messages` under `family` and returns the codewords.
fn encode_under(
    code: &BinaryBchCode,
    family: EncodeFamily,
    messages: &[BitVec],
    layout: SystematicLayout,
) -> Vec<BitVec> {
    let mut codewords = vec![BitVec::zeros(code.n()); messages.len()];
    let mut workspace = code.encode_workspace();
    code.encode_batch_family_into(family, messages, layout, &mut workspace, &mut codewords)
        .expect("an available family encodes a validated batch");
    codewords
}

/// Checks every registered family this code makes available against the
/// reference, on every declared layout and every batch length of `row`.
fn check_families_agree(row: &Row) {
    let code = build(row);
    for &layout in LAYOUTS {
        for &batch in row.batches {
            let batch_messages = messages(&code, batch);
            let reference = encode_under(&code, EncodeFamily::REFERENCE, &batch_messages, layout);
            for &family in EncodeFamily::REGISTERED {
                if !code.encode_family_available(family, layout) {
                    continue;
                }
                let selected = encode_under(&code, family, &batch_messages, layout);
                assert_eq!(
                    selected,
                    reference,
                    "{} at batch {batch} under {layout:?}: {family} differs from {}",
                    row.name,
                    EncodeFamily::REFERENCE
                );
            }
        }
    }
}

#[test]
fn registered_families_agree_on_the_corpus_rows() {
    for row in ROWS {
        check_families_agree(row);
    }
}

#[test]
fn registered_families_agree_at_packed_word_boundaries() {
    for row in WORD_BOUNDARY_ROWS {
        check_families_agree(row);
    }
}

#[test]
fn the_reference_family_is_available_on_every_corpus_row() {
    for row in ROWS.iter().chain(WORD_BOUNDARY_ROWS) {
        let code = build(row);
        for &layout in LAYOUTS {
            assert!(
                code.encode_family_available(EncodeFamily::REFERENCE, layout),
                "{} under {layout:?} must keep the scalar reference",
                row.name
            );
        }
    }
}

#[test]
fn at_least_two_families_are_registered_and_reachable() {
    assert!(
        EncodeFamily::REGISTERED.len() >= 2,
        "the seam registers at least the reference and one alternative"
    );
    let reachable: Vec<EncodeFamily> = EncodeFamily::REGISTERED
        .iter()
        .copied()
        .filter(|&family| {
            ROWS.iter()
                .any(|row| build(row).encode_family_available(family, SystematicLayout::default()))
        })
        .collect();
    assert_eq!(
        reachable.len(),
        EncodeFamily::REGISTERED.len(),
        "every registered family is available on some corpus row, found {reachable:?}"
    );
}

#[test]
fn family_names_round_trip_through_the_registry() {
    for &family in EncodeFamily::REGISTERED {
        assert_eq!(EncodeFamily::from_name(family.name()), Some(family));
    }
    assert_eq!(EncodeFamily::from_name("no-such-family"), None);
}

/// The selected family under this process's profile, which installs none, is
/// the reference. `bch_encode_dispatch_installed.rs` runs the same comparison
/// in a child process whose installed profile selects the table family, so
/// both arms of the seam are witnessed across worker counts.
#[test]
fn the_selected_family_is_bit_identical_across_worker_counts() {
    let row = &ROWS[2];
    let code = build(row);
    let batch_messages = messages(&code, 257);
    for &layout in LAYOUTS {
        let reference = encode_under(&code, EncodeFamily::REFERENCE, &batch_messages, layout);
        let ceiling = max_parallel_batch_workers().get().max(4) + 1;
        for workers in 1..=ceiling {
            let workers = NonZeroUsize::new(workers).expect("a positive worker count");
            let codewords = code
                .encode_batch_parallel(&batch_messages, layout, workers)
                .expect("a validated batch encodes");
            assert_eq!(
                codewords, reference,
                "{} under {layout:?} changed with {workers} workers",
                row.name
            );
        }
    }
}

#[test]
fn the_conservative_profile_selects_the_scalar_reference() {
    for row in ROWS {
        let code = build(row);
        for &layout in LAYOUTS {
            for &batch in row.batches {
                assert_eq!(
                    code.selected_encode_family(layout, batch),
                    EncodeFamily::REFERENCE,
                    "{} at batch {batch} under {layout:?}",
                    row.name
                );
            }
        }
    }
}

#[test]
fn the_selected_family_writes_the_bytes_the_batch_entry_points_write() {
    for row in ROWS {
        let code = build(row);
        for &layout in LAYOUTS {
            let batch_messages = messages(&code, 16);
            let selected = code.selected_encode_family(layout, batch_messages.len());
            let explicit = encode_under(&code, selected, &batch_messages, layout);
            let allocating = code
                .encode_batch(&batch_messages, layout)
                .expect("a validated batch encodes");
            let mut workspace = code.encode_workspace();
            let mut into = vec![BitVec::zeros(code.n()); batch_messages.len()];
            code.encode_batch_into(&batch_messages, layout, &mut workspace, &mut into)
                .expect("a validated batch encodes");
            assert_eq!(allocating, explicit, "{} under {layout:?}", row.name);
            assert_eq!(into, explicit, "{} under {layout:?}", row.name);
        }
    }
}

#[test]
fn an_unavailable_family_is_reported_rather_than_substituted() {
    // B1 has redundancy 10, below the block width the table family reduces
    // in, so the seam leaves it on the reference and an explicit request for
    // it is a typed error rather than a silent substitution.
    let row = &ROWS[0];
    let code = build(row);
    let layout = SystematicLayout::default();
    assert!(!code.encode_family_available(EncodeFamily::TableRemainder, layout));
    let batch_messages = messages(&code, 4);
    let mut codewords = vec![BitVec::zeros(code.n()); batch_messages.len()];
    let mut workspace = code.encode_workspace();
    let error = code
        .encode_batch_family_into(
            EncodeFamily::TableRemainder,
            &batch_messages,
            layout,
            &mut workspace,
            &mut codewords,
        )
        .expect_err("the table family is unavailable at redundancy 10");
    assert_eq!(
        error,
        BchError::EncodeFamilyUnavailable {
            family: EncodeFamily::TableRemainder
        }
    );
    assert_eq!(
        code.selected_encode_family(layout, batch_messages.len()),
        EncodeFamily::REFERENCE
    );
}

#[test]
fn an_empty_batch_encodes_under_every_available_family() {
    let code = build(&ROWS[2]);
    for &layout in LAYOUTS {
        for &family in EncodeFamily::REGISTERED {
            if !code.encode_family_available(family, layout) {
                continue;
            }
            let mut workspace = code.encode_workspace();
            code.encode_batch_family_into(family, &[], layout, &mut workspace, &mut [])
                .expect("the empty batch is a valid batch");
        }
    }
}
