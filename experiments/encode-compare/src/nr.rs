//! The NR rate-matched LDPC cells and the gf2 arm.

use crate::Arm;
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use std::hint::black_box;
use std::sync::Arc;

/// One NR cell: base graph, transmitted length and message length, at
/// redundancy version 0 and one bit per symbol.
#[derive(Clone, Copy, Debug)]
pub struct NrCell {
    pub base_graph: u8,
    pub n: usize,
    pub k: usize,
}

/// The measured cells, from the smallest BG2 to the largest BG1 frame of the
/// earlier comparison.
pub const NR_CELLS: [NrCell; 4] = [
    NrCell {
        base_graph: 2,
        n: 256,
        k: 121,
    },
    NrCell {
        base_graph: 2,
        n: 1440,
        k: 720,
    },
    NrCell {
        base_graph: 1,
        n: 2560,
        k: 2048,
    },
    NrCell {
        base_graph: 1,
        n: 8448,
        k: 4224,
    },
];

impl NrCell {
    /// The gf2 code of the cell.
    pub fn code(&self) -> Nr5gRateMatchedCode {
        QuasiCyclicLdpc::nr_5g_rate_matched(self.base_graph, self.n, self.k)
    }

    /// Seed of the cell's messages.
    pub fn seed(&self) -> u64 {
        (u64::from(self.base_graph) << 48) ^ ((self.n as u64) << 24) ^ self.k as u64
    }
}

/// gf2's canonical `BlockEncoder::encode` on a shared code.
pub struct Gf2NrArm {
    code: Arc<Nr5gRateMatchedCode>,
    messages: Arc<Vec<BitVec>>,
}

impl Gf2NrArm {
    pub fn new(code: Arc<Nr5gRateMatchedCode>, messages: Arc<Vec<BitVec>>) -> Self {
        Self { code, messages }
    }
}

impl Arm for Gf2NrArm {
    fn encode(&mut self, index: usize) {
        black_box(self.code.encode(&self.messages[index]));
    }
}
