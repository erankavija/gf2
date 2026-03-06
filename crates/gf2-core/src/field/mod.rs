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

mod traits;

#[cfg(test)]
pub(crate) mod axiom_tests;

pub use traits::{ConstField, FiniteField, FiniteFieldExt};
