//! Connector type-mismatch detection in the graph builder (`c09d3e95`).
//!
//! Connects a stage producing `SymbolBatch` into a stage consuming
//! `BitPackedBatch` and asserts `Chain::connect` returns
//! [`BuildError::TypeMismatch`].

use std::sync::Arc;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::CodeRate;

use gf2_sim::error::BuildError;
use gf2_sim::graph::Chain;
use gf2_sim::stage::erase;
use gf2_sim::stages::{DvbT2Encode, GrayQamMap};

#[test]
fn test_incompatible_connect_is_type_mismatch() {
    // GrayQamMap: BitPackedBatch → SymbolBatch.
    // DvbT2Encode: BitPackedBatch → BitPackedBatch (consumes BitPackedBatch).
    // Connecting map → encode wires a SymbolBatch producer into a
    // BitPackedBatch consumer, which `connect` must reject as a TypeMismatch.
    let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());

    let mut chain = Chain::new();
    let map = chain.add(erase(GrayQamMap::new(DvbT2Modulation::Qam16)));
    let enc = chain.add(erase(DvbT2Encode::new(codec)));

    match chain.connect(map, enc) {
        Err(BuildError::TypeMismatch {
            from_stage,
            to_stage,
            from_type,
            to_type,
        }) => {
            assert_eq!(from_stage, map);
            assert_eq!(to_stage, enc);
            assert_ne!(
                from_type, to_type,
                "the producer output type and consumer input type differ"
            );
        }
        other => panic!("expected BuildError::TypeMismatch, got {other:?}"),
    }
}
