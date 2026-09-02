//! One shared behavioral suite for the canonical block-code interfaces, run
//! over every implementation of them and over every base-field class the
//! library supports.
//!
//! # What this suite covers
//!
//! Four invariant groups, each written once as generic case functions and
//! applied to every code that carries the law:
//!
//! - **construction** — the derived dimension, generator, defining set,
//!   witnessed distance bound and correction radius of a completed BCH
//!   construction, and the typed errors an invalid request reports instead of
//!   a panic or a silent coercion.
//! - **encoding** — the systematic user layout, linearity, agreement of the
//!   trait, layout-explicit, caller-workspace, allocating-batch and
//!   caller-buffer-batch entry points, agreement of every registered encoding
//!   family with the scalar reference, and the allocation-free property of the
//!   repeated workspace path.
//! - **matrices** — the identity block of $G = [\,I_k \mid P\,]$, the
//!   parity-check layout $H = [\,-P^{\mathsf T} \mid I_{n-k}\,]$,
//!   $G H^{\mathsf T} = 0$, the rank of $H$, the shape errors the caller-buffer
//!   entry points report, and the equality of the opt-in cache wrapper with the
//!   code it wraps.
//! - **transformations** — the coordinate map's composition back to the mother
//!   code, the rank-derived dimension of a shortened or punctured code against
//!   a direct rank computation, membership of the derived codewords in the
//!   mother code, and the zero symbol sum of a one-symbol extension.
//!
//! # Rows and codes
//!
//! The rows are the predeclared conformance corpus of the `evidence-protocol`
//! section of `dev/active/ae03bcd0-general-bch/plan.md`: B1 to B4 over
//! $\mathrm{GF}(2)$, N1 over $\mathrm{GF}(3)$, N2 over $\mathrm{GF}(5)$, N3
//! over $\mathrm{GF}(9)$ and N4 over $\mathrm{GF}(2^8)$, constructed by
//! [`visit_bch_corpus`] and carrying the messages its seed `0xAE03BCD0` draws.
//! Every binary row runs in both representations: the packed
//! [`BinaryBchCode`] the corpus declares, and the field-generic
//! [`DenseBchCode`] rebuild [`bch_corpus_dense_twin`] produces from the same
//! construction. The mathematically valid boundary codes — full-space and
//! zero-dimensional — are constructed here over $\mathrm{GF}(2)$,
//! $\mathrm{GF}(3)$ and $\mathrm{GF}(9)$, and run the same cases.
//!
//! Beyond BCH, the cases run over every other implementor of the canonical
//! interfaces in this crate: [`LinearBlockCode`], the repetition code, the
//! three transformations, and the cache wrapper, which is what
//! `@/invariant/shared-test-contracts` asks of a shared interface. The
//! version-1 binary compatibility boundary is checked against the canonical
//! packed path wherever a packed code runs.
//!
//! # Tiers
//!
//! Everything runs in the fast tier except the DVB-T2 mother row's matrix
//! work. Its generator is $65343 \times 65535$, four orders of magnitude past
//! every other row: half a gigabyte per materialization and $4.3 \times 10^9$
//! cells in the identity-block walk, which leaves the case no margin against
//! the fast tier's per-test kill and no room beside the test binaries the tier
//! runs in parallel. It therefore carries a descriptive `#[ignore = "slow:
//! ..."]`, well inside the nightly tier's budget. That row's construction and
//! encoding, in both representations, stay in the fast tier.
//!
//! # Where implementation-specific tests remain
//!
//! This suite asserts only shared laws. Implementation-specific behavior stays
//! with its own module: the encoding dispatch seam's own corpus and worker
//! counts in `bch_encode_dispatch*.rs`, the matrix materialization's
//! allocation counts in `bch_matrix_allocation.rs`, external oracle and
//! standards-vector agreement in `bch_oracle_agreement.rs`, the primitive
//! search in `bch_primitive_verification.rs`, and the decoding laws of the
//! pre-cutover binary type in its own module. None of those assert a law this
//! suite asserts, so none is folded in here.
//!
//! The transformation cases are written against the canonical trait surface
//! only, so both of `Shortened`'s derivation paths — the general rank-derived
//! construction and a systematic fast path — run through the same assertions.

use gf2_coding::bch::error::BchError;
use gf2_coding::bch::spec::{
    BchCode, BchLength, BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance, RootExponent,
    RootSelection,
};
use gf2_coding::bch::{CachedMatrices, MatrixFill};
use gf2_coding::error::CodeError;
use gf2_coding::test_support::{
    self, bch_construction_contract, bch_corpus_dense_twin, bch_corpus_distance,
    bch_corpus_element, bch_corpus_length, bch_corpus_message_sequences, bch_encoding_contract,
    cached_matrices_contract, code_linearity_contract, coordinate_map_composition_contract,
    extension_contract, generic_ebch_16_11, matrix_shape_rejection_contract, packed_matrix_layout,
    packed_sequence_layout, puncturing_contract, shortening_contract, systematic_pair_contract,
    visit_bch_corpus, BchCorpusRow, BchCorpusVisitor,
};
use gf2_coding::traits::block::conformance::{self, RepetitionCode};
use gf2_coding::traits::block::{
    BlockCode, BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess, SymbolMatrix,
    SymbolSequence,
};
use gf2_coding::transform::{Extended, Punctured, Shortened};
use gf2_coding::LinearBlockCode;
use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension, FieldIdentity};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::modulus_select::select_modulus;
use gf2_core::field::{ConstField, FieldPoly, FieldVec, FiniteField};
use gf2_core::gf2m::Gf2mField;
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{QuotientElement, QuotientField};
use gf2_core::{BitMatrix, BitVec};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// The seed the evidence protocol predeclares for this corpus, reused for the
/// codes this suite constructs beside it.
const SEED: u64 = 0xAE03_BCD0;

/// Generator cells a row may carry and still run its matrix and transformation
/// cases in the fast tier.
///
/// Every corpus row but the DVB-T2 mother is at most $223 \times 255$; the
/// mother row is $65343 \times 65535$, which is 75000 times more cells, so the
/// threshold separates one row rather than tuning a budget.
const FAST_TIER_MATRIX_CELLS: usize = 1 << 20;

/// The word-boundary lengths every packed binary case runs at.
const WORD_BOUNDARY_LENGTHS: [usize; 5] = [0, 1, 63, 64, 65];

/// Generator rows the deferred row checks against their basis encodings.
const SAMPLED_GENERATOR_ROWS: usize = 16;

// ---------------------------------------------------------------------------
// Shared drivers
// ---------------------------------------------------------------------------

/// Returns the order of a symbol field.
fn field_order<F: FieldIdentity>(zero: &F) -> u128 {
    let id = zero.field_id();
    u128::from(id.characteristic()).pow(id.degree() as u32)
}

/// Draws `len` seeded symbols in the code's own representation.
fn seeded_symbols<C: BlockCode>(code: &C, len: usize, rng: &mut StdRng) -> C::Symbols {
    let zero = code.symbol_zero();
    let order = field_order(&zero);
    let mut symbols = C::Symbols::zeroed(len, &zero);
    for index in 0..len {
        let value = rng.gen_range(0..order);
        symbols
            .set(index, bch_corpus_element(&zero, value))
            .expect("an index below the requested length");
    }
    symbols
}

/// Returns a scalar that is not the multiplicative identity where the field has
/// one, so the homogeneity law is not vacuous outside $\mathrm{GF}(2)$.
fn nontrivial_scalar<F: FieldIdentity>(zero: &F) -> F {
    if field_order(zero) > 2 {
        bch_corpus_element(zero, 2)
    } else {
        zero.one_like()
    }
}

/// Every case an implementor of the encoder and generator capabilities runs.
fn encoder_and_generator_cases<C>(code: &C, messages: &[C::Symbols])
where
    C: BlockEncoder + GeneratorMatrixAccess,
{
    let zero = code.symbol_zero();
    let one = zero.one_like();
    for message in messages {
        conformance::block_encoder_contract(code, message);
    }
    conformance::generator_matrix_contract(code);
    conformance::generator_rows_encode_basis(code, &one);
    if messages.len() >= 2 {
        code_linearity_contract(code, &messages[0], &messages[1], &nontrivial_scalar(&zero));
    }
}

/// Every case an implementor of all three capabilities runs, whatever
/// message-coordinate order its layout records.
fn full_capability_cases<C>(code: &C, messages: &[C::Symbols])
where
    C: BlockEncoder + GeneratorMatrixAccess + ParityCheckMatrixAccess + Clone,
    C::GeneratorMatrix: Send + Sync,
    C::ParityCheckMatrix: Send + Sync,
{
    encoder_and_generator_cases(code, messages);
    conformance::parity_check_matrix_contract(code);
    conformance::generator_parity_orthogonality(code);
    matrix_shape_rejection_contract(code);
    cached_matrices_contract(code.clone());
    generator_has_full_row_rank(code);
    parity_check_has_full_row_rank(code);
    code.is_systematic()
        .expect("a code reports its message-coordinate layout");
}

/// Asserts that the generator's $k$ rows are independent, so the code has the
/// dimension it reports.
fn generator_has_full_row_rank<C>(code: &C)
where
    C: GeneratorMatrixAccess,
{
    assert_eq!(
        test_support::field_matrix_rank(&test_support::generator_as_field_matrix(code)),
        code.k(),
        "the generator has one independent row per dimension"
    );
}

/// [`full_capability_cases`] plus the canonical `[message | parity]` layout.
///
/// A code that records another message-coordinate order — `LinearBlockCode`
/// carries the Hamming systematic positions, which are not `0..k` — answers
/// [`GeneratorMatrixAccess::is_systematic`] for that order and runs the cases
/// above without this one.
fn canonical_layout_cases<C>(code: &C, messages: &[C::Symbols])
where
    C: BlockEncoder + GeneratorMatrixAccess + ParityCheckMatrixAccess + Clone,
    C::GeneratorMatrix: Send + Sync,
    C::ParityCheckMatrix: Send + Sync,
{
    full_capability_cases(code, messages);
    systematic_pair_contract(code);
}

/// Asserts that the parity check has the $n - k$ independent rows its contract
/// declares.
fn parity_check_has_full_row_rank<C>(code: &C)
where
    C: ParityCheckMatrixAccess,
{
    let parity = code
        .parity_check_matrix()
        .expect("the parity check materializes");
    let rows = code.parity_check_rows();
    assert_eq!(rows, code.n() - code.k(), "the redundancy is n - k");
    let mut dense = FieldMatrix::new(rows, code.n(), code.symbol_zero());
    for row in 0..rows {
        for col in 0..code.n() {
            dense.set(row, col, parity.get(row, col).expect("a parity cell"));
        }
    }
    assert_eq!(
        test_support::field_matrix_rank(&dense),
        rows,
        "the parity check has full row rank"
    );
}

/// Runs the transformation group over one mother code.
fn transformation_cases<C>(mother: &C, messages: &[C::Symbols])
where
    C: BlockEncoder + GeneratorMatrixAccess + ParityCheckMatrixAccess + Clone,
{
    let length = mother.n();
    extension_contract(mother, messages);

    // Coordinate sets spanning both blocks of the systematic layout: a message
    // coordinate, a parity coordinate, and the last coordinate.
    let removed = shared_coordinate_set(mother);
    shortening_contract(mother, &removed);
    puncturing_contract(mother, &removed);
    if length >= 4 {
        coordinate_map_composition_contract(mother, &[0], &[1]);
    }

    // The complete coordinate set is the zero-length boundary of both
    // transformations.
    let all: Vec<usize> = (0..length).collect();
    shortening_contract(mother, &all);
    puncturing_contract(mother, &all);

    let shortened =
        Shortened::new(mother.clone(), removed.iter().copied()).expect("a valid coordinate set");
    let shortened_messages = basis_messages(&shortened);
    encoder_and_generator_cases(&shortened, &shortened_messages);

    let punctured =
        Punctured::new(mother.clone(), removed.iter().copied()).expect("a valid coordinate set");
    let punctured_messages = basis_messages(&punctured);
    encoder_and_generator_cases(&punctured, &punctured_messages);

    let extended = Extended::new(mother.clone()).expect("a mother code extends");
    encoder_and_generator_cases(&extended, messages);
    conformance::parity_check_matrix_contract(&extended);
    conformance::generator_parity_orthogonality(&extended);
    extension_row_is_all_ones(&extended);

    // Chaining keeps the provenance: a shortened code extends, and an extended
    // code punctures back to the mother length.
    let chained = shortened
        .clone()
        .extend()
        .expect("a shortened code extends");
    assert_eq!(chained.n(), shortened.n() + 1);
    assert_eq!(
        chained.coordinate_map().mother_len(),
        length,
        "the chained map still reaches the original mother"
    );
    let repunctured = Punctured::new(extended, [length]).expect("the fresh coordinate is valid");
    assert_eq!(repunctured.n(), length, "puncturing undoes the extension");
    assert_eq!(repunctured.k(), mother.k());
}

/// Asserts that the extension's own parity-check row checks the zero-sum
/// constraint across every coordinate.
fn extension_row_is_all_ones<C>(extended: &Extended<C>)
where
    C: BlockCode + ParityCheckMatrixAccess,
{
    let parity = extended
        .parity_check_matrix()
        .expect("the parity check materializes");
    let one = extended.symbol_zero().one_like();
    let row = extended.parity_check_rows() - 1;
    for col in 0..extended.n() {
        assert_eq!(
            parity.get(row, col),
            Some(one.clone()),
            "the extension check is one at coordinate {col}"
        );
    }
}

/// Returns the coordinate set the transformation cases remove: a message
/// coordinate, a parity coordinate, and the final coordinate.
fn shared_coordinate_set<C: BlockCode>(code: &C) -> Vec<usize> {
    let mut coordinates = vec![0usize];
    if code.k() < code.n() {
        coordinates.push(code.k());
    }
    if code.n() >= 1 {
        coordinates.push(code.n() - 1);
    }
    coordinates.sort_unstable();
    coordinates.dedup();
    coordinates
}

/// Returns the message basis of a code, which is the fixture a derived code
/// carries when its dimension is not the mother's.
fn basis_messages<C: BlockCode>(code: &C) -> Vec<C::Symbols> {
    let zero = code.symbol_zero();
    let one = zero.one_like();
    (0..code.k().min(3))
        .map(|index| {
            let mut message = C::Symbols::zeroed(code.k(), &zero);
            message
                .set(index, one.clone())
                .expect("an index below the dimension");
            message
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Corpus visitors
// ---------------------------------------------------------------------------

/// Runs the construction group on every corpus row, in both representations
/// where the row admits both.
struct ConstructionCases {
    visited: Vec<String>,
}

impl BchCorpusVisitor for ConstructionCases {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        X::Base: Send + Sync + 'static,
        S: gf2_coding::bch::encode::SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync,
    {
        self.visited.push(row.id.to_owned());
        bch_construction_contract(code, row.designed_distance);
        let twin = bch_corpus_dense_twin(row, code);
        bch_construction_contract(&twin, row.designed_distance);
    }
}

/// Runs the encoding group on every corpus row, in both representations.
struct EncodingCases {
    visited: Vec<String>,
}

impl BchCorpusVisitor for EncodingCases {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        X::Base: Send + Sync + 'static,
        S: gf2_coding::bch::encode::SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync,
    {
        self.visited.push(row.id.to_owned());
        let messages = bch_corpus_message_sequences(code);
        bch_encoding_contract(code, &messages);
        for message in &messages {
            conformance::block_encoder_contract(code, message);
        }
        let zero = code.symbol_zero();
        code_linearity_contract(code, &messages[0], &messages[1], &nontrivial_scalar(&zero));

        let twin = bch_corpus_dense_twin(row, code);
        let twin_messages = bch_corpus_message_sequences(&twin);
        bch_encoding_contract(&twin, &twin_messages);

        // The two representations of one row encode the same symbols.
        for (packed, dense) in messages.iter().zip(&twin_messages) {
            let left = code.encode(packed).expect("a corpus message encodes");
            let right = twin.encode(dense).expect("a corpus message encodes");
            for index in 0..code.n() {
                assert_eq!(
                    SymbolSequence::get(&left, index),
                    SymbolSequence::get(&right, index),
                    "{} coordinate {index} agrees across representations",
                    row.id
                );
            }
        }
    }
}

/// Runs the matrix group on every corpus row whose generator fits the fast
/// tier, in both representations.
struct MatrixCases {
    visited: Vec<String>,
    deferred: Vec<String>,
}

impl BchCorpusVisitor for MatrixCases {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        X::Base: Send + Sync + 'static,
        S: gf2_coding::bch::encode::SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync,
    {
        if code.k().saturating_mul(code.n()) > FAST_TIER_MATRIX_CELLS {
            self.deferred.push(row.id.to_owned());
            return;
        }
        self.visited.push(row.id.to_owned());
        let messages = bch_corpus_message_sequences(code);
        canonical_layout_cases(code, &messages);
        assert_eq!(
            code.generator_matrix().expect("the generator materializes"),
            {
                let mut oracle = M::zeroed(code.k(), code.n(), &code.symbol_zero());
                test_support::bch_generator_matrix_by_encoding(code, &mut oracle)
                    .expect("the basis-vector oracle writes the generator");
                oracle
            },
            "{} generator equals the basis-vector oracle",
            row.id
        );
        assert_eq!(
            code.parity_check_matrix()
                .expect("the parity check materializes"),
            {
                let mut oracle = M::zeroed(code.parity_check_rows(), code.n(), &code.symbol_zero());
                test_support::bch_parity_check_matrix_by_encoding(code, &mut oracle)
                    .expect("the basis-vector oracle writes the parity check");
                oracle
            },
            "{} parity check equals the basis-vector oracle",
            row.id
        );

        let twin = bch_corpus_dense_twin(row, code);
        let twin_messages = bch_corpus_message_sequences(&twin);
        canonical_layout_cases(&twin, &twin_messages);
    }
}

/// Runs the transformation group on every corpus row whose generator fits the
/// fast tier.
struct TransformationCases {
    visited: Vec<String>,
    deferred: Vec<String>,
}

impl BchCorpusVisitor for TransformationCases {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        X::Base: Send + Sync + 'static,
        S: gf2_coding::bch::encode::SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync,
    {
        if code.k().saturating_mul(code.n()) > FAST_TIER_MATRIX_CELLS {
            self.deferred.push(row.id.to_owned());
            return;
        }
        self.visited.push(row.id.to_owned());
        let messages = bch_corpus_message_sequences(code);
        transformation_cases(code, &messages);
    }
}

/// The corpus row identifiers, in corpus order.
const CORPUS_ROWS: [&str; 8] = ["B1", "B2", "B3", "B4", "N1", "N2", "N3", "N4"];

/// The rows whose matrix and transformation work is deferred to the slow tier.
const DEFERRED_ROWS: [&str; 1] = ["B4"];

/// The rows whose matrix and transformation work runs in the fast tier.
fn fast_tier_rows() -> Vec<String> {
    CORPUS_ROWS
        .iter()
        .filter(|id| !DEFERRED_ROWS.contains(id))
        .map(|id| (*id).to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// Corpus cases
// ---------------------------------------------------------------------------

#[test]
fn construction_invariants_hold_on_every_corpus_row() {
    let mut cases = ConstructionCases {
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut cases);
    assert_eq!(cases.visited, CORPUS_ROWS, "every corpus row is visited");
}

#[test]
fn encoding_invariants_hold_on_every_corpus_row() {
    let mut cases = EncodingCases {
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut cases);
    assert_eq!(cases.visited, CORPUS_ROWS, "every corpus row is visited");
}

#[test]
fn matrix_invariants_hold_on_every_corpus_row() {
    let mut cases = MatrixCases {
        visited: Vec::new(),
        deferred: Vec::new(),
    };
    visit_bch_corpus(&mut cases);
    assert_eq!(cases.visited, fast_tier_rows());
    assert_eq!(cases.deferred, DEFERRED_ROWS);
}

#[test]
fn transformation_invariants_hold_on_every_corpus_row() {
    let mut cases = TransformationCases {
        visited: Vec::new(),
        deferred: Vec::new(),
    };
    visit_bch_corpus(&mut cases);
    assert_eq!(cases.visited, fast_tier_rows());
    assert_eq!(cases.deferred, DEFERRED_ROWS);
}

// ---------------------------------------------------------------------------
// The DVB-T2 mother row's matrices
// ---------------------------------------------------------------------------

/// Runs the matrix group on the deferred corpus row.
///
/// The row's generator is $65343 \times 65535$ packed bits, half a gigabyte
/// per materialization, so the identity-block walk alone reads $4.3 \times
/// 10^9$ cells. Orthogonality runs in the systematic-pair form, whose
/// $O(k(n-k))$ cost is what makes the statement reachable at this size at all;
/// the direct triple loop over the same pair would be $O(k(n-k)n)$.
struct DeferredMatrixCases {
    visited: Vec<String>,
}

impl BchCorpusVisitor for DeferredMatrixCases {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        X::Base: Send + Sync + 'static,
        S: gf2_coding::bch::encode::SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync,
    {
        if !DEFERRED_ROWS.contains(&row.id) {
            return;
        }
        self.visited.push(row.id.to_owned());
        conformance::generator_matrix_contract(code);
        conformance::parity_check_matrix_contract(code);
        systematic_pair_contract(code);
        matrix_shape_rejection_contract(code);
        parity_check_has_full_row_rank(code);
        for message in &bch_corpus_message_sequences(code) {
            let codeword = code.encode(message).expect("a corpus message encodes");
            test_support::assert_is_codeword(code, &codeword);
        }

        // Row $i$ of the generator is the encoding of message basis vector
        // $i$. Walking all 65343 rows is 65343 encodes of a 65535-symbol
        // codeword; a seeded sample carries the same statement at a size this
        // row admits, and every other corpus row runs the full walk.
        let generator = code.generator_matrix().expect("the generator materializes");
        let zero = code.symbol_zero();
        let one = zero.one_like();
        let mut rng = StdRng::seed_from_u64(SEED);
        for _ in 0..SAMPLED_GENERATOR_ROWS {
            let index = rng.gen_range(0..code.k());
            let mut basis = S::zeroed(code.k(), &zero);
            basis
                .set(index, one.clone())
                .expect("an index below the dimension");
            let encoded = code.encode(&basis).expect("a basis message encodes");
            for col in 0..code.n() {
                assert_eq!(
                    SymbolSequence::get(&encoded, col),
                    generator.get(index, col),
                    "generator row {index} is the encoding of its basis vector"
                );
            }
        }
    }
}

#[test]
#[ignore = "slow: the DVB-T2 mother row materializes a 65343 x 65535 generator, over a gigabyte per pass, and walks 4.3e9 identity-block cells"]
fn matrix_invariants_hold_on_the_dvb_t2_mother_row() {
    let mut cases = DeferredMatrixCases {
        visited: Vec::new(),
    };
    visit_bch_corpus(&mut cases);
    assert_eq!(cases.visited, DEFERRED_ROWS);
}

// ---------------------------------------------------------------------------
// Boundary codes
// ---------------------------------------------------------------------------

/// Returns the binary splitting field the boundary codes are built in.
fn binary_extension(degree: usize, modulus: u64) -> BinaryPrimeExt {
    BinaryPrimeExt::new(Gf2mField::new(degree, modulus)).expect("a primitive binary polynomial")
}

/// Builds a binary boundary code of length `length` requesting `distance`.
fn binary_boundary(length: u64, distance: u64) -> BinaryBchCode {
    BinaryBchCode::construct(BchSpec::NonPrimitiveConsecutive {
        extension: binary_extension(4, 0b10011),
        length: bch_corpus_length(length),
        root: RootSelection::Canonical,
        first_root: RootExponent::from(0),
        designed_distance: bch_corpus_distance(distance),
    })
    .expect("a valid binary boundary spec")
}

/// Builds a prime-base boundary code over $\mathrm{GF}(3)$ at length 13.
fn prime_boundary(distance: u64) -> DenseBchCode<QuotientField<Fp<3>>> {
    DenseBchCode::<QuotientField<Fp<3>>>::consecutive_roots_auto(
        Fp::<3>::zero(),
        3,
        bch_corpus_length(13),
        RootExponent::from(0),
        bch_corpus_distance(distance),
    )
    .expect("a valid GF(3) boundary spec")
}

/// Builds an extension-base boundary code over $\mathrm{GF}(9)$ at length 10.
fn extension_boundary(distance: u64) -> DenseBchCode<QuotientField<QuotientElement<Fp<3>>>> {
    let modulus = select_modulus(&Fp::<3>::zero(), 2).expect("a GF(9) modulus");
    let gf9 = QuotientField::new(Fp::<3>::zero(), modulus).expect("GF(9)");
    DenseBchCode::<QuotientField<QuotientElement<Fp<3>>>>::consecutive_roots_auto(
        gf9.ext_zero(),
        2,
        bch_corpus_length(10),
        RootExponent::from(0),
        bch_corpus_distance(distance),
    )
    .expect("a valid GF(9) boundary spec")
}

/// Runs every applicable case on one boundary code.
fn boundary_cases<X, S, M>(code: &BchCode<X, S, M>, designed_distance: u64, rng: &mut StdRng)
where
    X: FieldExtension,
    X::Base: 'static,
    S: gf2_coding::bch::encode::SystematicKernel<X::Base>,
    M: MatrixFill<X::Base> + Send + Sync,
{
    bch_construction_contract(code, designed_distance);
    let messages: Vec<S> = (0..2)
        .map(|_| seeded_symbols(code, code.k(), rng))
        .collect();
    bch_encoding_contract(code, &messages);
    canonical_layout_cases(code, &messages);
    transformation_cases(code, &messages);
}

#[test]
fn boundary_codes_run_the_shared_cases_over_every_field_class() {
    let mut rng = StdRng::seed_from_u64(SEED);

    let full_space = binary_boundary(5, 1);
    assert_eq!((full_space.n(), full_space.k()), (5, 5));
    assert_eq!(full_space.generator().degree(), Some(0));
    boundary_cases(&full_space, 1, &mut rng);

    let zero_dimensional = binary_boundary(5, 6);
    assert_eq!((zero_dimensional.n(), zero_dimensional.k()), (5, 0));
    boundary_cases(&zero_dimensional, 6, &mut rng);

    let full_space = prime_boundary(1);
    assert_eq!((full_space.n(), full_space.k()), (13, 13));
    boundary_cases(&full_space, 1, &mut rng);

    let zero_dimensional = prime_boundary(14);
    assert_eq!((zero_dimensional.n(), zero_dimensional.k()), (13, 0));
    boundary_cases(&zero_dimensional, 14, &mut rng);

    let full_space = extension_boundary(1);
    assert_eq!((full_space.n(), full_space.k()), (10, 10));
    boundary_cases(&full_space, 1, &mut rng);

    let zero_dimensional = extension_boundary(11);
    assert_eq!((zero_dimensional.n(), zero_dimensional.k()), (10, 0));
    boundary_cases(&zero_dimensional, 11, &mut rng);
}

// ---------------------------------------------------------------------------
// Packed word boundaries
// ---------------------------------------------------------------------------

#[test]
fn packed_representations_hold_the_contract_at_word_boundaries() {
    conformance::symbol_sequence_contract::<Fp<2>, BitVec>(&Fp::<2>::new(0), &Fp::<2>::new(1));
    conformance::symbol_matrix_contract::<Fp<2>, BitMatrix>(&Fp::<2>::new(0), &Fp::<2>::new(1));
    conformance::symbol_sequence_contract::<Fp<2>, FieldVec<Fp<2>>>(
        &Fp::<2>::new(0),
        &Fp::<2>::new(1),
    );
    conformance::symbol_matrix_contract::<Fp<2>, FieldMatrix<Fp<2>>>(
        &Fp::<2>::new(0),
        &Fp::<2>::new(1),
    );
}

#[test]
fn packed_binary_codes_hold_the_word_boundary_layout() {
    let mut rng = StdRng::seed_from_u64(SEED);
    // The GF(2^8) primitive narrow-sense row, whose length 255 is above every
    // word boundary the case reduces to.
    let mother = BinaryBchCode::<u64>::primitive_narrow_sense_auto(
        Fp::<2>::zero(),
        8,
        bch_corpus_distance(9),
    )
    .expect("the binary mother of the word-boundary cases");
    assert_packed_layout(&mother, &mut rng);

    for length in WORD_BOUNDARY_LENGTHS {
        let removed: Vec<usize> = (length..mother.n()).collect();

        let shortened = Shortened::new(mother.clone(), removed.iter().copied())
            .expect("a valid coordinate set");
        assert_eq!(shortened.n(), length, "shortening reaches length {length}");
        assert_packed_layout(&shortened, &mut rng);

        let punctured = Punctured::new(mother.clone(), removed.iter().copied())
            .expect("a valid coordinate set");
        assert_eq!(punctured.n(), length, "puncturing reaches length {length}");
        assert_packed_layout(&punctured, &mut rng);

        // The extension of a length `length - 1` code is a length `length`
        // packed code built by the third transformation.
        if length > 0 {
            let shorter: Vec<usize> = (length - 1..mother.n()).collect();
            let extended = Extended::new(
                Shortened::new(mother.clone(), shorter).expect("a valid coordinate set"),
            )
            .expect("a shortened code extends");
            assert_eq!(extended.n(), length);
            assert_packed_layout(&extended, &mut rng);
        }
    }
}

/// Asserts the packed layout of everything `code` writes: its codewords, its
/// generator, and its parity check where it has one.
fn assert_packed_layout<C>(code: &C, rng: &mut StdRng)
where
    C: BlockCode<Symbol = Fp<2>, Symbols = BitVec>
        + BlockEncoder
        + GeneratorMatrixAccess<GeneratorMatrix = BitMatrix>,
{
    let message = seeded_symbols(code, code.k(), rng);
    let codeword = code.encode(&message).expect("a message encodes");
    assert_eq!(codeword.len(), code.n());
    packed_sequence_layout(&message);
    packed_sequence_layout(&codeword);
    packed_matrix_layout(&code.generator_matrix().expect("the generator materializes"));
    conformance::block_encoder_contract(code, &message);
    conformance::binary_v1_encoder_agrees(code, &message);
}

// ---------------------------------------------------------------------------
// The rest of the implementor roster
// ---------------------------------------------------------------------------

#[test]
fn every_canonical_implementor_runs_the_capability_cases() {
    let mut rng = StdRng::seed_from_u64(SEED);

    let hamming = LinearBlockCode::hamming(3);
    let messages: Vec<BitVec> = (0..2)
        .map(|_| seeded_symbols(&hamming, hamming.k(), &mut rng))
        .collect();
    full_capability_cases(&hamming, &messages);
    transformation_cases(&hamming, &messages);
    for message in &messages {
        conformance::binary_v1_encoder_agrees(&hamming, message);
    }

    let repetition = RepetitionCode::new(5, Fp::<2>::new(0));
    let repetition_messages = basis_messages(&repetition);
    encoder_and_generator_cases(&repetition, &repetition_messages);
    conformance::parity_check_matrix_contract(&repetition);
    conformance::generator_parity_orthogonality(&repetition);

    let repetition = RepetitionCode::new(4, Fp::<7>::new(0));
    let repetition_messages = basis_messages(&repetition);
    encoder_and_generator_cases(&repetition, &repetition_messages);
    conformance::parity_check_matrix_contract(&repetition);
    conformance::generator_parity_orthogonality(&repetition);

    let ebch = generic_ebch_16_11();
    let ebch_messages: Vec<BitVec> = (0..2)
        .map(|_| seeded_symbols(&ebch, ebch.k(), &mut rng))
        .collect();
    encoder_and_generator_cases(&ebch, &ebch_messages);
    conformance::parity_check_matrix_contract(&ebch);
    conformance::generator_parity_orthogonality(&ebch);
    extension_row_is_all_ones(&ebch);

    // The cache wrapper is itself an implementor of both matrix capabilities.
    let cached = CachedMatrices::new(hamming.clone());
    conformance::generator_matrix_contract(&cached);
    conformance::parity_check_matrix_contract(&cached);
    conformance::generator_parity_orthogonality(&cached);
    matrix_shape_rejection_contract(&cached);
    assert_eq!(
        cached
            .is_systematic()
            .expect("the wrapper reports a layout"),
        hamming.is_systematic().expect("the code reports a layout"),
        "the cache wrapper reports the wrapped layout"
    );
}

// ---------------------------------------------------------------------------
// Typed construction errors
// ---------------------------------------------------------------------------

#[test]
fn invalid_constructions_report_typed_errors_over_every_field_class() {
    // A zero length and a zero designed distance are rejected by their own
    // newtypes before a spec exists.
    assert!(matches!(
        BchLength::try_from(0),
        Err(BchError::InvalidLength { length: 0 })
    ));
    assert!(matches!(
        DesignedDistance::try_from(0),
        Err(BchError::InvalidDesignedDistance {
            designed_distance: 0
        })
    ));

    let extension = binary_extension(4, 0b10011);

    // The primitive length belongs to the primitive variants.
    assert!(matches!(
        BinaryBchCode::consecutive_roots(
            extension.clone(),
            bch_corpus_length(15),
            RootSelection::Canonical,
            RootExponent::from(1),
            bch_corpus_distance(3),
        ),
        Err(BchError::LengthNotProperlyNonPrimitive { length: 15, .. })
    ));

    // A length that is not a divisor of the unit group has no root of that
    // exact order.
    assert!(matches!(
        BinaryBchCode::consecutive_roots(
            extension.clone(),
            bch_corpus_length(4),
            RootSelection::Canonical,
            RootExponent::from(1),
            bch_corpus_distance(3),
        ),
        Err(BchError::LengthNotCoprimeToCharacteristic { length: 4, .. })
            | Err(BchError::LengthDoesNotDivideUnitGroup { length: 4, .. })
    ));

    // An explicit root of the wrong exact order is rejected rather than used.
    assert!(matches!(
        BinaryBchCode::consecutive_roots(
            extension.clone(),
            bch_corpus_length(5),
            RootSelection::Explicit(extension.field().one()),
            RootExponent::from(1),
            bch_corpus_distance(3),
        ),
        Err(BchError::RootOrderMismatch { expected: 5, .. })
    ));

    // A root exponent outside the residue range, and a designed distance
    // outside `1..=n + 1`.
    assert!(matches!(
        BinaryBchCode::consecutive_roots(
            extension.clone(),
            bch_corpus_length(5),
            RootSelection::Canonical,
            RootExponent::from(5),
            bch_corpus_distance(3),
        ),
        Err(BchError::RootExponentOutOfRange {
            exponent: 5,
            length: 5
        })
    ));
    assert!(matches!(
        BinaryBchCode::consecutive_roots(
            extension.clone(),
            bch_corpus_length(5),
            RootSelection::Canonical,
            RootExponent::from(0),
            bch_corpus_distance(7),
        ),
        Err(BchError::DesignedDistanceOutOfRange {
            designed_distance: 7,
            length: 5
        })
    ));

    // An explicit generator whose coefficients leave the base field, and one
    // that does not divide the cyclic polynomial.
    let field = extension.field().clone();
    let outside = FieldPoly::new(vec![field.element(2), field.one()]);
    assert!(matches!(
        BinaryBchCode::from_generator(extension.clone(), bch_corpus_length(15), outside),
        Err(BchError::GeneratorCoefficientNotInBase { index: 0 })
    ));
    // (x + 1)^2 has a repeated root, and x^15 - 1 is squarefree over GF(2).
    let indivisible = FieldPoly::new(vec![field.one(), field.zero(), field.one()]);
    assert!(matches!(
        BinaryBchCode::from_generator(extension.clone(), bch_corpus_length(15), indivisible),
        Err(BchError::GeneratorNotDivisorOfCyclicPolynomial { length: 15 })
    ));

    // The same contract over a prime base and over an extension base: a
    // designed distance above `n + 1` is a typed error, never a panic.
    assert!(matches!(
        DenseBchCode::<QuotientField<Fp<3>>>::consecutive_roots_auto(
            Fp::<3>::zero(),
            3,
            bch_corpus_length(13),
            RootExponent::from(0),
            bch_corpus_distance(15),
        ),
        Err(BchError::DesignedDistanceOutOfRange {
            designed_distance: 15,
            length: 13
        })
    ));
    let modulus = select_modulus(&Fp::<3>::zero(), 2).expect("a GF(9) modulus");
    let gf9 = QuotientField::new(Fp::<3>::zero(), modulus).expect("GF(9)");
    assert!(matches!(
        DenseBchCode::<QuotientField<QuotientElement<Fp<3>>>>::consecutive_roots_auto(
            gf9.ext_zero(),
            2,
            bch_corpus_length(10),
            RootExponent::from(12),
            bch_corpus_distance(3),
        ),
        Err(BchError::RootExponentOutOfRange {
            exponent: 12,
            length: 10
        })
    ));

    // A length sharing a factor with the characteristic is rejected before any
    // derivation runs.
    assert!(matches!(
        DenseBchCode::<QuotientField<Fp<3>>>::consecutive_roots_auto(
            Fp::<3>::zero(),
            3,
            bch_corpus_length(6),
            RootExponent::from(0),
            bch_corpus_distance(3),
        ),
        Err(BchError::LengthNotCoprimeToCharacteristic {
            length: 6,
            characteristic: 3
        })
    ));
}

// ---------------------------------------------------------------------------
// Coordinate-map and error-surface details the transformations promise
// ---------------------------------------------------------------------------

#[test]
fn transformations_reject_invalid_coordinate_sets_with_typed_errors() {
    let code = LinearBlockCode::hamming(3);
    assert!(matches!(
        Shortened::new(code.clone(), [code.n()]),
        Err(CodeError::CoordinateOutOfRange { .. })
    ));
    assert!(matches!(
        Shortened::new(code.clone(), [0, 0]),
        Err(CodeError::DuplicateCoordinate { .. })
    ));
    assert!(matches!(
        Punctured::new(code.clone(), [code.n()]),
        Err(CodeError::CoordinateOutOfRange { .. })
    ));
    assert!(matches!(
        Punctured::new(code.clone(), [1, 1]),
        Err(CodeError::DuplicateCoordinate { .. })
    ));
}
