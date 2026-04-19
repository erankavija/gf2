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
//! # Wide accumulator types
//!
//! [`QuadraticExtWide<W>`] and [`CubicExtWide<W>`] are the wide accumulator
//! types associated with the two extension constructors. They store
//! component-wise, unreduced wide values (using the base field's `Wide`) so
//! that dot-product-style loops can accumulate many products with only a
//! single final [`crate::field::FiniteField::reduce_wide`] call.
//!
//! The `Wide` type propagates through nested towers naturally. For example:
//!
//! ```text
//! GF(p^2)  = QuadraticExt<Fp<P>>             Wide = QuadraticExtWide<u128>
//! GF(p^4)  = QuadraticExt<QuadraticExt<Fp<P>>>  Wide = QuadraticExtWide<QuadraticExtWide<u128>>
//! GF(p^6)  = CubicExt<QuadraticExt<Fp<P>>>      Wide = CubicExtWide<QuadraticExtWide<u128>>
//! ```
//!
//! At every tower level [`crate::field::FiniteField::max_unreduced_additions`]
//! collapses down to the base prime field's bound: accumulation is limited by
//! the smallest component accumulator in the tower (usually `u128`).
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

pub use batch::{BatchExtField, SimdKaratsubaHook};
pub use cubic::{CubicExt, CubicExtWide};
pub use ext_config::ExtConfig;
pub use quadratic::{QuadraticExt, QuadraticExtWide};
