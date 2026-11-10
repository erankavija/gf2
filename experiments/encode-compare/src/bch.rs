//! The DVB-T2 BCH cells and the gf2 arm.

use crate::Arm;
use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchCode, FrameSize};
use gf2_coding::traits::block::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use std::hint::black_box;
use std::sync::Arc;

/// One BCH cell: the rate-1/2 outer code of a frame size, encoding a pool of
/// `batch` distinct messages in turn.
#[derive(Clone, Copy, Debug)]
pub struct BchCell {
    pub frame: FrameSize,
    pub batch: usize,
}

/// The measured cells: both frame sizes at batch 1 and 256.
pub const BCH_CELLS: [BchCell; 4] = [
    BchCell {
        frame: FrameSize::Short,
        batch: 1,
    },
    BchCell {
        frame: FrameSize::Short,
        batch: 256,
    },
    BchCell {
        frame: FrameSize::Normal,
        batch: 1,
    },
    BchCell {
        frame: FrameSize::Normal,
        batch: 256,
    },
];

impl BchCell {
    /// The gf2 code of the cell.
    pub fn code(&self) -> DvbT2BchCode {
        dvb_t2_bch_code(self.frame, CodeRate::Rate1_2).expect("the DVB-T2 rate-1/2 outer code")
    }

    /// Seed of the cell's messages.
    pub fn seed(&self) -> u64 {
        match self.frame {
            FrameSize::Short => 0x7200,
            FrameSize::Normal => 0x32400,
        }
    }
}

/// gf2's canonical `BlockEncoder::encode_into` on a shared code.
pub struct Gf2BchArm {
    code: Arc<DvbT2BchCode>,
    messages: Arc<Vec<BitVec>>,
    codeword: BitVec,
}

impl Gf2BchArm {
    pub fn new(code: Arc<DvbT2BchCode>, messages: Arc<Vec<BitVec>>) -> Self {
        let codeword = BitVec::zeros(gf2_coding::traits::block::BlockCode::n(code.as_ref()));
        Self {
            code,
            messages,
            codeword,
        }
    }
}

impl Arm for Gf2BchArm {
    fn encode(&mut self, index: usize) {
        self.code
            .encode_into(&self.messages[index], &mut self.codeword)
            .expect("a message of the code's dimension");
        black_box(&self.codeword);
    }
}
