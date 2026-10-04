//! BCH (Bose-Chaudhuri-Hocquenghem) codes: [`spec`] constructs them over any
//! supported base field, [`encode`] and [`matrix`] encode them systematically
//! and materialize $G$ and $H$, and [`BinaryBchDecoder`] decodes the binary
//! ones. Extended, shortened, and punctured BCH codes are
//! [`Extended`](crate::transform::Extended),
//! [`Shortened`](crate::transform::Shortened), and
//! [`Punctured`](crate::transform::Punctured) over a canonical code.
//!
//! # Coordinate conventions
//!
//! Internal coordinate $i$ is the coefficient of $x^i$
//! ([`spec`](spec#coordinate-convention)), a [`SystematicLayout`] maps user
//! coordinates to internal ones, and [`matrix`] states the layout of $G$ and
//! $H$. [`BinaryBchDecoder`] reads internal coordinates. A cyclic rotation of
//! a codeword is a codeword, so a default-layout word decodes directly and the
//! reported error positions index the supplied word. A reversed codeword
//! belongs to the reciprocal code, which in general differs, so a
//! descending-layout word is mapped to internal coordinates first, through
//! [`SystematicPlan::internal_coordinate`](encode::SystematicPlan::internal_coordinate);
//! [`dvb_t2::DvbT2BchDecoder`] performs that mapping for the DVB-T2 layout.
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
//! // BCH(15, 7): designed distance 5 and a witnessed run of four roots, so t = 2.
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
