//! Conformance tests for the named eBCH(128,64,22) reference construction.

use gf2_coding::bch::extended::ExtendedBchCode;
use gf2_coding::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_core::{BitMatrix, BitVec};
use serde::Deserialize;

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
        "dev/active/b7157be6-osd/plan.md"
    );
    assert_eq!(
        fixture.decision_provenance.published_identity_artifact,
        "dev/reference_data/osd_ebch_128_64_fossorier1995.md"
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

    let code = ExtendedBchCode::ebch_128_64();
    assert_eq!(code.n(), fixture.identity.n);
    assert_eq!(code.k(), fixture.identity.k);
    assert_eq!(code.base_t(), fixture.identity.base_t);
    assert_eq!(
        canonical_rows(&code.generator_matrix()),
        fixture.generator_rows
    );
    assert_eq!(
        canonical_rows(code.parity_check()),
        fixture.parity_check_rows
    );
}

#[test]
fn ebch_128_64_matrices_are_systematic_orthogonal_and_even() {
    let code = ExtendedBchCode::ebch_128_64();
    let generator = code.generator_matrix();
    let parity_check = code.parity_check();

    assert_eq!((generator.rows(), generator.cols()), (64, 128));
    assert_eq!((parity_check.rows(), parity_check.cols()), (64, 128));
    assert!(code.is_systematic());

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
    let code = ExtendedBchCode::ebch_128_64();

    for bit in 0..code.k() {
        let mut message = BitVec::zeros(code.k());
        message.set(bit, true);
        let codeword = code.encode(&message);

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
