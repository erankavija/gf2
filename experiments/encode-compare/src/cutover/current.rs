//! The current revision's arm of the cutover comparison: the canonical
//! construction model's batch encoder and generator materialization, in the
//! descending layout the pre-cutover encoder writes.
//!
//! `cutover.sh` also compiles this file as the `api` module of an example in
//! an archive of the current revision, so it names its harness as
//! `super::harness`.

use super::harness::{digest, Arm, Cell, Config, Frame, Prepared, Row, Workload};
use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchCode, FrameSize};
use gf2_coding::bch::{BinaryBchCode, DesignedDistance, LayoutView, SystematicLayout};
use gf2_coding::traits::block::{BlockCode, BlockEncoder, GeneratorMatrixAccess};
use gf2_coding::CodeRate;
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};

const LAYOUT: SystematicLayout = SystematicLayout::MessageParityDescending;

/// The gf2 revision this crate builds against, labelled by `revision`.
pub struct Gf2 {
    pub revision: String,
}

enum Code {
    Primitive(LayoutView<BinaryPrimeExt, BitVec, BitMatrix>),
    Dvb(DvbT2BchCode),
}

fn build(row: &Row) -> Code {
    let code = match row.dvb {
        Some(frame) => {
            let frame = match frame {
                Frame::Short => FrameSize::Short,
                Frame::Normal => FrameSize::Normal,
            };
            Code::Dvb(dvb_t2_bch_code(frame, CodeRate::Rate1_2).expect("a DVB-T2 outer code"))
        }
        None => {
            let field = Gf2mField::new(row.m, row.prim).with_tables();
            let extension = BinaryPrimeExt::new(field).expect("a primitive polynomial");
            let distance =
                DesignedDistance::try_from(2 * row.t as u64 + 1).expect("a positive distance");
            let code = BinaryBchCode::primitive_narrow_sense(extension, distance)
                .expect("a narrow-sense code");
            Code::Primitive(LayoutView::new(code, LAYOUT))
        }
    };
    let (n, k) = match &code {
        Code::Primitive(view) => (view.code().n(), view.code().k()),
        Code::Dvb(code) => (code.n(), code.k()),
    };
    assert_eq!(
        (n, k),
        (row.n, row.k),
        "{} has the contract's shape",
        row.name
    );
    code
}

struct Encode {
    code: Code,
    messages: Vec<BitVec>,
    codewords: Vec<BitVec>,
}

impl Config for Encode {
    fn label(&self) -> String {
        match self.code {
            Code::Primitive(_) => "BchCode::encode_batch".to_owned(),
            Code::Dvb(_) => "DvbT2BchCode::encode per message".to_owned(),
        }
    }

    fn call(&mut self) {
        self.codewords = match &self.code {
            Code::Primitive(view) => view
                .code()
                .encode_batch(std::hint::black_box(&self.messages), LAYOUT)
                .expect("a validated batch"),
            Code::Dvb(code) => std::hint::black_box(&self.messages)
                .iter()
                .map(|message| code.encode(message).expect("a message of length k"))
                .collect(),
        };
    }

    fn output_digest(&mut self) -> u64 {
        let bits = self.codewords.first().map_or(0, BitVec::len);
        digest(
            self.codewords.len(),
            bits,
            self.codewords.iter().map(BitVec::words),
        )
    }
}

struct Materialize {
    code: Code,
    matrix: Option<BitMatrix>,
}

impl Config for Materialize {
    fn label(&self) -> String {
        match self.code {
            Code::Primitive(_) => "LayoutView::generator_matrix".to_owned(),
            Code::Dvb(_) => "DvbT2BchCode::generator_matrix".to_owned(),
        }
    }

    fn call(&mut self) {
        self.matrix = None;
        self.matrix = Some(
            match &self.code {
                Code::Primitive(view) => view.generator_matrix(),
                Code::Dvb(code) => code.generator_matrix(),
            }
            .expect("a materializable generator"),
        );
    }

    fn output_digest(&mut self) -> u64 {
        let matrix = self.matrix.as_ref().expect("a call before the digest");
        digest(
            matrix.rows(),
            matrix.cols(),
            (0..matrix.rows()).map(|row| matrix.row_words(row)),
        )
    }
}

impl Arm for Gf2 {
    fn role(&self) -> &'static str {
        "current"
    }

    fn name(&self) -> String {
        format!("gf2 {}", self.revision)
    }

    fn configs<'a>(&'a self, cell: &Cell, messages: &'a [Vec<u64>]) -> Vec<Prepared<'a>> {
        let code = build(cell.row);
        let config: Box<dyn Config> = match cell.workload {
            Workload::W1 => Box::new(Encode {
                code,
                messages: messages
                    .iter()
                    .map(|words| BitVec::from_words(words.clone(), cell.row.k))
                    .collect(),
                codewords: Vec::new(),
            }),
            Workload::W2 => Box::new(Materialize { code, matrix: None }),
        };
        vec![Ok(config)]
    }
}

/// Coefficients of the generator polynomial of `row`'s code, index $i$
/// holding the coefficient of $x^i$: the codeword of the message whose only
/// one is its lowest-degree coefficient, read in the descending layout.
///
/// # Panics
///
/// Panics if the row does not build the contract's code.
pub fn generator_coefficients(row: &Row) -> Vec<u8> {
    let mut message = BitVec::zeros(row.k);
    message.set(row.k - 1, true);
    let codeword = match build(row) {
        Code::Primitive(view) => view.encode(&message),
        Code::Dvb(code) => code.encode(&message),
    }
    .expect("a message of length k");
    let r = row.n - row.k;
    (0..=r)
        .map(|degree| u8::from(codeword.get(row.n - 1 - degree)))
        .collect()
}
