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
//! - [`FieldIdentity`] — Carriers that name their algebraic identity
//!   ([`FieldId`]) and their canonical prime-field coordinates.
//! - [`FieldExtension`] — The relation "E is an extension of B", witnessed by
//!   a value: embedding, checked restriction, relative degree, relative
//!   Frobenius, and the order relationships between the two fields.
//!
//! # Extensions and identity
//!
//! - [`extension`] — [`FieldId`], [`FieldIdentity`], [`FieldExtension`], the
//!   validation certificates that make repeated construction cheap, and the
//!   concrete witnesses [`BinaryPrimeExt`], [`ConstExt`], and [`TrivialExt`]
//!   covering GF(2) inside a runtime GF(2^m), the compile-time binomial
//!   towers, and the trivial extension of any carrier.
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
pub mod charpoly;
pub mod expr;
pub mod extension;
pub mod extension_wiedemann;
pub mod inverse;
pub mod irreducibility;
pub mod matrix;
pub mod modulus_select;
pub mod ntt;
pub mod ple;
pub mod poly;
pub mod poly_interpolate;
pub mod sparse_matrix;
mod traits;
pub mod triangular;
pub mod two_adic;
pub mod vec;
pub mod winograd;

#[cfg(any(test, feature = "test-support"))]
pub mod axiom_tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_random_matrix;

pub use batch_ops::{
    batch_inverse, batch_inverse_in_place, batch_inverse_skip_zeros,
    batch_inverse_skip_zeros_in_place, batch_inverse_with_scratch,
};
pub use extension::{
    convert_element, convert_into_const, Basis, BinaryPrimeExt, CertificateBasis, ConstExt,
    ConstSimpleExtension, ElementRepr, ExtensionCertificate, FactorWitness, FieldError,
    FieldExtension, FieldId, FieldIdentity, ModulusId, OrderCertificate, TrivialExt,
    ELEMENT_REPR_VERSION, FIELD_ID_ENCODING_VERSION,
};
pub use irreducibility::{prove_irreducible, IrreducibilityCertificate, IrreducibilityMethod};
pub use modulus_select::{
    select_modulus, select_modulus_id, select_modulus_with_registry, BaseValidationError,
    EmptyRegistry, ModulusRegistry, ModulusSelectionError, RegistryEntry, RegistryProvenance,
};
pub use ntt::ntt_inplace;
pub use ple::Permutation;
pub use poly::FieldPoly;
pub use poly_interpolate::{
    formal_derivative, interpolate, interpolate_auto, interpolate_fast, InterpolationError,
    INTERPOLATE_THRESHOLD,
};
pub use traits::{ConstField, FiniteField, FiniteFieldExt, PlePanelLane};
pub use two_adic::TwoAdicField;
pub use vec::{FieldVec, StridedIter};
