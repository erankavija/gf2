//! DVB-T2 LDPC codes built from the `@/citation/Etsi2015` tables, and the
//! DVB-T2 BICM (Bit-Interleaved Coded Modulation) components of §6: BCH and
//! LDPC coding in [`mod@concat`] and bit interleaving in [`bit_interleaver`].
//! `crates/gf2-coding/examples/dvb_t2_bicm_chain.rs` composes them with a QAM
//! mapper and demapper. Throughout, a positive LLR means bit 0 is more likely.

pub mod bit_interleaver;
pub(crate) mod builder;
pub mod concat;
pub(crate) mod dvb_t2_matrices;
pub(crate) mod params;

pub use bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation};
pub use params::{DvbParams, FrameSize};
