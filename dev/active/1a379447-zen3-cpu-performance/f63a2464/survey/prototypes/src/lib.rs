//! Bounded prototypes of the quantized, layered and QC-aware LDPC candidates.
//!
//! One module per family of the decision record. Nothing here is a production
//! decoder or changes one: the crate is outside the workspace, consumes
//! `gf2-coding`'s code, edge layout and LLR types through their safe public
//! interfaces, and exists to supply the contract, quality and timing evidence
//! the checkpoint decides on.
//!
//! Each family carries a scalar reference path, and the one vectorized kernel
//! is asserted byte-identical to it.

pub mod layered;
pub mod qc;
pub mod quantized;

pub use layered::{LayeredDecoder, LayeredOutcome};
pub use qc::{QcDecoder, QcLayout, QcOutcome};
pub use quantized::{
    quantized_check_row, Alphabet, QuantizedDecoder, QuantizedOutcome, QuantizedRule,
};
