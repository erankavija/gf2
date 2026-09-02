//! Test helpers shared between crate-internal unit tests, the integration
//! tests under `tests/`, and the bench targets.
//!
//! The helpers are allocation witnesses over the encoding workspaces, the
//! kernel-selection controls of the kernel-dispatched encode families, the
//! basis-vector matrix oracles the canonical materialization is measured and
//! compared against, the predeclared BCH conformance corpus with its seeded
//! messages, and a reader for the ETSI DVB-T2 verified vectors (the
//! `VV001-CR35_CSP/TestPoint*/...CSP.txt` files).
//!
//! Gated behind `cfg(any(test, feature = "test-support"))` so the helpers
//! are reachable from both unit tests inside the crate and integration
//! tests under `tests/` (which import them by enabling the
//! `test-support` feature on the dev-dependency self-reference).
//!
//! All helpers here are test-only — production code must not call them.

#![cfg(any(test, feature = "test-support"))]

use crate::bch::dvb_t2::{DvbBchParams, FrameSize};
use crate::bch::encode::{
    BchEncodeWorkspace, EncodeFamily, EncodeRegisters, SystematicKernel, SystematicLayout,
};
use crate::bch::matrix::{CachedMatrices, MatrixFill};
use crate::bch::spec::{
    BchCode, BchLength, BinaryBchCode, DenseBchCode, DesignedDistance, RootExponent, RootSelection,
};
use crate::error::CodeError;
use crate::product::ExtendedBchComponent;
use crate::traits::block::{
    BlockCode, BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess, SymbolMatrix,
    SymbolSequence,
};
use crate::transform::{CoordinateMap, Extended, Punctured, Shortened};
use crate::CodeRate;
use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension, FieldIdentity, TrivialExt};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::modulus_select::{select_modulus, SelectExtension};
use gf2_core::field::{ConstField, FieldPoly, FieldVec, FiniteField};
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{QuotientElement, QuotientField};
use gf2_core::{BitMatrix, BitVec};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::path::{Path, PathBuf};

/// The address, length, and capacity of every buffer `registers` holds.
///
/// The encoding paths size these buffers once and overwrite them in place
/// afterwards, so two equal snapshots witness that the encodes between them
/// reached no allocator.
pub fn encode_register_shape<W>(registers: &EncodeRegisters<W>) -> Vec<(usize, usize, usize)> {
    crate::bch::encode::register_shape(registers)
}

/// [`encode_register_shape`] for the buffers a caller's workspace owns.
pub fn encode_workspace_shape<W>(workspace: &BchEncodeWorkspace<W>) -> Vec<(usize, usize, usize)> {
    encode_register_shape(workspace.registers())
}

/// [`encode_register_shape`] for the scratch registers the calling thread
/// holds for the register word type `W`, or `None` before this thread has
/// encoded in that word type.
///
/// These are the only buffers an entry point that owns no workspace can
/// allocate.
pub fn encode_scratch_shape<W: 'static>() -> Option<Vec<(usize, usize, usize)>> {
    crate::bch::encode::encode_scratch_shape::<W>()
}

/// Writes `code`'s generator matrix by encoding the $k$ message basis
/// vectors, one row per encode.
///
/// This is the straightforward reading of the matrix contract. The canonical
/// materialization derives the same matrix from the generator polynomial's
/// recurrence instead, and this is the oracle its equality tests and the
/// `bch_genmatrix` bench measure it against.
///
/// # Errors
///
/// Returns [`CodeError::ShapeMismatch`] when `out` is not $k \times n$, and
/// propagates the encoder's own errors.
///
/// # Complexity
///
/// $O(k^2 r)$ base-field operations for $r = n - k$, where the canonical
/// materialization walks the output once.
pub fn bch_generator_matrix_by_encoding<X, S, M>(
    code: &BchCode<X, S, M>,
    out: &mut M,
) -> Result<(), CodeError>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    crate::bch::matrix::write_generator_by_encoding(code, out)
}

/// Writes `code`'s parity-check matrix as the transpose of the parity block
/// [`bch_generator_matrix_by_encoding`] produces, negated, beside the
/// identity.
///
/// # Errors
///
/// Returns [`CodeError::ShapeMismatch`] when `out` is not $(n-k) \times n$,
/// and propagates the encoder's own errors.
///
/// # Complexity
///
/// $O(k^2 r)$ base-field operations for $r = n - k$.
pub fn bch_parity_check_matrix_by_encoding<X, S, M>(
    code: &BchCode<X, S, M>,
    out: &mut M,
) -> Result<(), CodeError>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    crate::bch::matrix::write_parity_check_by_encoding(code, out)
}

/// Holds every kernel-dispatched encoding family on the portable kernel
/// bundle, or releases them back to runtime detection, and reports the
/// previous setting.
///
/// One switch covers the whole bundle, so forcing it exercises the fallback
/// arm of both [`EncodeFamily::BitsliceInterleaved`][bitslice] and
/// [`EncodeFamily::ClmulFold`][fold] whatever the host detects. Every arm
/// computes the same words.
///
/// [bitslice]: crate::bch::encode::EncodeFamily::BitsliceInterleaved
/// [fold]: crate::bch::encode::EncodeFamily::ClmulFold
pub fn force_scalar_encode_kernels(forced: bool) -> bool {
    crate::bch::encode::force_scalar_encode_kernels(forced)
}

/// The name of the kernel bundle a batch encode would run now,
/// `"avx2-pclmul"` or `"scalar"`.
pub fn selected_encode_kernel() -> &'static str {
    crate::bch::encode::selected_encode_kernels()
}

/// Builds the generic eBCH(16,11) fixture used by library tests.
///
/// The production [`ExtendedBchComponent::ebch_16_11`] constructor owns the
/// BCH parameters; this helper exposes its generic extension for tests that
/// exercise the canonical transform traits directly.
pub fn generic_ebch_16_11() -> Extended<BinaryBchCode> {
    ExtendedBchComponent::ebch_16_11().into_code_for_test()
}

/// Parses an ETSI CSP test-point file into a sequence of `BitVec` blocks.
///
/// Each `%`- or `#`-prefixed line begins a new block. Within a block,
/// `'0'` and `'1'` characters become bits; all other characters are
/// ignored. Empty blocks (no bits between two delimiters) are dropped.
///
/// # Panics
///
/// Panics if the file cannot be read.
pub fn parse_tp_blocks(path: &Path) -> Vec<BitVec> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    let mut blocks: Vec<BitVec> = Vec::new();
    let mut current: Option<BitVec> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('%') || line.starts_with('#') {
            if let Some(bv) = current.take() {
                if !bv.is_empty() {
                    blocks.push(bv);
                }
            }
            current = Some(BitVec::new());
            continue;
        }
        let bv = current.get_or_insert_with(BitVec::new);
        for ch in line.chars() {
            match ch {
                '0' => bv.push_bit(false),
                '1' => bv.push_bit(true),
                _ => {}
            }
        }
    }
    if let Some(bv) = current {
        if !bv.is_empty() {
            blocks.push(bv);
        }
    }
    blocks
}

/// Builds the canonical path to a VV001-CR35 test-point file under
/// `<config_dir>/TestPoint<NN>/VV001-CR35_TP<NN>_CSP.txt`.
///
/// `tp` may include an alphabetic suffix (e.g., `"07a"`); the
/// directory uses only the numeric prefix.
pub fn tp_path(config_dir: &Path, tp: &str) -> PathBuf {
    let tp_base = tp.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    config_dir
        .join(format!("TestPoint{}", tp_base))
        .join(format!("VV001-CR35_TP{}_CSP.txt", tp))
}

/// Builds the canonical path to a test-point file for an arbitrary
/// `VV<num>-<name>_CSP` directory.
///
/// Infers the file stem from the directory basename by stripping the
/// trailing `_CSP` suffix (if present), then constructs:
/// `<config_dir>/TestPoint<NN>/<file-stem>_TP<NN>_CSP.txt`
///
/// For example, given `config_dir = ".../VV014-64QAM34_CSP"` and
/// `tp = "07a"`, the resulting path is:
/// `.../VV014-64QAM34_CSP/TestPoint07/VV014-64QAM34_TP07a_CSP.txt`
///
/// `tp` may include an alphabetic suffix (e.g., `"07a"`); the
/// `TestPoint<NN>` directory uses only the numeric prefix.
///
/// # Panics
///
/// Panics if `config_dir` has no file name (e.g. it is `/`).
pub fn tp_path_for(config_dir: &Path, tp: &str) -> PathBuf {
    let dir_name = config_dir
        .file_name()
        .expect("config_dir must have a file name")
        .to_string_lossy();
    // Strip the trailing `_CSP` suffix to get the file stem used inside
    // the directory (e.g., `VV014-64QAM34_CSP` → `VV014-64QAM34`).
    let file_stem = dir_name
        .strip_suffix("_CSP")
        .unwrap_or(&dir_name)
        .to_owned();
    let tp_base = tp.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    config_dir
        .join(format!("TestPoint{tp_base}"))
        .join(format!("{file_stem}_TP{tp}_CSP.txt"))
}

// ---------------------------------------------------------------------------
// The predeclared BCH conformance corpus
// ---------------------------------------------------------------------------

/// The message seed the evidence protocol predeclares for the conformance
/// corpus.
pub const BCH_CORPUS_SEED: u64 = 0xAE03_BCD0;

/// Messages a corpus row carries at or below [`BCH_CORPUS_LARGE_LENGTH`].
///
/// Amendment 1 of the `evidence-protocol` section of
/// `dev/active/ae03bcd0-general-bch/plan.md` fixes this count, the one below
/// it, and the threshold between them.
pub const BCH_CORPUS_MESSAGES_SMALL: usize = 4;

/// Messages a corpus row carries above [`BCH_CORPUS_LARGE_LENGTH`], which
/// keeps the committed fixture proportionate to the evidence it carries.
pub const BCH_CORPUS_MESSAGES_LARGE: usize = 2;

/// Length above which a row carries [`BCH_CORPUS_MESSAGES_LARGE`] messages.
pub const BCH_CORPUS_LARGE_LENGTH: usize = 4096;

/// The predeclared identity of one conformance-corpus row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BchCorpusRow {
    /// Row identifier from the evidence protocol, such as `"B1"`.
    pub id: &'static str,
    /// Name of the `BchCode` constructor the row is built through.
    pub construction: &'static str,
    /// First consecutive root exponent $b$.
    pub first_root: u64,
    /// Requested designed distance $\delta$.
    pub designed_distance: u64,
}

/// Receives every constructed conformance-corpus row.
///
/// The corpus spans four base fields and two symbol representations, so its
/// rows cannot share one code type. A visitor keeps the construction in one
/// place while letting each consumer stay generic over the row's own types.
///
/// The bounds are the strongest the corpus rows satisfy rather than the
/// weakest a consumer might need: [`MatrixFill`] so a visitor reaches the
/// generator and parity-check capabilities, and thread-safe `'static` symbols
/// so it reaches the opt-in matrix cache. Every corpus row is built on a
/// representation that satisfies them, and an implementation whose own body
/// needs less may still state the weaker bounds it uses.
pub trait BchCorpusVisitor {
    /// Handles one constructed row.
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        X::Base: Send + Sync + 'static,
        S: SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync;
}

/// Constructs every predeclared corpus row and hands it to `visitor` in
/// corpus order.
///
/// Each row goes through the canonical [`BchCode::construct`] pipeline: the
/// three binary rows and the two prime-base rows select their splitting field
/// by the deterministic registry policy, the DVB-T2 mother row takes the field
/// polynomial and correction radius the in-tree ETSI parameter table pins, the
/// $\mathrm{GF}(9)$ row uses the tower presentation that policy selects, and
/// the Reed-Solomon row uses the degree-one extension of the selected
/// $\mathrm{GF}(2^8)$.
///
/// # Panics
///
/// Panics if any row fails to construct, which would mean the predeclared
/// corpus no longer describes a valid BCH code.
///
/// # Complexity
///
/// Dominated by the DVB-T2 mother row, whose canonical root search and
/// cyclotomic closure run over $2^{16} - 1$ exponents.
pub fn visit_bch_corpus<V: BchCorpusVisitor>(visitor: &mut V) {
    let binary = |degree: usize, distance: u64| {
        BinaryBchCode::<u64>::primitive_narrow_sense_auto(
            Fp::<2>::zero(),
            degree,
            bch_corpus_distance(distance),
        )
        .expect("a binary primitive narrow-sense row")
    };

    for (id, degree, distance) in [("B1", 4, 7), ("B2", 7, 21), ("B3", 8, 9)] {
        visitor.visit(
            &BchCorpusRow {
                id,
                construction: "primitive_narrow_sense_auto",
                first_root: 1,
                designed_distance: distance,
            },
            &binary(degree, distance),
        );
    }

    // The DVB-T2 normal-frame mother code, on the field polynomial and the
    // correction radius the in-tree ETSI parameter table pins.
    let dvb = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
    let etsi = BinaryPrimeExt::<u64>::new(Gf2mField::new(dvb.field_m, dvb.primitive_poly))
        .expect("the ETSI normal-frame polynomial is primitive");
    let dvb_distance = 2 * dvb.t as u64 + 1;
    visitor.visit(
        &BchCorpusRow {
            id: "B4",
            construction: "primitive_narrow_sense",
            first_root: 1,
            designed_distance: dvb_distance,
        },
        &BinaryBchCode::<u64>::primitive_narrow_sense(etsi, bch_corpus_distance(dvb_distance))
            .expect("the DVB-T2 normal-frame mother code"),
    );

    // GF(3) symbols, length 13 dividing |GF(27)*| = 26 properly.
    visitor.visit(
        &BchCorpusRow {
            id: "N1",
            construction: "consecutive_roots_auto",
            first_root: 1,
            designed_distance: 5,
        },
        &DenseBchCode::<QuotientField<Fp<3>>>::consecutive_roots_auto(
            Fp::<3>::zero(),
            3,
            bch_corpus_length(13),
            RootExponent::from(1),
            bch_corpus_distance(5),
        )
        .expect("a GF(3) consecutive-root row"),
    );

    // GF(5) symbols, length 31 dividing |GF(125)*| = 124 properly.
    visitor.visit(
        &BchCorpusRow {
            id: "N2",
            construction: "consecutive_roots_auto",
            first_root: 1,
            designed_distance: 4,
        },
        &DenseBchCode::<QuotientField<Fp<5>>>::consecutive_roots_auto(
            Fp::<5>::zero(),
            3,
            bch_corpus_length(31),
            RootExponent::from(1),
            bch_corpus_distance(4),
        )
        .expect("a GF(5) consecutive-root row"),
    );

    // GF(9) symbols in the selected tower presentation, length 10 dividing
    // |GF(81)*| = 80 properly.
    let gf9_modulus = select_modulus(&Fp::<3>::zero(), 2).expect("a GF(9) modulus");
    let gf9 = QuotientField::new(Fp::<3>::zero(), gf9_modulus).expect("GF(9)");
    visitor.visit(
        &BchCorpusRow {
            id: "N3",
            construction: "consecutive_roots_auto",
            first_root: 1,
            designed_distance: 3,
        },
        &DenseBchCode::<QuotientField<QuotientElement<Fp<3>>>>::consecutive_roots_auto(
            gf9.ext_zero(),
            2,
            bch_corpus_length(10),
            RootExponent::from(1),
            bch_corpus_distance(3),
        )
        .expect("a GF(9) consecutive-root row"),
    );

    // Reed-Solomon parameters: the splitting field is the symbol field, so the
    // witness is the degree-one extension of the selected GF(2^8).
    let gf256 = <BinaryPrimeExt as SelectExtension>::select(Fp::<2>::zero(), 8)
        .expect("the registry GF(2^8) selection");
    visitor.visit(
        &BchCorpusRow {
            id: "N4",
            construction: "primitive_narrow_sense",
            first_root: 1,
            designed_distance: 33,
        },
        &DenseBchCode::<TrivialExt<Gf2mElement>>::primitive_narrow_sense(
            TrivialExt::new(gf256.field().zero()),
            bch_corpus_distance(33),
        )
        .expect("a GF(2^8) Reed-Solomon row"),
    );
}

/// Wraps a predeclared designed distance.
///
/// # Panics
///
/// Panics unless `value` is positive.
pub fn bch_corpus_distance(value: u64) -> DesignedDistance {
    DesignedDistance::try_from(value).expect("a positive designed distance")
}

/// Wraps a predeclared code length.
///
/// # Panics
///
/// Panics unless `value` is positive.
pub fn bch_corpus_length(value: u64) -> BchLength {
    BchLength::try_from(value).expect("a positive length")
}

/// Returns the canonical index of a field element.
///
/// The index is the integer whose base-$p$ digits are the element's canonical
/// prime coordinates, coordinate zero least significant, which is the
/// numbering [`FieldIdentity::write_prime_coords`] defines.
pub fn bch_corpus_index<F: FieldIdentity>(value: &F) -> u128 {
    let characteristic = u128::from(value.field_id().characteristic());
    let mut coordinates = Vec::new();
    value.write_prime_coords(&mut coordinates);
    coordinates.iter().rev().fold(0u128, |index, &digit| {
        index * characteristic + u128::from(digit)
    })
}

/// Returns the element of `witness`'s field that `index` names.
///
/// # Panics
///
/// Panics if `index` is at or above the field order, which no corpus fixture
/// coordinate is.
pub fn bch_corpus_element<F: FieldIdentity>(witness: &F, index: u128) -> F {
    let characteristic = u128::from(witness.field_id().characteristic());
    let degree = witness.field_id().degree();
    let mut remaining = index;
    let mut coordinates = Vec::with_capacity(degree);
    for _ in 0..degree {
        coordinates.push((remaining % characteristic) as u64);
        remaining /= characteristic;
    }
    assert_eq!(remaining, 0, "a canonical index below the field order");
    witness
        .from_prime_coords(&coordinates)
        .expect("canonical coordinates of this field")
}

/// Returns how many messages a corpus row of length `n` carries.
pub fn bch_corpus_message_count(n: usize) -> usize {
    if n > BCH_CORPUS_LARGE_LENGTH {
        BCH_CORPUS_MESSAGES_LARGE
    } else {
        BCH_CORPUS_MESSAGES_SMALL
    }
}

/// Draws one corpus row's seeded messages as canonical base-field indices.
///
/// The row's own stream starts from [`BCH_CORPUS_SEED`], so a row reproduces
/// independently of the rows before it. Symbols are drawn in order from the
/// repository's standard seeded test generator, `rand`'s `StdRng` under
/// `SeedableRng::seed_from_u64`, as `gen_range(0..base_order)`.
pub fn bch_corpus_messages(k: usize, n: usize, base_order: u64) -> Vec<Vec<u64>> {
    let mut rng = StdRng::seed_from_u64(BCH_CORPUS_SEED);
    (0..bch_corpus_message_count(n))
        .map(|_| (0..k).map(|_| rng.gen_range(0..base_order)).collect())
        .collect()
}

/// Encodes a symbol sequence under the corpus hex convention.
///
/// Over $\mathrm{GF}(2)$ the symbols are bit-packed under the repository's
/// canonical little-endian bit indexing, symbol $i$ occupying bit $i \bmod 8$
/// of byte $\lfloor i/8 \rfloor$ with the final byte zero-padded. Over every
/// larger base field one symbol occupies one byte holding its canonical index,
/// which the corpus admits because no row's base field exceeds 256 elements.
///
/// # Panics
///
/// Panics if a symbol does not fit a byte.
pub fn bch_corpus_encode_symbols(symbols: &[u64], base_order: u64) -> String {
    let bytes: Vec<u8> = if base_order == 2 {
        let mut packed = vec![0u8; symbols.len().div_ceil(8)];
        for (index, &symbol) in symbols.iter().enumerate() {
            if symbol == 1 {
                packed[index >> 3] |= 1 << (index & 7);
            }
        }
        packed
    } else {
        symbols
            .iter()
            .map(|&symbol| u8::try_from(symbol).expect("a base field of at most 256 elements"))
            .collect()
    };
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from_digit(u32::from(byte >> 4), 16).expect("a hex digit"));
        text.push(char::from_digit(u32::from(byte & 0xf), 16).expect("a hex digit"));
    }
    text
}

/// Decodes `count` symbols written under the corpus hex convention.
///
/// # Panics
///
/// Panics if `text` is not lowercase hex or holds fewer than `count` symbols.
pub fn bch_corpus_decode_symbols(text: &str, count: usize, base_order: u64) -> Vec<u64> {
    let bytes: Vec<u8> = text
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).expect("hex is ASCII");
            u8::from_str_radix(digits, 16).expect("a hex byte")
        })
        .collect();
    if base_order == 2 {
        (0..count)
            .map(|index| u64::from((bytes[index >> 3] >> (index & 7)) & 1))
            .collect()
    } else {
        bytes[..count].iter().map(|&byte| u64::from(byte)).collect()
    }
}

// ---------------------------------------------------------------------------
// Shared conformance cases
// ---------------------------------------------------------------------------
//
// One function per law, applied to every implementation that carries the law.
// The trait-level cases live beside the traits they check, in
// `crate::traits::block::conformance`; the cases here are the ones that need
// BCH construction, the predeclared corpus, or a derived-code transformation.
// `tests/bch_conformance.rs` is the suite that runs both sets over the whole
// implementor roster.

/// Returns the order of `code`'s base field.
pub fn bch_base_order<X, S, M>(code: &BchCode<X, S, M>) -> u64
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let id = code.base_field_id();
    id.characteristic()
        .checked_pow(id.degree() as u32)
        .expect("a corpus base field order fits a u64")
}

/// Builds `code`'s seeded corpus messages in its own symbol representation.
///
/// The symbols are [`bch_corpus_messages`]'s canonical base-field indices
/// resolved through [`bch_corpus_element`], so a row draws the same messages
/// whichever representation carries them.
///
/// # Panics
///
/// Panics if a drawn index is not a canonical coordinate of the base field.
pub fn bch_corpus_message_sequences<X, S, M>(code: &BchCode<X, S, M>) -> Vec<S>
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let zero = code.extension().base_zero();
    bch_corpus_messages(code.k(), code.n(), bch_base_order(code))
        .iter()
        .map(|message| {
            let mut symbols = S::zeroed(code.k(), &zero);
            for (index, &value) in message.iter().enumerate() {
                symbols
                    .set(index, bch_corpus_element(&zero, u128::from(value)))
                    .expect("an index below the dimension");
            }
            symbols
        })
        .collect()
}

/// Rebuilds a corpus row through the same construction in the field-generic
/// dense representation, and asserts that every derived quantity agrees with
/// `code`.
///
/// The corpus builds each row once, in the representation that row is
/// declared in. This is how the suite reaches the other representation of a
/// row that admits both, so the packed and dense implementations of the
/// canonical interfaces run the same cases on the same code.
///
/// # Panics
///
/// Panics if the row does not reconstruct, if its construction is one the
/// corpus does not use, or if any derived quantity differs between the two
/// representations.
pub fn bch_corpus_dense_twin<X, S, M>(
    row: &BchCorpusRow,
    code: &BchCode<X, S, M>,
) -> DenseBchCode<X>
where
    X: FieldExtension,
    X::Base: 'static,
    S: SymbolSequence<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let extension = code.extension().clone();
    let designed_distance = bch_corpus_distance(row.designed_distance);
    let twin = match row.construction {
        "primitive_narrow_sense" | "primitive_narrow_sense_auto" => {
            DenseBchCode::<X>::primitive_narrow_sense(extension, designed_distance)
        }
        "consecutive_roots" | "consecutive_roots_auto" => DenseBchCode::<X>::consecutive_roots(
            extension,
            bch_corpus_length(code.n() as u64),
            RootSelection::Explicit(code.root().clone()),
            RootExponent::from(row.first_root),
            designed_distance,
        ),
        other => panic!("row {} uses the unhandled construction {other}", row.id),
    }
    .unwrap_or_else(|error| panic!("row {} rebuilds in the dense form: {error}", row.id));

    assert_eq!(twin.n(), code.n(), "{} length", row.id);
    assert_eq!(twin.k(), code.k(), "{} dimension", row.id);
    assert_eq!(twin.root(), code.root(), "{} root of unity", row.id);
    assert_eq!(twin.generator(), code.generator(), "{} generator", row.id);
    assert_eq!(
        twin.defining_set(),
        code.defining_set(),
        "{} defining set",
        row.id
    );
    assert_eq!(
        twin.distance_bound(),
        code.distance_bound(),
        "{} distance bound",
        row.id
    );
    assert_eq!(
        twin.correction_radius(),
        code.correction_radius(),
        "{} correction radius",
        row.id
    );
    twin
}

/// Asserts every relationship a completed BCH construction promises.
///
/// The dimension follows the generator degree, the generator is monic and
/// divides $x^n - 1$ over the base field, the defining set is a set of
/// distinct exponents closed under $q$-cyclotomic conjugacy whose size is that
/// degree, the recorded run witnesses the distance bound inside that set, the
/// bound reaches the requested `designed_distance`, the correction radius is
/// the radius that bound implies, and the two field identities are compatible.
///
/// # Panics
///
/// Panics when any of those relationships fails.
///
/// # Complexity
///
/// $O(n \log n)$ base-field operations for the cyclic division, plus $O(n)$
/// for the defining-set closure.
pub fn bch_construction_contract<X, S, M>(code: &BchCode<X, S, M>, designed_distance: u64)
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let length = code.n();
    let dimension = code.k();
    assert!(dimension <= length, "a block code has k <= n");

    let generator = code.generator();
    assert!(!generator.is_zero(), "a generator polynomial is nonzero");
    let degree = generator
        .degree()
        .expect("a nonzero polynomial has a degree");
    assert_eq!(
        dimension,
        length - degree,
        "the dimension follows the generator degree"
    );
    assert!(
        generator
            .leading_coeff()
            .expect("a nonzero polynomial has a leading coefficient")
            .is_one(),
        "the generator is monic"
    );

    let zero = code.extension().base_zero();
    let one = zero.one_like();
    let mut coefficients = vec![zero.clone(); length + 1];
    coefficients[length] = one.clone();
    coefficients[0] = -one;
    let cyclic = FieldPoly::new(coefficients);
    assert!(
        cyclic.div_rem(generator).1.is_zero(),
        "the generator divides x^n - 1 over the base field"
    );

    let exponents: Vec<u64> = code.defining_set().iter().map(|root| root.get()).collect();
    assert_eq!(
        exponents.len(),
        degree,
        "the defining set has one exponent per generator root"
    );
    let mut sorted = exponents.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        exponents.len(),
        "the defining set holds distinct exponents"
    );
    let order = u128::from(bch_base_order(code));
    let modulus = length as u128;
    for &exponent in &exponents {
        assert!(
            (exponent as u128) < modulus,
            "a defining exponent is a residue modulo the length"
        );
        let conjugate = (u128::from(exponent) * order % modulus) as u64;
        assert!(
            sorted.binary_search(&conjugate).is_ok(),
            "the defining set is closed under q-cyclotomic conjugacy"
        );
    }

    let bound = code.distance_bound();
    assert_eq!(
        bound.minimum_distance_lower_bound(),
        bound.consecutive_root_count() + 1,
        "the bound is the witnessing run length plus one"
    );
    match bound.first_root() {
        None => {
            assert!(
                exponents.is_empty(),
                "only the empty defining set witnesses no run"
            );
            assert_eq!(bound.consecutive_root_count(), 0);
        }
        Some(first) => {
            for step in 0..bound.consecutive_root_count() {
                let exponent = ((u128::from(first.get()) + step as u128) % modulus) as u64;
                assert!(
                    sorted.binary_search(&exponent).is_ok(),
                    "the witnessing run lies inside the defining set"
                );
            }
        }
    }
    assert!(
        bound.minimum_distance_lower_bound() as u64 >= designed_distance,
        "the witnessed bound reaches the requested designed distance"
    );
    assert_eq!(
        code.correction_radius(),
        (bound.minimum_distance_lower_bound() - 1) / 2,
        "the correction radius is the one the bound implies"
    );

    let base = code.base_field_id();
    let splitting = code.splitting_field_id();
    assert_eq!(
        base.characteristic(),
        splitting.characteristic(),
        "base and splitting field share a characteristic"
    );
    assert_eq!(
        splitting.degree(),
        base.degree() * code.extension().relative_degree(),
        "the splitting field is the relative extension of the base field"
    );
}

/// Asserts that every systematic encoding path of `code` writes the same
/// codeword and that the message is readable from the first $k$ user
/// coordinates.
///
/// The paths are the trait encoder, the layout-explicit entry point, the
/// caller-workspace entry point, the allocating and caller-buffer batch entry
/// points, and every registered [`EncodeFamily`] the representation makes
/// available under the plan. The workspace buffer shape is captured before and
/// after the encodes, which witnesses that the repeated path reached no
/// allocator.
///
/// # Panics
///
/// Panics when two paths disagree, when the systematic prefix does not carry
/// the message, or when an encode of a valid message fails.
///
/// # Complexity
///
/// One reduction per message and per available family, each $O(k r)$ base-field
/// operations.
pub fn bch_encoding_contract<X, S, M>(code: &BchCode<X, S, M>, messages: &[S])
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let layout = SystematicLayout::default();
    let mut workspace = code.encode_workspace();
    let shape_before = encode_workspace_shape(&workspace);

    let expected: Vec<S> = messages
        .iter()
        .map(|message| {
            let codeword = code.encode(message).expect("a k-symbol message encodes");
            assert_eq!(codeword.len(), code.n(), "a codeword holds n symbols");
            for index in 0..code.k() {
                assert_eq!(
                    codeword.get(index),
                    message.get(index),
                    "the default layout carries the message in coordinate {index}"
                );
            }
            codeword
        })
        .collect();

    for (message, codeword) in messages.iter().zip(&expected) {
        assert_eq!(
            &code
                .encode_systematic(message, layout)
                .expect("the layout-explicit path encodes"),
            codeword,
            "the layout-explicit path writes the trait encoder's codeword"
        );

        let mut written = S::zeroed(code.n(), &BlockCode::symbol_zero(code));
        code.encode_systematic_with(message, layout, &mut workspace, &mut written)
            .expect("the workspace path encodes");
        assert_eq!(
            &written, codeword,
            "the caller-workspace path writes the trait encoder's codeword"
        );
    }

    let batch = code
        .encode_batch(messages, layout)
        .expect("the allocating batch path encodes");
    assert_eq!(
        batch, expected,
        "the allocating batch path writes the per-message codewords"
    );

    let mut buffered = vec![S::zeroed(code.n(), &BlockCode::symbol_zero(code)); messages.len()];
    code.encode_batch_into(messages, layout, &mut workspace, &mut buffered)
        .expect("the caller-buffer batch path encodes");
    assert_eq!(
        buffered, expected,
        "the caller-buffer batch path writes the per-message codewords"
    );

    let plan_families: Vec<EncodeFamily> = EncodeFamily::REGISTERED
        .iter()
        .copied()
        .filter(|&family| code.encode_family_available(family, layout))
        .collect();
    assert!(
        plan_families.contains(&EncodeFamily::REFERENCE),
        "the reference family is available for every plan"
    );
    for family in plan_families {
        code.encode_batch_family_into(family, messages, layout, &mut workspace, &mut buffered)
            .unwrap_or_else(|error| panic!("family {family:?} encodes: {error}"));
        assert_eq!(
            buffered, expected,
            "family {family:?} writes the reference family's codewords"
        );
    }

    assert_eq!(
        encode_workspace_shape(&workspace),
        shape_before,
        "the workspace buffers keep their addresses, lengths and capacities"
    );
}

/// Asserts that `code` encodes linearly: the encoding of a sum is the sum of
/// the encodings, and scaling a message scales its codeword.
///
/// # Panics
///
/// Panics when either law fails or when a valid message does not encode.
pub fn code_linearity_contract<C>(
    code: &C,
    left: &C::Symbols,
    right: &C::Symbols,
    scalar: &C::Symbol,
) where
    C: BlockEncoder,
{
    let zero = code.symbol_zero();
    let mut sum = C::Symbols::zeroed(code.k(), &zero);
    let mut scaled = C::Symbols::zeroed(code.k(), &zero);
    for index in 0..code.k() {
        let a = left.get(index).expect("a message coordinate");
        let b = right.get(index).expect("a message coordinate");
        sum.set(index, a.clone() + b).expect("a message coordinate");
        scaled
            .set(index, a * scalar.clone())
            .expect("a message coordinate");
    }

    let encoded_left = code.encode(left).expect("a message encodes");
    let encoded_right = code.encode(right).expect("a message encodes");
    let encoded_sum = code.encode(&sum).expect("a message encodes");
    let encoded_scaled = code.encode(&scaled).expect("a message encodes");

    for index in 0..code.n() {
        let a = encoded_left.get(index).expect("a codeword coordinate");
        let b = encoded_right.get(index).expect("a codeword coordinate");
        assert_eq!(
            encoded_sum.get(index),
            Some(a.clone() + b),
            "encoding is additive at coordinate {index}"
        );
        assert_eq!(
            encoded_scaled.get(index),
            Some(a * scalar.clone()),
            "encoding is homogeneous at coordinate {index}"
        );
    }
}

/// Asserts the canonical systematic user layout of a generator and
/// parity-check pair: $G = [\,I_k \mid P\,]$ and $H = [\,-P^{\mathsf T} \mid
/// I_{n-k}\,]$.
///
/// The two shapes together are $G H^{\mathsf T} = 0$ by algebra, so this is the
/// orthogonality statement in the form whose cost is $O(k(n-k))$ rather than
/// $O(k(n-k)n)$. Codes small enough for the direct triple loop also run
/// [`crate::traits::block::conformance::generator_parity_orthogonality`].
///
/// # Panics
///
/// Panics when either matrix departs from the layout, or when `code` does not
/// report itself systematic.
pub fn systematic_pair_contract<C>(code: &C)
where
    C: GeneratorMatrixAccess + ParityCheckMatrixAccess,
{
    assert!(
        code.is_systematic()
            .expect("a canonical code reports its layout"),
        "the canonical layout puts the message coordinates first"
    );
    let generator = code.generator_matrix().expect("the generator materializes");
    let parity = code
        .parity_check_matrix()
        .expect("the parity check materializes");
    let dimension = code.k();
    let redundancy = code.parity_check_rows();
    assert_eq!((generator.rows(), generator.cols()), (dimension, code.n()));
    assert_eq!((parity.rows(), parity.cols()), (redundancy, code.n()));

    let zero = code.symbol_zero();
    let one = zero.one_like();
    for row in 0..dimension {
        for col in 0..dimension {
            let expected = if row == col {
                one.clone()
            } else {
                zero.clone()
            };
            assert_eq!(
                generator.get(row, col),
                Some(expected),
                "the generator's leading block is the identity at ({row}, {col})"
            );
        }
    }
    for row in 0..redundancy {
        for col in 0..redundancy {
            let expected = if row == col {
                one.clone()
            } else {
                zero.clone()
            };
            assert_eq!(
                parity.get(row, dimension + col),
                Some(expected),
                "the parity check's trailing block is the identity at ({row}, {col})"
            );
        }
    }
    for row in 0..dimension {
        for check in 0..redundancy {
            let parity_symbol = generator
                .get(row, dimension + check)
                .expect("a generator parity cell");
            assert_eq!(
                parity.get(check, row),
                Some(-parity_symbol),
                "the parity check carries the negated transposed parity block at ({check}, {row})"
            );
        }
    }
}

/// Asserts that the caller-buffer matrix entry points reject every wrong
/// shape with [`CodeError::ShapeMismatch`] instead of writing a partial
/// result.
///
/// # Panics
///
/// Panics when a wrong shape is accepted or reported through another error.
pub fn matrix_shape_rejection_contract<C>(code: &C)
where
    C: GeneratorMatrixAccess + ParityCheckMatrixAccess,
{
    let zero = code.symbol_zero();
    for (rows, cols) in [
        (code.k() + 1, code.n()),
        (code.k(), code.n() + 1),
        (code.k(), code.n().saturating_sub(1)),
    ] {
        let mut out = C::GeneratorMatrix::zeroed(rows, cols, &zero);
        assert!(
            matches!(
                code.generator_matrix_into(&mut out),
                Err(CodeError::ShapeMismatch { .. })
            ),
            "the generator rejects the shape {rows} x {cols}"
        );
    }
    for (rows, cols) in [
        (code.parity_check_rows() + 1, code.n()),
        (code.parity_check_rows(), code.n() + 1),
    ] {
        let mut out = C::ParityCheckMatrix::zeroed(rows, cols, &zero);
        assert!(
            matches!(
                code.parity_check_matrix_into(&mut out),
                Err(CodeError::ShapeMismatch { .. })
            ),
            "the parity check rejects the shape {rows} x {cols}"
        );
    }
}

/// Asserts that the opt-in matrix cache returns the wrapped code's matrices,
/// returns them again from the retained value, and rebuilds them after
/// [`CachedMatrices::clear`].
///
/// # Panics
///
/// Panics when a cached materialization differs from the wrapped code's.
pub fn cached_matrices_contract<C>(code: C)
where
    C: GeneratorMatrixAccess + ParityCheckMatrixAccess + Clone,
    C::GeneratorMatrix: Send + Sync,
    C::ParityCheckMatrix: Send + Sync,
{
    let expected_generator = code.generator_matrix().expect("the generator materializes");
    let expected_parity = code
        .parity_check_matrix()
        .expect("the parity check materializes");
    let cached = CachedMatrices::new(code);
    for pass in 0..2 {
        assert_eq!(
            cached.generator_matrix().expect("a cached generator"),
            expected_generator,
            "the cache returns the wrapped generator on pass {pass}"
        );
        assert_eq!(
            cached.parity_check_matrix().expect("a cached parity check"),
            expected_parity,
            "the cache returns the wrapped parity check on pass {pass}"
        );
    }
    cached.clear();
    assert_eq!(
        cached.generator_matrix().expect("a rebuilt generator"),
        expected_generator,
        "the cache rebuilds the generator after a clear"
    );
    assert_eq!(
        cached
            .parity_check_matrix()
            .expect("a rebuilt parity check"),
        expected_parity,
        "the cache rebuilds the parity check after a clear"
    );
}

/// Copies `code`'s generator into the field-generic dense representation.
///
/// The rank computations the transformation cases compare against need one
/// representation, and this is the conversion that reaches it from any
/// implementor of the canonical matrix capability.
///
/// # Panics
///
/// Panics when the generator does not materialize.
pub fn generator_as_field_matrix<C>(code: &C) -> FieldMatrix<C::Symbol>
where
    C: GeneratorMatrixAccess,
{
    let source = code.generator_matrix().expect("the generator materializes");
    let mut matrix = FieldMatrix::new(code.k(), code.n(), code.symbol_zero());
    for row in 0..code.k() {
        for col in 0..code.n() {
            matrix.set(row, col, source.get(row, col).expect("a generator cell"));
        }
    }
    matrix
}

/// Returns the rank of a dense matrix, answering zero for an empty one.
pub fn field_matrix_rank<F: FiniteField>(matrix: &FieldMatrix<F>) -> usize {
    if matrix.rows() == 0 || matrix.cols() == 0 {
        0
    } else {
        matrix.rank()
    }
}

/// Returns the columns of `length` that `removed` keeps, in ascending order.
pub fn kept_coordinates(length: usize, removed: &[usize]) -> Vec<usize> {
    (0..length)
        .filter(|column| !removed.contains(column))
        .collect()
}

/// Returns the projection of `generator` onto the columns `removed` keeps.
pub fn projected_generator<F: FiniteField>(
    generator: &FieldMatrix<F>,
    removed: &[usize],
    zero: &F,
) -> FieldMatrix<F> {
    let kept = kept_coordinates(generator.cols(), removed);
    let mut projected = FieldMatrix::new(generator.rows(), kept.len(), zero.clone());
    for row in 0..generator.rows() {
        for (column, &source) in kept.iter().enumerate() {
            projected.set(row, column, generator.get(row, source));
        }
    }
    projected
}

/// Returns a generator of the subcode that vanishes on `removed`, projected
/// onto the surviving columns.
///
/// This is the direct reading of shortening: solve the vanishing constraints
/// over the message space, then read the surviving coordinates. It is the
/// oracle the transformation cases measure the wrapper's rank-derived
/// dimension against.
pub fn constrained_generator<F: FiniteField>(
    generator: &FieldMatrix<F>,
    removed: &[usize],
    zero: &F,
) -> FieldMatrix<F> {
    let kept = kept_coordinates(generator.cols(), removed);
    let basis: Vec<FieldVec<F>> = if removed.is_empty() {
        (0..generator.rows())
            .map(|row| {
                let mut vector = FieldVec::zeros_from(generator.rows(), zero);
                vector.set(row, zero.one_like());
                vector
            })
            .collect()
    } else {
        let mut constraints = FieldMatrix::new(removed.len(), generator.rows(), zero.clone());
        for (row, &column) in removed.iter().enumerate() {
            for message in 0..generator.rows() {
                constraints.set(row, message, generator.get(message, column));
            }
        }
        constraints.nullspace()
    };

    let mut result = FieldMatrix::new(basis.len(), kept.len(), zero.clone());
    for (row, vector) in basis.iter().enumerate() {
        for (column, &source) in kept.iter().enumerate() {
            let mut value = zero.zero_like();
            for message in 0..generator.rows() {
                value += vector.get(message).clone() * generator.get(message, source);
            }
            result.set(row, column, value);
        }
    }
    result
}

/// Returns whether every row of `candidate` lies in the row space of `space`.
///
/// # Panics
///
/// Panics when the two matrices have different column counts.
pub fn row_space_contains<F: FiniteField>(
    space: &FieldMatrix<F>,
    candidate: &FieldMatrix<F>,
    zero: &F,
) -> bool {
    assert_eq!(
        space.cols(),
        candidate.cols(),
        "row spaces compare inside one coordinate space"
    );
    if space.cols() == 0 {
        return true;
    }
    let mut stacked = FieldMatrix::new(space.rows() + candidate.rows(), space.cols(), zero.clone());
    for row in 0..space.rows() {
        for col in 0..space.cols() {
            stacked.set(row, col, space.get(row, col));
        }
    }
    for row in 0..candidate.rows() {
        for col in 0..candidate.cols() {
            stacked.set(space.rows() + row, col, candidate.get(row, col));
        }
    }
    field_matrix_rank(&stacked) == field_matrix_rank(space)
}

/// Asserts that the symbols of `word` at the positions in `positions` are all
/// zero, and returns nothing.
///
/// # Panics
///
/// Panics when a listed position carries a nonzero symbol.
pub fn assert_zero_at<F, S>(word: &S, positions: &[usize])
where
    F: FieldIdentity,
    S: SymbolSequence<F>,
{
    for &position in positions {
        assert!(
            word.get(position)
                .expect("a coordinate inside the word")
                .is_zero(),
            "coordinate {position} is zero"
        );
    }
}

/// Asserts that `word` is a codeword of `code`, by its parity-check syndrome.
///
/// # Panics
///
/// Panics when a syndrome coordinate is nonzero or the parity check does not
/// materialize.
pub fn assert_is_codeword<C>(code: &C, word: &C::Symbols)
where
    C: ParityCheckMatrixAccess,
{
    let parity = code
        .parity_check_matrix()
        .expect("the parity check materializes");
    let zero = code.symbol_zero();
    for check in 0..code.parity_check_rows() {
        let mut sum = zero.zero_like();
        for column in 0..code.n() {
            let symbol = word.get(column).expect("a codeword coordinate");
            sum += parity.get(check, column).expect("a parity cell") * symbol;
        }
        assert!(sum.is_zero(), "the syndrome vanishes at check {check}");
    }
}

/// Asserts that a derived code reports an information set that is a genuine
/// information set: restricting its generator to those columns is the
/// identity.
///
/// # Panics
///
/// Panics when the pivots are not ascending, out of range, or not an identity
/// restriction.
pub fn information_set_contract<C>(code: &C, information_set: &[usize])
where
    C: GeneratorMatrixAccess,
{
    assert_eq!(
        information_set.len(),
        code.k(),
        "an information set has one coordinate per dimension"
    );
    assert!(
        information_set.windows(2).all(|pair| pair[0] < pair[1]),
        "the information set is in ascending pivot order"
    );
    let generator = code.generator_matrix().expect("the generator materializes");
    let zero = code.symbol_zero();
    let one = zero.one_like();
    for (row, &column) in information_set.iter().enumerate() {
        assert!(column < code.n(), "an information coordinate is in range");
        for other in 0..code.k() {
            let expected = if other == row {
                one.clone()
            } else {
                zero.clone()
            };
            assert_eq!(
                generator.get(other, column),
                Some(expected),
                "the generator restricted to the information set is the identity at ({other}, {column})"
            );
        }
    }
}

/// Asserts the shortening contract for `mother` on the coordinate set
/// `removed`.
///
/// The derived length drops by the removed count, the derived dimension is the
/// rank of the mother subcode that vanishes on those coordinates, the derived
/// row space is that subcode's projection, the reported information set is a
/// genuine one, the coordinate map is the ascending inclusion of the surviving
/// coordinates, and every derived codeword lifts to a mother codeword that
/// vanishes on the removed set.
///
/// # Panics
///
/// Panics when any of those statements fails.
///
/// # Complexity
///
/// Dominated by the mother generator materialization and the two rank
/// computations over it.
pub fn shortening_contract<C>(mother: &C, removed: &[usize])
where
    C: BlockEncoder + GeneratorMatrixAccess + ParityCheckMatrixAccess + Clone,
{
    let zero = mother.symbol_zero();
    let mother_generator = generator_as_field_matrix(mother);
    let oracle = constrained_generator(&mother_generator, removed, &zero);
    let expected_dimension = field_matrix_rank(&oracle);

    let shortened =
        Shortened::new(mother.clone(), removed.iter().copied()).expect("a valid coordinate set");
    assert_eq!(
        shortened.n(),
        mother.n() - removed.len(),
        "shortening removes one coordinate per removed position"
    );
    assert_eq!(
        shortened.k(),
        expected_dimension,
        "the shortened dimension is the rank of the vanishing subcode"
    );

    let kept = kept_coordinates(mother.n(), removed);
    let map = shortened.coordinate_map();
    assert_eq!(map.derived_len(), shortened.n());
    assert_eq!(map.mother_len(), mother.n());
    for (derived, &source) in kept.iter().enumerate() {
        assert_eq!(
            map.mother_position(derived).expect("a derived position"),
            source,
            "the coordinate map keeps the surviving coordinates in order"
        );
    }
    let mut sorted_removed = removed.to_vec();
    sorted_removed.sort_unstable();
    assert_eq!(
        shortened.shortened_positions(),
        &sorted_removed[..],
        "the removed positions are reported in ascending order"
    );

    information_set_contract(&shortened, shortened.information_set());
    let derived_generator = generator_as_field_matrix(&shortened);
    assert!(
        row_space_contains(&oracle, &derived_generator, &zero),
        "every shortened codeword is a projected mother codeword"
    );
    assert!(
        row_space_contains(&derived_generator, &oracle, &zero),
        "every projected mother codeword is a shortened codeword"
    );

    for row in 0..shortened.k() {
        let mut basis = C::Symbols::zeroed(shortened.k(), &zero);
        basis
            .set(row, zero.one_like())
            .expect("an index below the dimension");
        let codeword = shortened.encode(&basis).expect("a basis message encodes");
        let lifted = shortened
            .extend_codeword(&codeword)
            .expect("a derived codeword lifts");
        assert_eq!(
            lifted.len(),
            mother.n(),
            "the lift is in mother coordinates"
        );
        assert_zero_at(&lifted, &sorted_removed);
        assert_is_codeword(mother, &lifted);
    }
}

/// Asserts the puncturing contract for `mother` on the coordinate set
/// `removed`.
///
/// The derived length drops by the removed count, the derived dimension is the
/// rank of the projected mother generator, the derived row space is that
/// projection's row space, the reported information set is a genuine one, and
/// the coordinate map is the ascending inclusion of the surviving coordinates.
///
/// # Panics
///
/// Panics when any of those statements fails.
pub fn puncturing_contract<C>(mother: &C, removed: &[usize])
where
    C: BlockEncoder + GeneratorMatrixAccess + Clone,
{
    let zero = mother.symbol_zero();
    let mother_generator = generator_as_field_matrix(mother);
    let oracle = projected_generator(&mother_generator, removed, &zero);
    let expected_dimension = field_matrix_rank(&oracle);

    let punctured =
        Punctured::new(mother.clone(), removed.iter().copied()).expect("a valid coordinate set");
    assert_eq!(
        punctured.n(),
        mother.n() - removed.len(),
        "puncturing removes one coordinate per removed position"
    );
    assert_eq!(
        punctured.k(),
        expected_dimension,
        "the punctured dimension is the rank of the projected generator"
    );

    let kept = kept_coordinates(mother.n(), removed);
    let map = punctured.coordinate_map();
    assert_eq!(map.derived_len(), punctured.n());
    assert_eq!(map.mother_len(), mother.n());
    for (derived, &source) in kept.iter().enumerate() {
        assert_eq!(
            map.mother_position(derived).expect("a derived position"),
            source,
            "the coordinate map keeps the surviving coordinates in order"
        );
    }

    information_set_contract(&punctured, punctured.information_set());
    let derived_generator = generator_as_field_matrix(&punctured);
    assert!(
        row_space_contains(&oracle, &derived_generator, &zero),
        "every punctured codeword is a projected mother codeword"
    );
    assert!(
        row_space_contains(&derived_generator, &oracle, &zero),
        "every projected mother codeword is a punctured codeword"
    );
}

/// Asserts the one-symbol extension contract for `mother`.
///
/// The derived code keeps the mother dimension, gains the final coordinate,
/// makes every codeword's symbol sum vanish, keeps the mother codeword in the
/// inherited coordinates, and represents the appended coordinate as a fresh
/// one with no mother preimage.
///
/// # Panics
///
/// Panics when any of those statements fails.
pub fn extension_contract<C>(mother: &C, messages: &[C::Symbols])
where
    C: BlockEncoder + Clone,
{
    let extended = Extended::new(mother.clone()).expect("a mother code extends");
    assert_eq!(extended.k(), mother.k(), "extension keeps the dimension");
    assert_eq!(
        extended.n(),
        mother.n() + 1,
        "extension adds one coordinate"
    );
    assert_eq!(
        extended.extension_position(),
        extended.n() - 1,
        "the appended coordinate is the final one"
    );

    let map = extended.coordinate_map();
    assert_eq!(map.derived_len(), extended.n());
    for position in 0..mother.n() {
        assert_eq!(
            map.mother_position_opt(position)
                .expect("a derived position"),
            Some(position),
            "an inherited coordinate keeps its mother position"
        );
    }
    assert_eq!(
        map.mother_position_opt(extended.extension_position())
            .expect("the appended position"),
        None,
        "the appended coordinate has no mother preimage"
    );

    for message in messages {
        let mother_codeword = mother.encode(message).expect("a message encodes");
        let codeword = extended.encode(message).expect("a message encodes");
        let mut sum = mother.symbol_zero().zero_like();
        for position in 0..mother.n() {
            let symbol = codeword.get(position).expect("a codeword coordinate");
            assert_eq!(
                Some(symbol.clone()),
                mother_codeword.get(position),
                "the inherited coordinates carry the mother codeword"
            );
            sum += &symbol;
        }
        sum += &codeword
            .get(extended.extension_position())
            .expect("the appended coordinate");
        assert!(
            sum.is_zero(),
            "the appended coordinate makes the symbol sum vanish"
        );
    }
}

/// Asserts that a chain of two shortenings composes its coordinate map back to
/// the original mother, and that composition with the identity is the identity.
///
/// # Panics
///
/// Panics when a composed position differs from the two-hop lookup.
pub fn coordinate_map_composition_contract<C>(mother: &C, first: &[usize], second: &[usize])
where
    C: BlockEncoder + GeneratorMatrixAccess + Clone,
{
    let inner =
        Shortened::new(mother.clone(), first.iter().copied()).expect("a valid coordinate set");
    let inner_map = inner.coordinate_map().clone();
    let inner_length = inner.n();
    let outer = inner
        .shorten(second.iter().copied())
        .expect("a valid coordinate set");

    let kept = kept_coordinates(inner_length, second);
    let map = outer.coordinate_map();
    assert_eq!(map.mother_len(), mother.n(), "the map reaches the mother");
    assert_eq!(map.derived_len(), outer.n());
    for (derived, &intermediate) in kept.iter().enumerate() {
        assert_eq!(
            map.mother_position(derived).expect("a derived position"),
            inner_map
                .mother_position(intermediate)
                .expect("an intermediate position"),
            "the composed map is the two-hop lookup at {derived}"
        );
    }

    let identity = CoordinateMap::identity(map.derived_len());
    assert_eq!(
        &map.compose(&identity)
            .expect("composition with the identity"),
        map,
        "composing with the identity leaves the map unchanged"
    );
}

/// Asserts that a packed binary sequence uses canonical little-endian bit
/// indexing and keeps every bit above its length zero.
///
/// # Panics
///
/// Panics when an accessor disagrees with the backing word or a padding bit is
/// set.
pub fn packed_sequence_layout(value: &BitVec) {
    for index in 0..value.len() {
        let word = value.words()[index >> 6];
        assert_eq!(
            SymbolSequence::<Fp<2>>::get(value, index),
            Some(Fp::<2>::new((word >> (index & 63)) & 1)),
            "canonical bit index {index} is bit {} of word {}",
            index & 63,
            index >> 6
        );
    }
    assert_padding_is_zero(value.words(), value.len(), "sequence");
}

/// Asserts that a packed binary matrix uses canonical little-endian bit
/// indexing in every row and keeps every bit above its column count zero.
///
/// # Panics
///
/// Panics when an accessor disagrees with the backing word or a padding bit is
/// set.
pub fn packed_matrix_layout(value: &BitMatrix) {
    for row in 0..value.rows() {
        let words = value.row_words(row);
        for col in 0..value.cols() {
            assert_eq!(
                SymbolMatrix::<Fp<2>>::get(value, row, col),
                Some(Fp::<2>::new((words[col >> 6] >> (col & 63)) & 1)),
                "canonical bit index {col} of row {row}"
            );
        }
        assert_padding_is_zero(words, value.cols(), "matrix row");
    }
}

/// Asserts that no bit at or above `length` is set in `words`.
///
/// # Panics
///
/// Panics when a padding bit is set.
fn assert_padding_is_zero(words: &[u64], length: usize, what: &str) {
    for (index, &word) in words.iter().enumerate() {
        let base = index * 64;
        if base >= length {
            assert_eq!(word, 0, "the {what} tail word {index} is zero-padded");
            continue;
        }
        let used = length - base;
        if used < 64 {
            assert_eq!(
                word >> used,
                0,
                "the {what} final word is zero above bit {used}"
            );
        }
    }
}
