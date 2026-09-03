//! Test helpers shared between crate-internal unit tests, the integration
//! tests under `tests/`, and the bench targets.
//!
//! The helpers are allocation witnesses over the encoding workspaces, the
//! kernel-selection controls of the kernel-dispatched encode families, the
//! basis-vector matrix oracles the canonical materialization is measured and
//! compared against, the predeclared BCH conformance corpus with its seeded
//! messages, a mother-code wrapper that forces the rank-derived shortening
//! derivation, and a reader for the ETSI DVB-T2 verified vectors (the
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
use crate::bch::encode::{BchEncodeWorkspace, EncodeRegisters, SystematicKernel};
use crate::bch::spec::{
    BchCode, BchLength, BinaryBchCode, DenseBchCode, DesignedDistance, RootExponent,
};
use crate::error::CodeError;
use crate::product::ExtendedBchComponent;
use crate::traits::block::{
    BlockCode, BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess, SymbolMatrix,
};
use crate::transform::Extended;
use crate::CodeRate;
use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension, FieldIdentity, TrivialExt};
use gf2_core::field::modulus_select::{select_modulus, SelectExtension};
use gf2_core::field::ConstField;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{QuotientElement, QuotientField};
use gf2_core::BitVec;
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

/// A mother-code wrapper whose only difference is that it reports no
/// systematic layout.
///
/// [`Shortened`](crate::transform::Shortened) reads the mother's
/// [`is_systematic`](GeneratorMatrixAccess::is_systematic) report to select
/// [`SystematicRestriction`](crate::transform::ShortenedDerivation::SystematicRestriction),
/// so wrapping a systematic mother in this forces the same coordinate set
/// onto [`RankDerived`](crate::transform::ShortenedDerivation::RankDerived).
/// One fixture then produces both derivations of one code, which is how the
/// suites compare them.
///
/// Every other method delegates, so the two mothers have the same generator,
/// check matrix, dimensions, and codewords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RankDerivedMother<C>(pub C);

impl<C: BlockCode> BlockCode for RankDerivedMother<C> {
    type Symbol = C::Symbol;
    type Symbols = C::Symbols;

    fn symbol_zero(&self) -> Self::Symbol {
        self.0.symbol_zero()
    }

    fn k(&self) -> usize {
        self.0.k()
    }

    fn n(&self) -> usize {
        self.0.n()
    }
}

impl<C: BlockEncoder> BlockEncoder for RankDerivedMother<C> {
    fn encode_into(
        &self,
        message: &Self::Symbols,
        codeword: &mut Self::Symbols,
    ) -> Result<(), CodeError> {
        self.0.encode_into(message, codeword)
    }
}

impl<C: GeneratorMatrixAccess> GeneratorMatrixAccess for RankDerivedMother<C> {
    type GeneratorMatrix = C::GeneratorMatrix;

    fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
        self.0.generator_matrix_into(out)
    }

    /// Reports `false` whatever the wrapped code reports, which is what
    /// forces the rank-derived construction.
    fn is_systematic(&self) -> Result<bool, CodeError> {
        Ok(false)
    }
}

impl<C: ParityCheckMatrixAccess> ParityCheckMatrixAccess for RankDerivedMother<C> {
    type ParityCheckMatrix = C::ParityCheckMatrix;

    fn parity_check_rows(&self) -> usize {
        self.0.parity_check_rows()
    }

    fn parity_check_matrix_into(&self, out: &mut Self::ParityCheckMatrix) -> Result<(), CodeError> {
        self.0.parity_check_matrix_into(out)
    }
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
pub trait BchCorpusVisitor {
    /// Handles one constructed row.
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>;
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
