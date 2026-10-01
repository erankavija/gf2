//! Conformance tests for the named eBCH(128,64,22) reference construction.
//!
//! The reference code is the one-symbol extension of the primitive
//! narrow-sense BCH(127, 64) code over $\mathrm{GF}(2^7) =
//! \mathrm{GF}(2)\[x\]/(x^7 + x + 1)$ with designed distance 21, presented in
//! the systematic layout [`SystematicLayout::MessageParityDescending`]. The
//! fixture's generator and parity-check rows are that presentation's
//! canonical matrices.

use gf2_coding::bch::spec::{BinaryBchCode, DesignedDistance};
use gf2_coding::bch::{LayoutView, SystematicLayout};
use gf2_coding::traits::block::{
    BlockCode, BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess,
};
use gf2_coding::transform::Extended;
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};
use serde::Deserialize;

/// The eBCH(128,64,22) reference code over the base code's declared layout.
type ReferenceCode = Extended<LayoutView<BinaryPrimeExt, BitVec, BitMatrix>>;

const FIXTURE_TEXT: &str = include_str!("data/ebch_128_64_reference.json");

#[derive(Debug, Deserialize)]
struct ReferenceFixture {
    schema_version: u32,
    identity: Identity,
    field: Field,
    decision_provenance: DecisionProvenance,
    serialization: Serialization,
    generator_rows: Vec<String>,
    parity_check_rows: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Identity {
    name: String,
    n: usize,
    k: usize,
    minimum_distance: usize,
    base_n: usize,
    base_t: usize,
    systematic_convention: String,
    extension_bit_position: usize,
}

#[derive(Debug, Deserialize)]
struct Field {
    degree: usize,
    primitive_polynomial_binary: String,
    primitive_polynomial_hex: String,
    primitive_polynomial: String,
}

#[derive(Debug, Deserialize)]
struct DecisionProvenance {
    kind: String,
    decision: String,
    artifact: String,
    published_identity_artifact: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct Serialization {
    format: String,
    bit_index: String,
    row_encoding: String,
}

fn fixture() -> ReferenceFixture {
    serde_json::from_str(FIXTURE_TEXT).expect("reference fixture must be valid JSON")
}

/// Builds the reference code from its fixture-recorded field and base radius.
fn reference_code() -> ReferenceCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(7, 0b10000011).with_tables())
        .expect("x^7 + x + 1 presents GF(128)");
    let base = BinaryBchCode::primitive_narrow_sense(
        extension,
        DesignedDistance::try_from(21).expect("a positive designed distance"),
    )
    .expect("the primitive narrow-sense BCH(127, 64) code");
    Extended::new(LayoutView::new(
        base,
        SystematicLayout::MessageParityDescending,
    ))
    .expect("the one-symbol extension of BCH(127, 64)")
}

/// Returns the base code's correction radius.
fn base_radius(code: &ReferenceCode) -> usize {
    code.mother().code().correction_radius()
}

fn canonical_rows(matrix: &BitMatrix) -> Vec<String> {
    (0..matrix.rows())
        .map(|row| {
            let words = matrix.row_words(row);
            assert_eq!(words.len(), 2, "128-column fixture rows have two words");
            format!("{:016x}:{:016x}", words[0], words[1])
        })
        .collect()
}

#[test]
fn ebch_128_64_matches_canonical_fixture() {
    let fixture = fixture();
    assert_eq!(fixture.schema_version, 1);
    assert_eq!(fixture.identity.name, "eBCH(128,64,22)");
    assert_eq!(fixture.identity.n, 128);
    assert_eq!(fixture.identity.k, 64);
    assert_eq!(fixture.identity.minimum_distance, 22);
    assert_eq!(fixture.identity.base_n, 127);
    assert_eq!(fixture.identity.base_t, 10);
    assert_eq!(
        fixture.identity.systematic_convention,
        "message_then_parity"
    );
    assert_eq!(fixture.identity.extension_bit_position, 127);
    assert_eq!(fixture.field.degree, 7);
    assert_eq!(fixture.field.primitive_polynomial_binary, "0b10000011");
    assert_eq!(fixture.field.primitive_polynomial_hex, "0x83");
    assert_eq!(fixture.field.primitive_polynomial, "x^7 + x + 1");
    assert_eq!(fixture.decision_provenance.kind, "campaign_decision");
    assert_eq!(fixture.decision_provenance.decision, "D-21");
    assert_eq!(
        fixture.decision_provenance.artifact,
        "dev/archive/b7157be6-osd/active/plan.md"
    );
    assert_eq!(
        fixture.decision_provenance.published_identity_artifact,
        "dev/reference_data/osd_ebch_128_64_fossorier1994.md"
    );
    assert!(fixture
        .decision_provenance
        .rationale
        .contains("not source-determined"));
    assert_eq!(fixture.serialization.format, "row-major-u64-le-hex-v1");
    assert_eq!(
        fixture.serialization.bit_index,
        "column c maps to bit (c & 63) of word (c >> 6)"
    );
    assert_eq!(
        fixture.serialization.row_encoding,
        "two lowercase 16-digit hex words, columns 0..63 first"
    );

    let code = reference_code();
    assert_eq!(code.n(), fixture.identity.n);
    assert_eq!(code.k(), fixture.identity.k);
    assert_eq!(code.mother().n(), fixture.identity.base_n);
    assert_eq!(base_radius(&code), fixture.identity.base_t);
    assert_eq!(
        canonical_rows(&code.generator_matrix().expect("the generator materializes")),
        fixture.generator_rows
    );
    assert_eq!(
        canonical_rows(
            &code
                .parity_check_matrix()
                .expect("the parity check materializes")
        ),
        fixture.parity_check_rows
    );
}

#[test]
fn ebch_128_64_matrices_are_systematic_orthogonal_and_even() {
    let code = reference_code();
    let generator = code.generator_matrix().expect("the generator materializes");
    let parity_check = code
        .parity_check_matrix()
        .expect("the parity check materializes");

    assert_eq!((generator.rows(), generator.cols()), (64, 128));
    assert_eq!((parity_check.rows(), parity_check.cols()), (64, 128));
    assert!(code
        .is_systematic()
        .expect("systematic status is decidable"));

    for row in 0..generator.rows() {
        for column in 0..code.k() {
            assert_eq!(
                generator.get(row, column),
                row == column,
                "systematic identity mismatch at ({row}, {column})"
            );
        }
        assert!(
            generator.row_as_bitvec(row).count_ones().is_multiple_of(2),
            "extended generator row {row} must have even weight"
        );
    }

    let product = &generator * &parity_check.transpose();
    assert_eq!(product, BitMatrix::zeros(64, 64));
}

#[test]
fn ebch_128_64_encoding_preserves_messages_and_extension_parity() {
    let code = reference_code();

    for bit in 0..code.k() {
        let mut message = BitVec::zeros(code.k());
        message.set(bit, true);
        let codeword = code.encode(&message).expect("a 64-bit message encodes");

        assert_eq!(codeword.len(), 128);
        for message_bit in 0..code.k() {
            assert_eq!(codeword.get(message_bit), message.get(message_bit));
        }
        assert_eq!(codeword.count_ones() % 2, 0);
        assert_eq!(
            codeword.get(127),
            (0..127).filter(|&column| codeword.get(column)).count() % 2 == 1,
            "extension bit must make basis codeword {bit} even"
        );
    }
}
