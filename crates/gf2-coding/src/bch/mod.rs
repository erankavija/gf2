//! BCH (Bose-Chaudhuri-Hocquenghem) codes.
//!
//! BCH codes are a family of cyclic error-correcting codes that can correct
//! multiple random errors using algebraic decoding over a splitting field of
//! the code-symbol field. [`spec`] constructs them over any supported base
//! field; [`BinaryBchDecoder`] decodes binary codes over GF(2^m).
//!
//! # Organization
//!
//! - [`spec`]: the construction model, its validating pipeline, and the
//!   canonical code type over any supported base field
//! - [`encode`]: systematic encoding over any supported base field, with the
//!   explicit user-layout contract, the layout-declaring view, and the
//!   profile-driven dispatch among equivalent batch-encoding algorithm
//!   families
//! - [`matrix`]: generator and parity-check materialization in the default
//!   user layout and under a declared one, with caller buffers and explicit
//!   opt-in caching
//! - [`error`]: the BCH construction and decoding error surface
//! - [`BinaryBchDecoder`]: binary Berlekamp-Massey and Chien decoding with
//!   verified [`BchDecodeOutcome`]s
//! - [`dvb_t2`]: DVB-T2 standard BCH outer codes
//!
//! Extended BCH codes are [`Extended`](crate::transform::Extended) over a
//! canonical code.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::encode::SystematicLayout;
//! use gf2_coding::bch::{BchDecodeOutcome, BinaryBchCode, BinaryBchDecoder, DesignedDistance};
//! use gf2_coding::traits::block::{BlockCode, BlockEncoder};
//! use gf2_core::field::extension::BinaryPrimeExt;
//! use gf2_core::gf2m::Gf2mField;
//! use gf2_core::BitVec;
//!
//! // BCH(15, 7) with designed distance 5, so t = 2.
//! let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011))?;
//! let code = BinaryBchCode::primitive_narrow_sense(extension, DesignedDistance::try_from(5)?)?;
//! assert_eq!((code.n(), code.k()), (15, 7));
//!
//! let message = BitVec::ones(code.k());
//! let mut received = code.encode(&message)?;
//! received.set(2, !received.get(2));
//! received.set(11, !received.get(11));
//!
//! let decoder = BinaryBchDecoder::new(&code);
//! let mut workspace = decoder.workspace();
//! let outcome = decoder.correct_in_place(&mut received, &mut workspace)?;
//! assert_eq!(outcome, BchDecodeOutcome::Corrected { count: 2 });
//! assert_eq!(
//!     code.systematic_message(&received, SystematicLayout::default())?,
//!     message
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod core;
pub mod dvb_t2;
pub mod encode;
pub mod error;
pub mod matrix;
pub mod spec;
pub use encode::{LayoutView, SystematicLayout};

pub use core::{BchDecodeOutcome, BchDecodeReport, BchDecodeWorkspace, BinaryBchDecoder, CodeRate};
pub use matrix::{CachedMatrices, MatrixFill};
pub use spec::{
    BchCode, BchDistanceBound, BchLength, BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance,
    RootExponent, RootSelection,
};
