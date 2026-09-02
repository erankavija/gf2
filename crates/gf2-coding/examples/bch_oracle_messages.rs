//! Emits the predeclared BCH conformance corpus that the external oracles read.
//!
//! The evidence protocol in `dev/active/ae03bcd0-general-bch/plan.md` fixes
//! eight rows and one message seed. This example constructs every row through
//! the canonical [`BchCode::construct`] pipeline, then writes the *inputs* an
//! external oracle needs to build the identical code: the base- and
//! splitting-field presentations gf2 selected, the primitive $n$-th root of
//! unity $\alpha$ gf2 derived, the code parameters, and the seeded messages.
//!
//! It deliberately writes no generator polynomial and no codeword. Those are
//! the quantities under comparison; the oracles derive their own and the
//! integration suite `bch_oracle_agreement` recomputes gf2's.
//!
//! # Canonical index
//!
//! Every field element is written as its *canonical index*: the integer whose
//! base-$p$ digits are the element's canonical prime coordinates, coordinate
//! zero least significant. That is exactly the numbering
//! [`FieldIdentity::write_prime_coords`] defines, so the index is a property of
//! the field presentation rather than of any carrier type.
//!
//! # Symbol-sequence encoding
//!
//! A sequence of base-field symbols is written as a lowercase hex string. Over
//! $\mathrm{GF}(2)$ the symbols are bit-packed under the repository's canonical
//! little-endian bit indexing: symbol $i$ occupies bit $i \bmod 8$ of byte
//! $\lfloor i/8 \rfloor$, and the final byte is zero-padded. Over every larger
//! base field one symbol occupies one byte holding its canonical index, which
//! the corpus admits because no row's base field exceeds 256 elements.
//!
//! # Usage
//!
//! ```text
//! cargo run --release --example bch_oracle_messages -- <output.json>
//! ```

use std::fmt::Write as _;

use gf2_coding::bch::spec::{
    BchCode, BchLength, BinaryBchCode, DenseBchCode, DesignedDistance, RootExponent,
};
use gf2_coding::traits::block::{SymbolMatrix, SymbolSequence};
use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension, FieldIdentity, TrivialExt};
use gf2_core::field::modulus_select::{select_modulus, SelectExtension};
use gf2_core::field::ConstField;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{QuotientElement, QuotientField};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// The message seed the evidence protocol predeclares.
const SEED: u64 = 0xAE03_BCD0;

/// $x^{16} + x^5 + x^3 + x^2 + 1$, the normal-frame field polynomial ETSI
/// EN 302 755 pins and the polynomial `DvbBchParams::for_code` carries.
const ETSI_NORMAL_MODULUS: u64 = 0b1_0000_0000_0010_1101;

/// Messages per row below the size threshold, and above it.
const MESSAGES_SMALL: usize = 4;
const MESSAGES_LARGE: usize = 2;
/// Rows longer than this carry [`MESSAGES_LARGE`] messages instead of
/// [`MESSAGES_SMALL`], which keeps the committed fixture proportionate to the
/// evidence it carries.
const LARGE_ROW_LENGTH: usize = 4096;

/// One corpus row reduced to presentation-independent integers.
struct Row {
    id: &'static str,
    construction: &'static str,
    characteristic: u64,
    base_degree: usize,
    ext_degree: usize,
    relative_degree: usize,
    base_order: u64,
    ext_order: u128,
    /// Defining polynomial of $B$ over $\mathrm{GF}(p)$, prime residues from
    /// the constant term upward. Empty when $B$ is the prime field itself.
    base_modulus: Vec<u64>,
    /// Defining polynomial of $E$ over $B$, canonical base-field indices from
    /// the constant term upward. Empty when $E = B$.
    ext_modulus: Vec<u128>,
    n: usize,
    first_root: u64,
    designed_distance: u64,
    k: usize,
    /// Canonical index of $\alpha$ in $E$.
    alpha: u128,
    defining_set: Vec<u64>,
    messages: Vec<String>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: bch_oracle_messages <output.json>");
    let rows = corpus();
    std::fs::write(&path, render(&rows)).expect("the output path is writable");
    eprintln!("wrote {} rows to {path}", rows.len());
}

/// Builds every predeclared corpus row through the canonical pipeline.
fn corpus() -> Vec<Row> {
    let binary = |degree: usize, distance: u64| {
        BinaryBchCode::<u64>::primitive_narrow_sense_auto(
            Fp::<2>::zero(),
            degree,
            designed(distance),
        )
        .expect("a binary primitive narrow-sense row")
    };

    let mut rows = vec![
        describe("B1", "primitive_narrow_sense_auto", &binary(4, 7), 1, 7),
        describe("B2", "primitive_narrow_sense_auto", &binary(7, 21), 1, 21),
        describe("B3", "primitive_narrow_sense_auto", &binary(8, 9), 1, 9),
    ];

    // B4: the DVB-T2 normal-frame mother code, built on the ETSI field
    // polynomial made explicit rather than on automatic selection.
    let etsi = BinaryPrimeExt::<u64>::new(Gf2mField::new(16, ETSI_NORMAL_MODULUS))
        .expect("the ETSI normal-frame polynomial is primitive");
    let b4 = BinaryBchCode::<u64>::primitive_narrow_sense(etsi, designed(25))
        .expect("the DVB-T2 normal-frame mother code");
    rows.push(describe("B4", "primitive_narrow_sense", &b4, 1, 25));

    // N1: GF(3) symbols, length 13 dividing |GF(27)*| = 26 properly.
    let n1 = DenseBchCode::<QuotientField<Fp<3>>>::consecutive_roots_auto(
        Fp::<3>::zero(),
        3,
        length(13),
        RootExponent::from(1),
        designed(5),
    )
    .expect("a GF(3) consecutive-root row");
    rows.push(describe("N1", "consecutive_roots_auto", &n1, 1, 5));

    // N2: GF(5) symbols, length 31 dividing |GF(125)*| = 124 properly.
    let n2 = DenseBchCode::<QuotientField<Fp<5>>>::consecutive_roots_auto(
        Fp::<5>::zero(),
        3,
        length(31),
        RootExponent::from(1),
        designed(4),
    )
    .expect("a GF(5) consecutive-root row");
    rows.push(describe("N2", "consecutive_roots_auto", &n2, 1, 4));

    // N3: GF(9) symbols in the tower presentation gf2 selects, length 10
    // dividing |GF(81)*| = 80 properly.
    let gf9_modulus = select_modulus(&Fp::<3>::zero(), 2).expect("a GF(9) modulus");
    let gf9 = QuotientField::new(Fp::<3>::zero(), gf9_modulus).expect("GF(9)");
    let n3 = DenseBchCode::<QuotientField<QuotientElement<Fp<3>>>>::consecutive_roots_auto(
        gf9.ext_zero(),
        2,
        length(10),
        RootExponent::from(1),
        designed(3),
    )
    .expect("a GF(9) consecutive-root row");
    rows.push(describe("N3", "consecutive_roots_auto", &n3, 1, 3));

    // N4: Reed-Solomon parameters. The splitting field is the symbol field,
    // so the witness is the degree-one extension of the selected GF(2^8).
    let gf256 = <BinaryPrimeExt as SelectExtension>::select(Fp::<2>::zero(), 8)
        .expect("the registry GF(2^8) selection");
    let n4 = DenseBchCode::<TrivialExt<Gf2mElement>>::primitive_narrow_sense(
        TrivialExt::new(gf256.field().zero()),
        designed(33),
    )
    .expect("a GF(2^8) Reed-Solomon row");
    rows.push(describe("N4", "primitive_narrow_sense", &n4, 1, 33));

    rows
}

fn designed(value: u64) -> DesignedDistance {
    DesignedDistance::try_from(value).expect("a positive designed distance")
}

fn length(value: u64) -> BchLength {
    BchLength::try_from(value).expect("a positive length")
}

/// Reduces a constructed code to the presentation-independent row record.
fn describe<X, S, M>(
    id: &'static str,
    construction: &'static str,
    code: &BchCode<X, S, M>,
    first_root: u64,
    designed_distance: u64,
) -> Row
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let base_id = code.base_field_id();
    let ext_id = code.splitting_field_id();
    let characteristic = base_id.characteristic();
    let base_degree = base_id.degree();
    let relative_degree = code.extension().relative_degree();
    let base_order = u64::try_from(base_id.order().expect("a representable base order"))
        .expect("a base order below 2^64");

    let base_modulus = base_id
        .modulus()
        .map(|modulus| modulus.coefficients().to_vec())
        .unwrap_or_default();
    let ext_modulus = if relative_degree == 1 {
        Vec::new()
    } else {
        let modulus = ext_id.modulus().expect("a proper extension has a modulus");
        (0..=relative_degree)
            .map(|degree| digits_to_index(modulus.coefficient(degree), characteristic))
            .collect()
    };

    let mut coordinates = Vec::new();
    code.root().write_prime_coords(&mut coordinates);
    let alpha = digits_to_index(&coordinates, characteristic);

    Row {
        id,
        construction,
        characteristic,
        base_degree,
        ext_degree: ext_id.degree(),
        relative_degree,
        base_order,
        ext_order: ext_id.order().expect("a representable extension order"),
        base_modulus,
        ext_modulus,
        n: code.n(),
        first_root,
        designed_distance,
        k: code.k(),
        alpha,
        defining_set: code
            .defining_set()
            .iter()
            .map(|exponent| exponent.get())
            .collect(),
        messages: messages(code.k(), code.n(), base_order),
    }
}

/// Draws this row's seeded messages and encodes them.
///
/// The row's own stream starts from [`SEED`], so a row reproduces
/// independently of the rows before it.
fn messages(k: usize, n: usize, base_order: u64) -> Vec<String> {
    let count = if n > LARGE_ROW_LENGTH {
        MESSAGES_LARGE
    } else {
        MESSAGES_SMALL
    };
    let mut rng = StdRng::seed_from_u64(SEED);
    (0..count)
        .map(|_| {
            let symbols: Vec<u64> = (0..k).map(|_| rng.gen_range(0..base_order)).collect();
            encode_symbols(&symbols, base_order)
        })
        .collect()
}

/// Interprets base-$p$ digits, least significant first, as a canonical index.
fn digits_to_index(digits: &[u64], characteristic: u64) -> u128 {
    digits.iter().rev().fold(0u128, |index, &digit| {
        index * u128::from(characteristic) + u128::from(digit)
    })
}

/// Encodes a symbol sequence under the module-level hex convention.
fn encode_symbols(symbols: &[u64], base_order: u64) -> String {
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
        write!(text, "{byte:02x}").expect("writing to a String never fails");
    }
    text
}

/// Renders the corpus as stable, newline-terminated JSON.
fn render(rows: &[Row]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    writeln!(out, "  \"seed\": \"0x{SEED:08X}\",").expect("String write");
    out.push_str("  \"rows\": [\n");
    for (index, row) in rows.iter().enumerate() {
        out.push_str("    {\n");
        writeln!(out, "      \"id\": \"{}\",", row.id).expect("String write");
        writeln!(out, "      \"construction\": \"{}\",", row.construction).expect("String write");
        writeln!(out, "      \"characteristic\": {},", row.characteristic).expect("String write");
        writeln!(out, "      \"base_degree\": {},", row.base_degree).expect("String write");
        writeln!(out, "      \"ext_degree\": {},", row.ext_degree).expect("String write");
        writeln!(out, "      \"relative_degree\": {},", row.relative_degree).expect("String write");
        writeln!(out, "      \"base_order\": {},", row.base_order).expect("String write");
        writeln!(out, "      \"ext_order\": {},", row.ext_order).expect("String write");
        writeln!(
            out,
            "      \"base_modulus\": {},",
            numbers(&row.base_modulus)
        )
        .expect("String write");
        writeln!(out, "      \"ext_modulus\": {},", numbers(&row.ext_modulus))
            .expect("String write");
        writeln!(out, "      \"n\": {},", row.n).expect("String write");
        writeln!(out, "      \"first_root\": {},", row.first_root).expect("String write");
        writeln!(
            out,
            "      \"designed_distance\": {},",
            row.designed_distance
        )
        .expect("String write");
        writeln!(out, "      \"k\": {},", row.k).expect("String write");
        writeln!(out, "      \"alpha\": {},", row.alpha).expect("String write");
        writeln!(
            out,
            "      \"defining_set\": {},",
            numbers(&row.defining_set)
        )
        .expect("String write");
        out.push_str("      \"messages\": [\n");
        for (position, message) in row.messages.iter().enumerate() {
            let comma = if position + 1 == row.messages.len() {
                ""
            } else {
                ","
            };
            writeln!(out, "        \"{message}\"{comma}").expect("String write");
        }
        out.push_str("      ]\n");
        out.push_str(if index + 1 == rows.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

/// Renders integers as a flat JSON array.
fn numbers<T: std::fmt::Display>(values: &[T]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write!(out, "{value}").expect("String write");
    }
    out.push(']');
    out
}
