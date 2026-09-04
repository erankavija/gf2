//! Emits the predeclared BCH conformance corpus that the external oracles read.
//!
//! The evidence protocol in `dev/active/ae03bcd0-general-bch/plan.md` fixes
//! eight rows and one message seed. `gf2_coding::test_support::visit_bch_corpus`
//! owns the construction; this example reduces each constructed row to the
//! *inputs* an external oracle needs to build the identical code: the base- and
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
//! `FieldIdentity::write_prime_coords` defines, so the index is a property of
//! the field presentation rather than of any carrier type.
//!
//! # Usage
//!
//! ```text
//! ./scripts/cargo-budget.sh cargo run --release --features test-support \
//!     --example bch_oracle_messages -- <output.json>
//! ```

use std::fmt::Write as _;

use gf2_coding::bch::encode::SystematicKernel;
use gf2_coding::bch::spec::BchCode;
use gf2_coding::test_support::{
    bch_corpus_encode_symbols, bch_corpus_index, bch_corpus_messages, visit_bch_corpus,
    BchCorpusRow, BchCorpusVisitor, BCH_CORPUS_SEED,
};
use gf2_coding::traits::block::SymbolMatrix;
use gf2_core::field::extension::FieldExtension;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: bch_oracle_messages <output.json>");
    let mut writer = CorpusWriter::default();
    visit_bch_corpus(&mut writer);
    std::fs::write(&path, writer.finish()).expect("the output path is writable");
    eprintln!("wrote {} rows to {path}", writer.rows);
}

/// Accumulates the corpus rows as JSON while the visitor walks them.
#[derive(Default)]
struct CorpusWriter {
    body: String,
    rows: usize,
}

impl CorpusWriter {
    /// Returns the complete, newline-terminated corpus document.
    fn finish(&self) -> String {
        format!(
            "{{\n  \"seed\": \"0x{BCH_CORPUS_SEED:08X}\",\n  \"rows\": [\n{}\n  ]\n}}\n",
            self.body
        )
    }
}

impl BchCorpusVisitor for CorpusWriter {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        if self.rows > 0 {
            self.body.push_str(",\n");
        }
        self.rows += 1;

        let base_id = code.base_field_id();
        let ext_id = code.splitting_field_id();
        let characteristic = base_id.characteristic();
        let relative_degree = code.extension().relative_degree();
        let base_order = u64::try_from(base_id.order().expect("a representable base order"))
            .expect("a base order below 2^64");

        let base_modulus = base_id
            .modulus()
            .map(|modulus| modulus.coefficients().to_vec())
            .unwrap_or_default();
        let ext_modulus: Vec<u128> = if relative_degree == 1 {
            Vec::new()
        } else {
            let modulus = ext_id.modulus().expect("a proper extension has a modulus");
            (0..=relative_degree)
                .map(|degree| digits_to_index(modulus.coefficient(degree), characteristic))
                .collect()
        };

        let mut out = String::new();
        out.push_str("    {\n");
        field(&mut out, "id", &format!("\"{}\"", row.id));
        field(
            &mut out,
            "construction",
            &format!("\"{}\"", row.construction),
        );
        field(&mut out, "characteristic", &characteristic.to_string());
        field(&mut out, "base_degree", &base_id.degree().to_string());
        field(&mut out, "ext_degree", &ext_id.degree().to_string());
        field(&mut out, "relative_degree", &relative_degree.to_string());
        field(&mut out, "base_order", &base_order.to_string());
        field(
            &mut out,
            "ext_order",
            &ext_id
                .order()
                .expect("a representable extension order")
                .to_string(),
        );
        field(&mut out, "base_modulus", &numbers(&base_modulus));
        field(&mut out, "ext_modulus", &numbers(&ext_modulus));
        field(&mut out, "n", &code.n().to_string());
        field(&mut out, "first_root", &row.first_root.to_string());
        field(
            &mut out,
            "designed_distance",
            &row.designed_distance.to_string(),
        );
        field(&mut out, "k", &code.k().to_string());
        field(
            &mut out,
            "alpha",
            &bch_corpus_index(code.root()).to_string(),
        );
        let defining_set: Vec<u64> = code
            .defining_set()
            .iter()
            .map(|exponent| exponent.get())
            .collect();
        field(&mut out, "defining_set", &numbers(&defining_set));

        out.push_str("      \"messages\": [\n");
        let messages = bch_corpus_messages(code.k(), code.n(), base_order);
        for (position, message) in messages.iter().enumerate() {
            let comma = if position + 1 == messages.len() {
                ""
            } else {
                ","
            };
            let text = bch_corpus_encode_symbols(message, base_order);
            writeln!(out, "        \"{text}\"{comma}").expect("String write");
        }
        out.push_str("      ]\n    }");
        self.body.push_str(&out);
    }
}

/// Appends one comma-terminated JSON member.
fn field(out: &mut String, name: &str, value: &str) {
    writeln!(out, "      \"{name}\": {value},").expect("String write");
}

/// Interprets base-$p$ digits, least significant first, as a canonical index.
fn digits_to_index(digits: &[u64], characteristic: u64) -> u128 {
    digits.iter().rev().fold(0u128, |index, &digit| {
        index * u128::from(characteristic) + u128::from(digit)
    })
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
