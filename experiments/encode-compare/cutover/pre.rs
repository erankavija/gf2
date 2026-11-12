//! The pre-cutover revision's arm of the cutover comparison: the legacy
//! `BchCode`/`BchEncoder` surface, whose systematic layout is the descending
//! `[message | parity]` order the harness digests.
//!
//! `cutover.sh` compiles this file as the `api` module of an example inside an
//! archive of the pre-cutover revision; it does not build against the current
//! one, whose legacy surface is removed.

use super::harness::{digest, Arm, Cell, Config, Frame, Prepared, Row, Workload};
use gf2_coding::bch::dvb_t2::FrameSize;
use gf2_coding::bch::{BchCode, BchEncoder};
use gf2_coding::traits::GeneratorMatrixAccess;
use gf2_coding::CodeRate;
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};

/// The gf2 revision this example builds against, labelled by `revision`.
pub struct Gf2 {
    pub revision: String,
}

fn build(row: &Row) -> BchCode {
    let code = match row.dvb {
        Some(Frame::Short) => BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2),
        Some(Frame::Normal) => BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2),
        None => BchCode::new(
            row.n,
            row.k,
            row.t,
            Gf2mField::new(row.m, row.prim).with_tables(),
        ),
    };
    assert_eq!(
        (code.n(), code.k()),
        (row.n, row.k),
        "{} has the contract's shape",
        row.name
    );
    code
}

struct Encode {
    encoder: BchEncoder,
    messages: Vec<BitVec>,
    codewords: Vec<BitVec>,
}

impl Config for Encode {
    fn label(&self) -> String {
        "BchEncoder::encode_batch".to_owned()
    }

    fn call(&mut self) {
        self.codewords = self
            .encoder
            .encode_batch(std::hint::black_box(&self.messages));
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
    code: BchCode,
    matrix: Option<BitMatrix>,
}

impl Config for Materialize {
    fn label(&self) -> String {
        "BchCode::generator_matrix".to_owned()
    }

    fn call(&mut self) {
        self.matrix = None;
        self.matrix = Some(self.code.generator_matrix());
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
        "pre"
    }

    fn name(&self) -> String {
        format!("gf2 {}", self.revision)
    }

    fn configs<'a>(&'a self, cell: &Cell, messages: &'a [Vec<u64>]) -> Vec<Prepared<'a>> {
        let code = build(cell.row);
        let config: Box<dyn Config> = match cell.workload {
            Workload::W1 => Box::new(Encode {
                encoder: BchEncoder::new(code),
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
