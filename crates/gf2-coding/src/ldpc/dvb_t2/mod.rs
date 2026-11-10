//! DVB-T2 LDPC codes built from the `@/citation/Etsi2015` tables, and the
//! DVB-T2 BICM (Bit-Interleaved Coded Modulation) components of §6: BCH and
//! LDPC coding in [`mod@concat`], bit interleaving in [`bit_interleaver`], and
//! the bit-to-cell-word demultiplexer and cell mapping in [`cell_mapper`].
//! `crates/gf2-coding/examples/dvb_t2_bicm_chain.rs` composes them.
//! Throughout, a positive LLR means bit 0 is more likely.

pub mod bit_interleaver;
pub(crate) mod builder;
pub mod cell_mapper;
pub mod concat;
pub(crate) mod dvb_t2_matrices;
pub(crate) mod params;

pub use bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation};
pub use cell_mapper::{DvbT2CellDemux, DvbT2CellMapper};
pub use params::{DvbParams, FrameSize};
