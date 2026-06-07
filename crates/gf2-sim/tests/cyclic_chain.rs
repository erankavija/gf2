//! Cycle detection in the graph builder (`c09d3e95`).
//!
//! Builds a cyclic chain and asserts `Chain::build` returns
//! [`BuildError::Cyclic`].

use std::sync::Arc;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::CodeRate;

use gf2_sim::error::BuildError;
use gf2_sim::graph::Chain;
use gf2_sim::stage::erase;
use gf2_sim::stages::{BitInterleave, DvbT2Encode};

#[test]
fn test_cyclic_chain_is_rejected() {
    // Two BitPackedBatch → BitPackedBatch stages: their output and input types
    // match, so `connect` accepts an edge in either direction. Connecting them
    // both ways forms a 2-cycle that `build` must reject.
    let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
    let interleaver = Arc::new(DvbT2BitInterleaver::new(modcod));

    let mut chain = Chain::new();
    let enc = chain.add(erase(DvbT2Encode::new(codec)));
    let il = chain.add(erase(BitInterleave::new(interleaver)));

    chain
        .connect(enc, il)
        .expect("BitPacked → BitPacked is valid");
    chain
        .connect(il, enc)
        .expect("BitPacked → BitPacked is valid");

    match chain.build() {
        Err(BuildError::Cyclic { involved }) => {
            assert_eq!(
                involved,
                vec![enc, il],
                "both stages lie on the cycle and are reported"
            );
        }
        Err(other) => panic!("expected BuildError::Cyclic, got {other:?}"),
        Ok(_) => panic!("expected BuildError::Cyclic, got a built pipeline"),
    }
}
