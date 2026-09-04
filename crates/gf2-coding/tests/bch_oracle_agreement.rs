//! Agreement of the predeclared BCH corpus with two external oracles.
//!
//! The evidence protocol in `dev/active/ae03bcd0-general-bch/plan.md` fixes
//! eight conformance rows, one message seed, and two oracles: SageMath's
//! `codes.BCHCode` and GAP's GUAVA `BCHCode`. The committed fixtures under
//! `tests/data/bch_oracle/` carry what each oracle derived; the generation
//! scripts, versions and provenance live in
//! `dev/active/ae03bcd0-general-bch/oracle-provenance.md`.
//!
//! A BCH code is fixed by $(q, n, b, \delta)$ *and* the primitive $n$-th root
//! of unity $\alpha$, so each oracle is run on gf2's $\alpha$, and GUAVA's
//! unaided `BCHCode` — which picks `PrimitiveUnityRoot(q, n)` — is compared
//! against the gf2 code carrying that root instead. The fixtures record which
//! presentation each oracle used and both roots in gf2's coordinates.
//!
//! The GAP script calls `BCHCode` on every row as an attempt bounded by the
//! run's heap and records what that attempt observed, so a row's code object
//! is a fact of the run. Amendment 2 of the plan's `evidence-protocol`
//! section names the rows whose GUAVA result is the `BCHCode` generator
//! derivation and the cyclic-code polynomial map instead of a code object.
//!
//! Every comparison here is between the fixture and a code this suite
//! constructs: the corpus rows from
//! `gf2_coding::test_support::visit_bch_corpus`, which is the same
//! construction the corpus emitter used, and, where GUAVA's root differs, the
//! same construction with that root supplied explicitly.
//!
//! The authoritative DVB-T2 vectors enter through
//! `the_etsi_dvb_t2_streams_encode_to_their_verified_codewords`, which encodes
//! the ETSI verification stream's TP04 payloads through the canonical mother
//! code and compares them with TP05 bit for bit.
//!
//! # Coordinates
//!
//! Fixture polynomials and codewords are written in ascending polynomial
//! degree, with each base-field symbol given by its canonical index: the
//! integer whose base-$p$ digits are the symbol's canonical prime
//! coordinates. Codewords therefore need the layout's coordinate map before
//! they can be compared with an encoder result in user coordinates.

mod test_vectors;

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use gf2_coding::bch::dvb_t2::generators::{product_of_generators, NORMAL_GENERATORS};
use gf2_coding::bch::dvb_t2::{DvbBchParams, FrameSize};
use gf2_coding::bch::encode::{SystematicKernel, SystematicLayout};
use gf2_coding::bch::spec::{BchCode, BchSpec, BinaryBchCode, RootExponent, RootSelection};
use gf2_coding::bch::{BchCode as LegacyBchCode, BchEncoder};
use gf2_coding::test_support::{
    bch_corpus_decode_symbols, bch_corpus_distance, bch_corpus_element, bch_corpus_encode_symbols,
    bch_corpus_index, bch_corpus_length, bch_corpus_messages, visit_bch_corpus, BchCorpusRow,
    BchCorpusVisitor,
};
use gf2_coding::traits::block::{BlockCode, SymbolMatrix};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension};
use gf2_core::field::modulus_select::SelectExtension;
use gf2_core::field::{ConstField, FieldPoly};
use gf2_core::gfp::Fp;
use gf2_core::BitVec;
use serde_json::Value;
use sha2::{Digest, Sha256};
use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

/// Length at or below which rows are compared symbol by symbol together; a
/// longer row is compared in its own case. Amendment 1 of the plan's
/// `evidence-protocol` section fixes this threshold.
const FAST_TIER_LENGTH: usize = 4096;

/// The seed the shortened-payload check draws its DVB-T2 payloads from.
const DVB_PAYLOAD_SEED: u64 = 0xAE03_BCD0;

/// Payloads the shortened-parity check encodes, per Amendment 1 of the plan's
/// `evidence-protocol` section.
const DVB_PAYLOADS: usize = 3;

// ---------------------------------------------------------------------------
// Fixture access
// ---------------------------------------------------------------------------

/// Returns the committed fixture directory.
fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/bch_oracle")
}

/// Loads one committed fixture.
fn fixture(name: &str) -> Value {
    let path = data_dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("parsing {}: {error}", path.display()))
}

/// Indexes a fixture's rows by their row identifier.
fn rows_by_id(fixture: &Value) -> BTreeMap<String, Value> {
    fixture["rows"]
        .as_array()
        .expect("a fixture carries a row array")
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("a row identifier").to_owned(),
                row.clone(),
            )
        })
        .collect()
}

/// Reads an unsigned member.
fn number(row: &Value, key: &str) -> u128 {
    row[key]
        .as_u64()
        .map(u128::from)
        .unwrap_or_else(|| panic!("member {key} is an unsigned number"))
}

/// Reads an array of unsigned members.
fn numbers(row: &Value, key: &str) -> Vec<u128> {
    row[key]
        .as_array()
        .unwrap_or_else(|| panic!("member {key} is an array"))
        .iter()
        .map(|value| {
            u128::from(
                value
                    .as_u64()
                    .unwrap_or_else(|| panic!("member {key} holds unsigned numbers")),
            )
        })
        .collect()
}

/// Reads an array of string members.
fn strings(row: &Value, key: &str) -> Vec<String> {
    row[key]
        .as_array()
        .unwrap_or_else(|| panic!("member {key} is an array"))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("member {key} holds strings"))
                .to_owned()
        })
        .collect()
}

/// Reads a boolean member.
fn flag(row: &Value, key: &str) -> bool {
    row[key]
        .as_bool()
        .unwrap_or_else(|| panic!("member {key} is a boolean"))
}

// ---------------------------------------------------------------------------
// Shared per-row derivations
// ---------------------------------------------------------------------------

/// Returns a code's generator coefficients as canonical base-field indices,
/// ascending in degree.
fn generator_indices<X, S, M>(code: &BchCode<X, S, M>) -> Vec<u128>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    code.generator().iter().map(bch_corpus_index).collect()
}

/// Returns a code's defining set as plain exponents.
fn defining_set<X, S, M>(code: &BchCode<X, S, M>) -> Vec<u128>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    code.defining_set()
        .iter()
        .map(|exponent| u128::from(exponent.get()))
        .collect()
}

/// Returns the order of a code's base field.
fn base_order<X, S, M>(code: &BchCode<X, S, M>) -> u64
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    u64::try_from(
        code.base_field_id()
            .order()
            .expect("a representable base order"),
    )
    .expect("a base order below 2^64")
}

/// Builds a symbol sequence from canonical base-field indices.
fn sequence_from<X, S, M>(code: &BchCode<X, S, M>, symbols: &[u128]) -> S
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let zero = BlockCode::symbol_zero(code);
    let mut sequence = S::zeroed(symbols.len(), &zero);
    for (index, &symbol) in symbols.iter().enumerate() {
        sequence
            .set(index, bch_corpus_element(&zero, symbol))
            .expect("an in-range coordinate");
    }
    sequence
}

/// Builds a polynomial from canonical base-field indices, ascending in degree.
fn polynomial_from<X, S, M>(code: &BchCode<X, S, M>, symbols: &[u128]) -> FieldPoly<X::Base>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let zero = BlockCode::symbol_zero(code);
    FieldPoly::new(
        symbols
            .iter()
            .map(|&symbol| bch_corpus_element(&zero, symbol))
            .collect(),
    )
}

/// Asserts that one oracle's two encodings of every corpus message agree with
/// `code`'s.
///
/// `native` holds the oracle's own cyclic encoding $c(x) = m(x)\,g(x)$ and
/// `systematic` the oracle's systematic codeword, both in canonical
/// coordinates under the corpus hex convention.
fn assert_codewords_agree<X, S, M>(
    oracle: &str,
    id: &str,
    code: &BchCode<X, S, M>,
    native: &[String],
    systematic: &[String],
) where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let order = base_order(code);
    let messages = bch_corpus_messages(code.k(), code.n(), order);
    assert_eq!(
        native.len(),
        messages.len(),
        "{oracle} encodes every message of row {id}"
    );
    assert_eq!(systematic.len(), messages.len(), "{id}");

    let zero = BlockCode::symbol_zero(code);
    let plan = code.systematic_plan(SystematicLayout::MessageParityAscending);
    for (position, message) in messages.iter().enumerate() {
        let indices: Vec<u128> = message.iter().map(|&symbol| u128::from(symbol)).collect();

        // The oracle's own encoder produces a codeword of the gf2 code and
        // reproduces the polynomial product against gf2's generator.
        let oracle_native = bch_corpus_decode_symbols(&native[position], code.n(), order);
        let native_poly = polynomial_from(
            code,
            &oracle_native
                .iter()
                .map(|&symbol| u128::from(symbol))
                .collect::<Vec<_>>(),
        );
        let (_, remainder) = native_poly.div_rem(code.generator());
        assert!(
            remainder.is_zero(),
            "{oracle} produced a word outside the gf2 code on row {id} message {position}"
        );
        let product = polynomial_from(code, &indices).mul(code.generator());
        for degree in 0..code.n() {
            assert_eq!(
                product.coeff_or_zero(degree, &zero),
                native_poly.coeff_or_zero(degree, &zero),
                "{oracle} differs from the polynomial encoding of row {id} \
                 message {position} at degree {degree}"
            );
        }

        // The oracle's systematic codeword equals the default-layout encoding
        // once the layout's coordinate map is applied.
        let oracle_systematic = bch_corpus_decode_symbols(&systematic[position], code.n(), order);
        let encoded = code
            .encode_systematic(
                &sequence_from(code, &indices),
                SystematicLayout::MessageParityAscending,
            )
            .expect("a validated message encodes");
        for user in 0..code.n() {
            let internal = plan
                .internal_coordinate(user)
                .expect("a user coordinate of this code");
            assert_eq!(
                encoded.get(user).expect("an in-range coordinate"),
                bch_corpus_element(&zero, u128::from(oracle_systematic[internal])),
                "{oracle} differs from the systematic encoding of row {id} \
                 message {position} at user coordinate {user}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Visitors
// ---------------------------------------------------------------------------

/// Checks that the corpus fixture records the code this suite constructs.
struct ConstructionRecord {
    corpus: BTreeMap<String, Value>,
    visited: Vec<String>,
}

impl BchCorpusVisitor for ConstructionRecord {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let record = self
            .corpus
            .get(row.id)
            .unwrap_or_else(|| panic!("the corpus fixture is missing row {}", row.id));
        self.visited.push(row.id.to_owned());

        let base_id = code.base_field_id();
        let ext_id = code.splitting_field_id();
        assert_eq!(record["construction"], row.construction, "{}", row.id);
        assert_eq!(
            number(record, "characteristic"),
            u128::from(base_id.characteristic()),
            "{}",
            row.id
        );
        assert_eq!(
            number(record, "base_degree"),
            base_id.degree() as u128,
            "{}",
            row.id
        );
        assert_eq!(
            number(record, "ext_degree"),
            ext_id.degree() as u128,
            "{}",
            row.id
        );
        assert_eq!(
            number(record, "relative_degree"),
            code.extension().relative_degree() as u128,
            "{}",
            row.id
        );
        assert_eq!(
            number(record, "base_order"),
            u128::from(base_order(code)),
            "{}",
            row.id
        );
        assert_eq!(number(record, "n"), code.n() as u128, "{}", row.id);
        assert_eq!(number(record, "k"), code.k() as u128, "{}", row.id);
        assert_eq!(
            number(record, "first_root"),
            u128::from(row.first_root),
            "{}",
            row.id
        );
        assert_eq!(
            number(record, "designed_distance"),
            u128::from(row.designed_distance),
            "{}",
            row.id
        );
        assert_eq!(
            number(record, "alpha"),
            bch_corpus_index(code.root()),
            "{} records the derived primitive root",
            row.id
        );
        assert_eq!(
            numbers(record, "defining_set"),
            defining_set(code),
            "{}",
            row.id
        );

        let base_modulus: Vec<u128> = base_id
            .modulus()
            .map(|modulus| {
                modulus
                    .coefficients()
                    .iter()
                    .map(|&c| u128::from(c))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(
            numbers(record, "base_modulus"),
            base_modulus,
            "{} records the selected base-field presentation",
            row.id
        );
    }
}

/// Checks that the seeded messages in the corpus fixture regenerate.
struct MessageDeterminism {
    corpus: BTreeMap<String, Value>,
}

impl BchCorpusVisitor for MessageDeterminism {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let record = &self.corpus[row.id];
        let order = base_order(code);
        let regenerated: Vec<String> = bch_corpus_messages(code.k(), code.n(), order)
            .iter()
            .map(|message| bch_corpus_encode_symbols(message, order))
            .collect();
        assert_eq!(
            strings(record, "messages"),
            regenerated,
            "{} messages must regenerate from the predeclared seed",
            row.id
        );
    }
}

/// Checks one oracle's generator polynomial, dimension and defining set.
struct GeneratorAgreement {
    oracle: &'static str,
    rows: BTreeMap<String, Value>,
    visited: Vec<String>,
}

impl BchCorpusVisitor for GeneratorAgreement {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let record = self
            .rows
            .get(row.id)
            .unwrap_or_else(|| panic!("{} produced no result for row {}", self.oracle, row.id));
        self.visited.push(row.id.to_owned());
        assert_eq!(
            numbers(record, "generator"),
            generator_indices(code),
            "{} disagrees with the constructed generator on row {}",
            self.oracle,
            row.id
        );
        assert_eq!(
            number(record, "k"),
            code.k() as u128,
            "{} disagrees with the constructed dimension on row {}",
            self.oracle,
            row.id
        );
        assert_eq!(
            numbers(record, "defining_set"),
            defining_set(code),
            "{} disagrees with the constructed defining set on row {}",
            self.oracle,
            row.id
        );
    }
}

/// Checks one oracle's codewords against the encoder, for the rows a tier
/// admits.
struct CodewordAgreement {
    oracle: &'static str,
    rows: BTreeMap<String, Value>,
    /// Only rows whose length satisfies this predicate are compared.
    large: bool,
    visited: Vec<String>,
}

impl BchCorpusVisitor for CodewordAgreement {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        if (code.n() > FAST_TIER_LENGTH) != self.large {
            return;
        }
        let record = &self.rows[row.id];
        self.visited.push(row.id.to_owned());
        assert_codewords_agree(
            self.oracle,
            row.id,
            code,
            &strings(record, "codewords_native"),
            &strings(record, "codewords_systematic"),
        );
    }
}

/// Checks GUAVA's unaided `BCHCode` object against the gf2 code carrying the
/// root GUAVA built it on.
///
/// GUAVA's `BCHCode` selects `PrimitiveUnityRoot(q, n)`. Where that element is
/// gf2's own $\alpha$ the compared code is the corpus row itself. Where it is
/// a different element of order $n$, the fixture carries it in gf2's
/// coordinates and this visitor constructs the gf2 code on that explicit root,
/// so GUAVA's own code object is the compared object on every row.
struct GuavaCodeObjectAgreement {
    rows: BTreeMap<String, Value>,
    visited: Vec<String>,
}

impl BchCorpusVisitor for GuavaCodeObjectAgreement {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let record = &self.rows[row.id];
        if !flag(record, "bchcode_built") {
            // The run's heap did not admit this row's code object;
            // `a_row_without_a_guava_code_object_records_the_derivation_and_the_attempt`
            // asserts what the row carries instead and that it is a row
            // Amendment 2 of the plan's `evidence-protocol` section names.
            return;
        }
        self.visited.push(row.id.to_owned());

        let guava_root = number(record, "guava_root_gf2_index");
        let rebuilt = if flag(record, "alpha_matches_guava_default") {
            assert_eq!(
                guava_root,
                bch_corpus_index(code.root()),
                "row {} records GUAVA's root as gf2's own",
                row.id
            );
            None
        } else {
            Some(
                BchCode::<X, S, M>::construct(BchSpec::NonPrimitiveConsecutive {
                    extension: code.extension().clone(),
                    length: bch_corpus_length(code.n() as u64),
                    root: RootSelection::Explicit(bch_corpus_element(code.root(), guava_root)),
                    first_root: RootExponent::from(row.first_root),
                    designed_distance: bch_corpus_distance(row.designed_distance),
                })
                .unwrap_or_else(|error| panic!("row {} at GUAVA's root: {error}", row.id)),
            )
        };
        let compared = rebuilt.as_ref().unwrap_or(code);

        assert_eq!(
            numbers(record, "bchcode_generator"),
            generator_indices(compared),
            "GUAVA's BCHCode disagrees with the constructed generator on row {}",
            row.id
        );
        assert_eq!(
            number(record, "bchcode_k"),
            compared.k() as u128,
            "GUAVA's BCHCode disagrees with the constructed dimension on row {}",
            row.id
        );
        assert_eq!(
            numbers(record, "bchcode_defining_set"),
            defining_set(compared),
            "GUAVA's BCHCode disagrees with the constructed defining set on row {}",
            row.id
        );
        assert_codewords_agree(
            "GUAVA's BCHCode",
            row.id,
            compared,
            &strings(record, "bchcode_codewords_native"),
            &strings(record, "bchcode_codewords_systematic"),
        );
    }
}

// ---------------------------------------------------------------------------
// The corpus agreement tests
// ---------------------------------------------------------------------------

/// Every corpus row the protocol predeclares.
const CORPUS_IDS: &[&str] = &["B1", "B2", "B3", "B4", "N1", "N2", "N3", "N4"];

/// The rows on which Amendment 2 of the plan's `evidence-protocol` section
/// admits a GUAVA result derived without a code object, because the generator
/// matrix `BCHCode` materializes does not fit the run's heap at that length.
/// Naming them here is what keeps the admission specific: a run that builds a
/// code object on one of these rows, or fails to build one on any other row,
/// fails `a_row_without_a_guava_code_object_records_the_derivation_and_the_attempt`
/// and `guava_bchcode_objects_agree_at_their_own_root`.
const ROWS_WITHOUT_A_GUAVA_CODE_OBJECT: &[&str] = &["B4"];

/// The corpus rows that carry GUAVA's own `BCHCode` object, in corpus order.
fn rows_with_a_guava_code_object() -> Vec<&'static str> {
    CORPUS_IDS
        .iter()
        .copied()
        .filter(|id| !ROWS_WITHOUT_A_GUAVA_CODE_OBJECT.contains(id))
        .collect()
}

#[test]
fn the_corpus_fixture_records_the_constructed_rows() {
    let mut visitor = ConstructionRecord {
        corpus: rows_by_id(&fixture("corpus.json")),
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut visitor);
    assert_eq!(visitor.visited, CORPUS_IDS);
}

#[test]
fn the_corpus_messages_regenerate_from_the_predeclared_seed() {
    let corpus = fixture("corpus.json");
    assert_eq!(
        corpus["seed"].as_str().expect("a recorded seed"),
        "0xAE03BCD0"
    );
    visit_bch_corpus(&mut MessageDeterminism {
        corpus: rows_by_id(&corpus),
    });
}

#[test]
fn sagemath_derives_the_same_generator_and_defining_set() {
    let mut visitor = GeneratorAgreement {
        oracle: "SageMath",
        rows: rows_by_id(&fixture("sage.json")),
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut visitor);
    assert_eq!(visitor.visited, CORPUS_IDS);
}

#[test]
fn gap_guava_derives_the_same_generator_and_defining_set() {
    let mut visitor = GeneratorAgreement {
        oracle: "GAP with GUAVA",
        rows: rows_by_id(&fixture("gap.json")),
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut visitor);
    assert_eq!(visitor.visited, CORPUS_IDS);
}

#[test]
fn sagemath_codewords_match_the_encoder() {
    let mut visitor = CodewordAgreement {
        oracle: "SageMath",
        rows: rows_by_id(&fixture("sage.json")),
        large: false,
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut visitor);
    assert_eq!(visitor.visited, ["B1", "B2", "B3", "N1", "N2", "N3", "N4"]);
}

#[test]
fn gap_guava_codewords_match_the_encoder() {
    let mut visitor = CodewordAgreement {
        oracle: "GAP with GUAVA",
        rows: rows_by_id(&fixture("gap.json")),
        large: false,
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut visitor);
    assert_eq!(visitor.visited, ["B1", "B2", "B3", "N1", "N2", "N3", "N4"]);
}

#[test]
fn guava_bchcode_objects_agree_at_their_own_root() {
    let mut visitor = GuavaCodeObjectAgreement {
        rows: rows_by_id(&fixture("gap.json")),
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut visitor);
    assert_eq!(visitor.visited, rows_with_a_guava_code_object());
}

/// The DVB-T2 mother row is compared separately because its codewords are
/// 65535 coordinates wide.
#[test]
fn oracle_codewords_match_the_encoder_at_the_mother_length() {
    for (oracle, name) in [("SageMath", "sage.json"), ("GAP with GUAVA", "gap.json")] {
        let mut visitor = CodewordAgreement {
            oracle,
            rows: rows_by_id(&fixture(name)),
            large: true,
            visited: Vec::new(),
        };
        visit_bch_corpus(&mut visitor);
        assert_eq!(visitor.visited, ["B4"]);
    }
}

#[test]
fn both_oracle_fixtures_cover_every_corpus_row() {
    for name in ["corpus.json", "sage.json", "gap.json"] {
        let ids: Vec<String> = rows_by_id(&fixture(name)).into_keys().collect();
        let mut expected: Vec<String> = CORPUS_IDS.iter().map(|id| (*id).to_owned()).collect();
        expected.sort();
        assert_eq!(ids, expected, "{name} covers the whole corpus");
    }
}

/// Asserts that `key` holds a non-empty string.
fn recorded<'a>(record: &'a Value, key: &str) -> &'a str {
    let value = record[key]
        .as_str()
        .unwrap_or_else(|| panic!("member {key} is a string"));
    assert!(!value.is_empty(), "member {key} is recorded");
    value
}

/// Members through which each oracle states the exact build it ran as. The
/// values are the run's own observations and live in the receipt; the suite
/// asserts that every one of them reached the fixture.
const SAGE_IDENTITY: &[&str] = &[
    "version",
    "library_version",
    "interpreter",
    "interpreter_executable",
];

/// [`SAGE_IDENTITY`] for GAP with GUAVA.
const GAP_IDENTITY: &[&str] = &[
    "gap_version",
    "gap_kernel_version",
    "gap_build_version",
    "gap_build_datetime",
    "gap_architecture",
    "gmp_version",
    "heap",
    "guava_version",
    "guava_path",
    "sonata_version",
    "sonata_path",
];

#[test]
fn the_oracle_fixtures_record_their_identities_and_self_checks() {
    let sage = fixture("sage.json");
    assert_eq!(sage["oracle"]["system"], "SageMath");
    for key in SAGE_IDENTITY {
        recorded(&sage["oracle"], key);
    }
    assert!(
        recorded(&sage["oracle"], "version").contains("SageMath version"),
        "the SageMath fixture records the library banner it ran under"
    );
    for row in sage["rows"].as_array().expect("rows") {
        let id = row["id"].as_str().expect("an identifier");
        let corpus = rows_by_id(&fixture("corpus.json"));
        assert_eq!(
            number(&row["transport_checks"], "alpha_order"),
            number(&corpus[id], "n"),
            "SageMath checked the transported root's order on row {id}"
        );
    }

    let gap = fixture("gap.json");
    assert_eq!(gap["oracle"]["system"], "GAP with GUAVA");
    for key in GAP_IDENTITY {
        recorded(&gap["oracle"], key);
    }
    for row in gap["rows"].as_array().expect("rows") {
        let id = row["id"].as_str().expect("an identifier");
        assert!(
            flag(row, "generator_divides_x_n_minus_one"),
            "GUAVA checked divisibility on row {id}"
        );
        assert!(
            flag(row, "defining_set_root_checked"),
            "GUAVA checked the defining set against the derived generator on row {id}"
        );

        // Every row calls GUAVA's own `BCHCode` and records what that attempt
        // observed, so whether a code object exists is a fact of the run. The
        // two marks are samples of one process-wide high-water mark, the first
        // taken after the `BCHCode` attempt and the second after the row's
        // `GeneratorPolCode` attempt, so the second covers both and neither
        // can fall below the other.
        recorded(row, "bchcode_attempt_heap");
        let bchcode_peak = number(row, "bchcode_attempt_peak_rss_kib");
        let row_peak = number(row, "row_attempts_peak_rss_kib");
        assert!(
            bchcode_peak > 0,
            "row {id} records the mark its `BCHCode` attempt is bounded by"
        );
        assert!(
            row_peak >= bchcode_peak,
            "row {id} samples one monotone mark, so the second sample is at least the first"
        );

        // GUAVA's `BCHCode` builds on `PrimitiveUnityRoot`. Its generator
        // equals the one derived at gf2's root exactly when the two roots
        // agree; where they differ the fixture carries both roots and both
        // generators, and `guava_bchcode_objects_agree_at_their_own_root`
        // compares the `BCHCode` object with the gf2 code on GUAVA's root.
        if !flag(row, "bchcode_built") {
            continue;
        }
        if flag(row, "alpha_matches_guava_default") {
            assert!(
                flag(row, "bchcode_generator_matches"),
                "row {id} uses GUAVA's own root, so its own generator must agree"
            );
            assert_eq!(
                numbers(row, "bchcode_generator"),
                numbers(row, "generator"),
                "row {id}"
            );
        } else {
            assert_ne!(
                number(row, "alpha_index"),
                number(row, "guava_default_root_index"),
                "row {id} records a distinct root"
            );
        }
    }
}

/// The encoding map the oracle records for a row whose code object the run's
/// heap did not admit.
const GUAVA_POLYNOMIAL_ENCODER: &str = "GUAVA cyclic-code encoding map c(x) = m(x) * G(x)";

/// A row whose `BCHCode` attempt exceeded the run's heap carries GUAVA's own
/// generator derivation and its cyclic-code polynomial encoding map, together
/// with what the attempt observed. This is the whole of the admitted shape:
/// there is no state in which a row simply has no GUAVA result, and the rows
/// that may take this shape are exactly the ones Amendment 2 of the plan's
/// `evidence-protocol` section names.
#[test]
fn a_row_without_a_guava_code_object_records_the_derivation_and_the_attempt() {
    let gap = fixture("gap.json");
    let mut without: Vec<&str> = Vec::new();
    for row in gap["rows"].as_array().expect("rows") {
        let id = row["id"].as_str().expect("an identifier");
        if flag(row, "bchcode_built") {
            continue;
        }
        without.push(id);
        assert!(
            row["bchcode_generator"].is_null(),
            "row {id} builds no code object, so it records no code-object generator"
        );
        assert!(
            !numbers(row, "generator").is_empty(),
            "row {id} records GUAVA's generator derivation"
        );
        assert_eq!(
            row["native_encoder"], GUAVA_POLYNOMIAL_ENCODER,
            "row {id} encodes through GUAVA's cyclic-code polynomial map"
        );
        let native = strings(row, "codewords_native");
        assert!(!native.is_empty(), "row {id} carries codewords");
        assert_eq!(strings(row, "codewords_systematic").len(), native.len());

        // What the bounded attempt itself observed, so the absence of a code
        // object is a measurement rather than an assertion.
        recorded(row, "bchcode_attempt_heap");
        assert!(
            number(row, "bchcode_attempt_cpu_ms") > 0,
            "row {id} records an attempt that actually ran"
        );
        assert!(
            number(row, "bchcode_attempt_peak_rss_kib") > 0,
            "row {id} records the mark its `BCHCode` attempt is bounded by"
        );
        assert!(
            number(row, "row_attempts_peak_rss_kib") > 0,
            "row {id} records the mark both of its attempts are bounded by"
        );
    }
    assert_eq!(
        without, ROWS_WITHOUT_A_GUAVA_CODE_OBJECT,
        "the rows without a GUAVA code object are exactly the amended ones"
    );
}

// ---------------------------------------------------------------------------
// The DVB-T2 standards vectors
// ---------------------------------------------------------------------------

/// Returns the ETSI-pinned normal-frame parameters the corpus row B4 uses.
fn dvb_normal_params() -> DvbBchParams {
    DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate1_2)
}

/// Rebuilds the canonical mother code the shortened DVB-T2 code of `params`
/// derives from.
fn dvb_mother_code_for(params: DvbBchParams) -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(gf2_core::gf2m::Gf2mField::new(
        params.field_m,
        params.primitive_poly,
    ))
    .expect("the ETSI field polynomial is primitive");
    BinaryBchCode::primitive_narrow_sense(extension, bch_corpus_distance(2 * params.t as u64 + 1))
        .expect("the DVB-T2 mother code")
}

/// Rebuilds the canonical DVB-T2 normal-frame mother code.
fn dvb_mother_code() -> BinaryBchCode {
    dvb_mother_code_for(dvb_normal_params())
}

/// Returns the ETSI generator product $g_1 \cdots g_t$ as ascending bits.
fn etsi_generator_bits() -> Vec<u128> {
    let params = dvb_normal_params();
    let field = gf2_core::gf2m::Gf2mField::new(params.field_m, params.primitive_poly);
    let product = product_of_generators(&field, NORMAL_GENERATORS, params.t);
    let degree = product.degree().expect("a nonzero ETSI generator product");
    (0..=degree)
        .map(|index| u128::from(product.coeff_or_zero(index, &field.zero()).is_one()))
        .collect()
}

#[test]
fn the_registry_selects_the_etsi_normal_frame_field_polynomial() {
    let params = dvb_normal_params();
    let selected = <BinaryPrimeExt as SelectExtension>::select(Fp::<2>::zero(), params.field_m)
        .expect("a degree-16 selection");
    assert_eq!(
        selected.field().primitive_polynomial(),
        params.primitive_poly,
        "the deterministic registry selection is the ETSI normal-frame polynomial"
    );
}

#[test]
fn the_mother_generator_is_the_etsi_generator_product() {
    let code = dvb_mother_code();
    assert_eq!(code.n(), 65535);
    assert_eq!(code.n() - code.k(), dvb_normal_params().m);
    assert_eq!(
        generator_indices(&code),
        etsi_generator_bits(),
        "the canonical mother generator is the ETSI EN 302 755 product g_1..g_12"
    );
}

#[test]
fn both_oracles_reproduce_the_etsi_generator_product() {
    let expected = etsi_generator_bits();
    for (oracle, name) in [("SageMath", "sage.json"), ("GAP with GUAVA", "gap.json")] {
        let rows = rows_by_id(&fixture(name));
        assert_eq!(
            numbers(&rows["B4"], "generator"),
            expected,
            "{oracle} reproduces the ETSI EN 302 755 generator product"
        );
    }
}

#[test]
fn the_shortened_dvb_t2_parity_matches_the_mother_code() {
    let params = dvb_normal_params();
    let mother = dvb_mother_code();
    let legacy = BchEncoder::new(LegacyBchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2));
    let shortening = mother.k() - params.k;

    for index in 0..DVB_PAYLOADS {
        let payload = BitVec::random_seeded(params.k, DVB_PAYLOAD_SEED + index as u64);
        let mut padded = BitVec::zeros(mother.k());
        for position in 0..params.k {
            padded.set(shortening + position, payload.get(position));
        }

        let shortened = legacy.encode(&payload);
        let full = mother
            .encode_systematic(&padded, SystematicLayout::MessageParityDescending)
            .expect("a validated message encodes");

        assert_eq!(shortened.len(), params.n);
        for parity in 0..params.m {
            assert_eq!(
                shortened.get(params.k + parity),
                full.get(mother.k() + parity),
                "payload {index} parity bit {parity} differs between the shortened \
                 ETSI generator and the canonical mother code"
            );
        }
    }
}

/// The ETSI verification and validation reference stream set this suite
/// consumes, whose configuration name fixes the frame size and code rate.
const ETSI_STREAM_SET: &str = "VV001-CR35";

/// Frames the VV001-CR35 streams carry.
const ETSI_STREAM_FRAMES: usize = 4;

/// Blocks each VV001-CR35 frame carries.
const ETSI_STREAM_BLOCKS_PER_FRAME: usize = 202;

/// Prefix of every line this case prints as an observed fact of its run.
///
/// `oracle/run.sh` copies these lines into the run receipt verbatim, so the
/// receipt's standards-vector figures come from the run rather than from
/// prose. See `@/inv/claims-trace-to-artifacts`.
const ETSI_FACT: &str = "dvb-vectors:";

/// Returns the SHA-256 of `path` as lowercase hex.
fn sha256_of(path: &Path) -> String {
    let mut file = File::open(path).expect("a stream file the loader read");
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).expect("the stream file digests");
    format!("{:x}", hasher.finalize())
}

/// Returns `path` relative to `base` when it lies under it, and `path` itself
/// otherwise.
fn under<'a>(path: &'a Path, base: &Path) -> &'a Path {
    path.strip_prefix(base).unwrap_or(path)
}

/// Describes the first coordinate at which an encoded block differs from its
/// verified counterpart, or `None` when the two agree.
fn first_difference(
    expected: &BitVec,
    payload: &BitVec,
    encoded: &BitVec,
    params: DvbBchParams,
    mother_k: usize,
) -> Option<String> {
    for bit in 0..params.k {
        if expected.get(bit) != payload.get(bit) {
            return Some(format!("message bit {bit}"));
        }
    }
    for parity in 0..params.m {
        if expected.get(params.k + parity) != encoded.get(mother_k + parity) {
            return Some(format!("parity bit {parity}"));
        }
    }
    None
}

#[test]
fn the_etsi_dvb_t2_streams_encode_to_their_verified_codewords() {
    let base = test_vectors_path();
    if !test_vectors_available() {
        println!("{ETSI_FACT} streams absent, comparison not run");
        println!("{ETSI_FACT} resolved_directory={}", base.display());
        eprintln!("set DVB_TEST_VECTORS_PATH to the directory holding VV001-CR35_CSP");
        return;
    }

    let set = TestVectorSet::load(&base, ETSI_STREAM_SET).expect("the VV001-CR35 stream set loads");
    let params = DvbBchParams::for_code(FrameSize::Normal, set.config.code_rate);
    let mother = dvb_mother_code_for(params);
    let shortening = mother.k() - params.k;

    let tp04 = set.tp04.as_ref().expect("VV001-CR35 carries test point 04");
    let tp05 = set.tp05.as_ref().expect("VV001-CR35 carries test point 05");
    assert_eq!(
        tp04.num_frames(),
        ETSI_STREAM_FRAMES,
        "the TP04 stream carries every frame"
    );
    assert_eq!(
        tp05.num_frames(),
        ETSI_STREAM_FRAMES,
        "the TP05 stream carries every frame"
    );

    let mut compared = 0usize;
    let mut agreeing = 0usize;
    let mut mismatches: Vec<String> = Vec::new();
    for frame in 0..ETSI_STREAM_FRAMES {
        let payloads = tp04.frame(frame);
        let codewords = tp05.frame(frame);
        assert_eq!(
            payloads.len(),
            ETSI_STREAM_BLOCKS_PER_FRAME,
            "frame {frame}"
        );
        assert_eq!(
            codewords.len(),
            ETSI_STREAM_BLOCKS_PER_FRAME,
            "frame {frame}"
        );

        for (block, payload) in payloads.iter().enumerate() {
            let expected = &codewords[block].data;
            assert_eq!(
                payload.data.len(),
                params.k,
                "frame {frame} block {block} carries K_bch payload bits"
            );
            assert_eq!(
                expected.len(),
                params.n,
                "frame {frame} block {block} carries N_bch codeword bits"
            );

            // The standard's shortening places the payload at the low message
            // coordinates of the descending transmission layout, so the mother
            // code sees zero symbols above it.
            let mut padded = BitVec::zeros(mother.k());
            for position in 0..params.k {
                padded.set(shortening + position, payload.data.get(position));
            }
            let full = mother
                .encode_systematic(&padded, SystematicLayout::MessageParityDescending)
                .expect("a validated message encodes");

            compared += 1;
            match first_difference(expected, &payload.data, &full, params, mother.k()) {
                Some(where_) => {
                    mismatches.push(format!("frame {frame} block {block} differs at {where_}"))
                }
                None => agreeing += 1,
            }
        }
    }

    // The facts the run receipt quotes, printed before the verdict so a
    // failing run still says what it read and how far the agreement went.
    println!("{ETSI_FACT} resolved_directory={}", base.display());
    println!("{ETSI_FACT} stream_set={ETSI_STREAM_SET}");
    for (label, stream) in [("tp04", tp04), ("tp05", tp05)] {
        println!(
            "{ETSI_FACT} {label}={} sha256={}",
            under(&stream.path, &base).display(),
            sha256_of(&stream.path)
        );
    }
    println!(
        "{ETSI_FACT} frames={ETSI_STREAM_FRAMES} blocks_per_frame={ETSI_STREAM_BLOCKS_PER_FRAME}"
    );
    println!("{ETSI_FACT} blocks_compared={compared} blocks_agreeing={agreeing}");

    assert!(
        mismatches.is_empty(),
        "{} of {compared} VV001-CR35 blocks differ from their verified codewords; first: {}",
        mismatches.len(),
        mismatches[0]
    );
    assert_eq!(compared, ETSI_STREAM_FRAMES * ETSI_STREAM_BLOCKS_PER_FRAME);
}
