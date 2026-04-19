//! Generic finite field trait hierarchy.
//!
//! This module provides abstract traits for finite field arithmetic, enabling
//! generic algorithms over any field type (binary extensions, prime fields, tower extensions).
//!
//! # Traits
//!
//! - [`FiniteField`] — Core trait: arithmetic, identities, wide accumulation.
//! - [`ConstField`] — Extension for `Copy` fields with zero-cost constructors.
//! - [`FiniteFieldExt`] — Blanket convenience methods: `square`, `pow`, `frobenius`.
//! - [`TwoAdicField`] — Fields with a large power-of-two subgroup of `F^*`,
//!   enabling radix-2 NTT butterflies.
//!
//! # Batch operations
//!
//! - [`batch_ops`] — Montgomery's trick for inverting many elements with a
//!   single field inversion.
//!
//! # Polynomials
//!
//! - [`poly`] — [`FieldPoly<F>`](poly::FieldPoly), a generic univariate
//!   polynomial type. It is the single source of truth in `gf2-core`:
//!   [`Gf2mPoly_<V>`](crate::gf2m::Gf2mPoly_) is now a thin `pub type`
//!   alias for `FieldPoly<Gf2mElement_<V>>`. The module covers the
//!   full basic algebraic surface — addition, subtraction, negation,
//!   scalar multiplication, polynomial multiplication (schoolbook +
//!   Karatsuba dispatch), Euclidean division and GCD, Horner
//!   evaluation, per-point batch evaluation, construction from roots,
//!   and products of polynomial slices. Advanced upgrades (subproduct
//!   tree batch evaluation, Lagrange interpolation, balanced product
//!   tree + batch GCD, NTT) land in sibling tasks that build on this
//!   surface.

pub mod batch_ops;
pub mod poly;
mod traits;
pub mod two_adic;
pub mod vec;

#[cfg(any(test, feature = "test-support"))]
pub mod axiom_tests;

pub use batch_ops::{
    batch_inverse, batch_inverse_in_place, batch_inverse_skip_zeros,
    batch_inverse_skip_zeros_in_place, batch_inverse_with_scratch,
};
pub use poly::FieldPoly;
pub use traits::{ConstField, FiniteField, FiniteFieldExt};
pub use two_adic::TwoAdicField;
pub use vec::{FieldVec, StridedIter};
