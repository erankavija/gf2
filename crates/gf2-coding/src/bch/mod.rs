//! BCH (Bose-Chaudhuri-Hocquenghem) codes.
//!
//! BCH codes are a family of cyclic error-correcting codes that can correct
//! multiple random errors using algebraic decoding over a splitting field of
//! the code-symbol field. [`spec`] constructs them over any supported base
//! field; the decoding path in this module is binary, over GF(2^m).
//!
//! # Organization
//!
//! - `core`: Core BCH types and algorithms
//! - [`spec`]: the construction model, its validating pipeline, and the
//!   canonical code type over any supported base field
//! - [`error`]: the BCH construction and decoding error surface
//! - [`extended`]: extended BCH codes
//! - [`dvb_t2`]: DVB-T2 standard BCH outer codes
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::{BchCode, BchEncoder, BchDecoder};
//! use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
//! use gf2_core::gf2m::Gf2mField;
//! use gf2_core::BitVec;
//!
//! // Create BCH(15, 11, 1) code
//! let field = Gf2mField::new(4, 0b10011);
//! let code = BchCode::new(15, 11, 1, field);
//! let encoder = BchEncoder::new(code.clone());
//! let decoder = BchDecoder::new(code);
//!
//! // Encode message
//! let msg = BitVec::ones(11);
//! let cw = encoder.encode(&msg);
//!
//! // Inject single-bit error
//! let mut received = cw.clone();
//! received.set(5, !received.get(5));
//!
//! // Decode and correct
//! let decoded = decoder.decode(&received);
//! assert_eq!(decoded, msg);
//! ```

mod core;
pub mod dvb_t2;
pub mod error;
pub mod extended;
pub mod matrix;
pub mod spec;

pub use core::{
    BchCode, BchDecodeOutcome, BchDecodeReport, BchDecodeWorkspace, BchDecoder, BchEncoder,
    BinaryBchDecoder, CodeRate,
};
pub use matrix::CachedMatrices;
// The canonical code type keeps its module path while `core::BchCode` still
// occupies this name; `spec::BchCode` is reachable as `spec::DenseBchCode` or
// `spec::BinaryBchCode` for the two standard representations.
pub use spec::{
    BchDistanceBound, BchLength, BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance,
    RootExponent, RootSelection,
};
