//! Generic finite field trait hierarchy.

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
