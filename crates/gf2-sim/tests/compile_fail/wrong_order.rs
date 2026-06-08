//! Calling `.decoder()` before `.modcod()` must NOT compile: the typestate
//! marker enforces the BICM stage order. `Pipeline::dvb_t2()` returns a
//! `Builder<NeedsModcod>`, which has no `decoder` method — that method exists
//! only on `Builder<NeedsDecoder>`, reached after `.modcod(...)`.

use gf2_sim::Pipeline;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};

fn main() {
    let _ = Pipeline::dvb_t2()
        .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true));
}
