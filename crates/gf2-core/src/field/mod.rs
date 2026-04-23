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
//! # Fast transforms
//!
//! - [`ntt`] — radix-2 Number Theoretic Transform over [`TwoAdicField`].
//!   Powers the `O(n log n)` fast polynomial multiplication path
//!   [`FieldPoly::mul_ntt`](poly::FieldPoly::mul_ntt) and the free
//!   function [`poly::mul_fast`].
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
//!   evaluation, naive per-point batch evaluation, subproduct-tree
//!   batch evaluation, construction from roots, and products of
//!   polynomial slices. Further algorithmic upgrades (Lagrange
//!   interpolation, balanced product tree + batch GCD, NTT) land in
//!   sibling tasks that build on this surface.

pub mod batch_ops;
pub mod matrix;
pub mod ntt;
pub mod poly;
pub mod poly_interpolate;
mod traits;
pub mod two_adic;
pub mod vec;

#[cfg(any(test, feature = "test-support"))]
pub mod axiom_tests;

pub use batch_ops::{
    batch_inverse, batch_inverse_in_place, batch_inverse_skip_zeros,
    batch_inverse_skip_zeros_in_place, batch_inverse_with_scratch,
};
pub use ntt::ntt_inplace;
pub use poly::FieldPoly;
pub use poly_interpolate::{
    formal_derivative, interpolate, interpolate_auto, interpolate_fast, InterpolationError,
    INTERPOLATE_THRESHOLD,
};
pub use traits::{ConstField, FiniteField, FiniteFieldExt};
pub use two_adic::TwoAdicField;
pub use vec::{FieldVec, StridedIter};
