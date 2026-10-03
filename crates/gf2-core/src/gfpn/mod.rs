//! GF(p^n) — Extension Field Arithmetic
//!
//! Algebraic extensions of prime fields in two forms: compile-time tower
//! constructions ([`QuadraticExt`], [`CubicExt`]), each defined by an
//! irreducible polynomial specified via [`ExtConfig`], and polynomial
//! quotient fields of arbitrary degree in [`quotient`], whose modulus is
//! certificate-validated at runtime or fixed by the type.
//!
//! The tower types accumulate unreduced products in [`QuadraticExtWide`] and
//! [`CubicExtWide`], which nest with the tower. At every tower level
//! [`crate::field::FiniteField::max_unreduced_additions`] is the base prime
//! field's bound.
//!
//! # Examples
//!
//! ```
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::ExtConfig;
//!
//! // Define GF(7²) with β = 3 (a quadratic non-residue mod 7).
//! struct Fq2Config;
//!
//! impl ExtConfig for Fq2Config {
//!     type BaseField = Fp<7>;
//!     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
//! }
//!
//! // The non-residue is accessible:
//! assert_eq!(Fq2Config::NON_RESIDUE.value(), 3);
//!
//! // mul_by_non_residue uses the default (generic multiply):
//! let x = Fp::<7>::new(4);
//! assert_eq!(Fq2Config::mul_by_non_residue(x).value(), 5); // 4*3 mod 7 = 5
//! ```

pub mod batch;
mod cubic;
mod ext_config;
mod quadratic;
pub mod quotient;

pub use batch::{BatchExtField, SimdKaratsubaHook};
pub use cubic::{CubicExt, CubicExtWide};
pub use ext_config::ExtConfig;
pub use quadratic::{QuadraticExt, QuadraticExtWide};
pub use quotient::{
    ConstQuotient, ConstQuotientConfig, ConstQuotientExt, QuotientElement, QuotientExt,
    QuotientField,
};
