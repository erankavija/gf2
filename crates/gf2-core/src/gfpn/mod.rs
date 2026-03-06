//! GF(p^n) — Tower Extension Field Arithmetic
//!
//! This module provides algebraic extensions of prime fields using the tower
//! construction. Elements of GF(p^n) are built by stacking quadratic and cubic
//! extensions, each defined by an irreducible polynomial specified via
//! [`ExtConfig`].
//!
//! # Architecture
//!
//! - [`ExtConfig`]: Trait specifying the non-residue β for each extension level.
//! - [`QuadraticExt<C>`]: Elements c₀ + c₁·u where u² = β.
//! - [`CubicExt<C>`]: Elements c₀ + c₁·v + c₂·v² where v³ = β.
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

mod cubic;
mod ext_config;
mod quadratic;

pub use cubic::CubicExt;
pub use ext_config::ExtConfig;
pub use quadratic::QuadraticExt;
