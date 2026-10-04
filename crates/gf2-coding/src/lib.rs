//! Error-correcting codes built on `gf2-core` primitives. The canonical trait
//! surface is generic over the code symbol field and representation
//! ([`FieldVec`](gf2_core::field::FieldVec)/[`FieldMatrix`](gf2_core::field::matrix::FieldMatrix)),
//! with packed binary specializations using
//! [`BitVec`](gf2_core::BitVec) and [`BitMatrix`](gf2_core::BitMatrix).
//! It provides linear block, BCH, convolutional, LDPC, GLDPC, product and CRC
//! codes and Reed-Muller subcodes; BCJR, GRAND, and ordered-statistics
//! decoders; modems, channels, and Monte Carlo simulation.

#![deny(unsafe_code)]

pub mod bch;
pub mod bcjr;
pub mod channel;
pub mod convolutional;
pub mod crc;
pub mod drm;
pub mod dvb_t2_bicm_harness;
pub mod error;
pub mod fading;
pub mod gldpc;
pub mod grand;
pub mod info_theory;
pub mod ldpc;
pub mod linear;
pub mod llr;
pub mod modem;
pub mod osd;
pub mod product;

pub mod simulation;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod traits;
pub mod transform;
pub mod tuning;

pub use bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
pub use bch::CodeRate;
pub use channel::AwgnChannel;
pub use convolutional::{ConvolutionalDecoder, ConvolutionalEncoder};
pub use error::{Capability, CodeCapability, CodeError, RepresentationId};
pub use ldpc::{
    CirculantMatrix, DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder, QuasiCyclicLdpc,
};
pub use linear::{LinearBlockCode, SyndromeTableDecoder};
pub use llr::Llr;
pub use traits::{DecoderResult, GeneratorMatrixAccess};
