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
//!   polynomial type with schoolbook arithmetic. Higher-level operations
//!   (division, Karatsuba, NTT, batch evaluation) land in follow-up
//!   tasks of the `bdf95060` story.

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
