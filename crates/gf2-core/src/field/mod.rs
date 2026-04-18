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
//!
//! # Batch operations
//!
//! - [`batch_ops`] — Montgomery's trick for inverting many elements with a
//!   single field inversion.

pub mod batch_ops;
mod traits;
pub mod vec;

#[cfg(test)]
pub(crate) mod axiom_tests;

pub use batch_ops::{
    batch_inverse, batch_inverse_in_place, batch_inverse_skip_zeros,
    batch_inverse_skip_zeros_in_place, batch_inverse_with_scratch,
};
pub use traits::{ConstField, FiniteField, FiniteFieldExt};
pub use vec::{FieldVec, StridedIter};
