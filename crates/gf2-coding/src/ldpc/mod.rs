//! LDPC codes and decoders, with constructions for DVB-T2 and 5G NR.

mod batch;
mod core;
pub mod dvb_t2;
mod edge_layout;
pub mod encoding;
pub mod flooding_profile;
mod layered;
mod min_sum;
pub mod nr_5g;
mod qc;
mod route;

pub use batch::{DecodedWord, LdpcBatchDecoder};
pub use core::*;
pub use edge_layout::EdgeLayout;
pub use layered::{LayerError, LayeredConfig, LayeredDecoder, LayeredRule};
pub use min_sum::{min_sum_check_row, MinSumRule};
pub use qc::QcDecoder;
pub use route::{
    decide_batch_route, BatchRoute, LaneBackend, LaneCapability, QcCapability, QcRouteBounds,
    LANE_CLAIM_QC_VECTORS,
};
