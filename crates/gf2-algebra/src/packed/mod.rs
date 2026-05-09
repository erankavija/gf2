//! Packed finite-field abstractions.
//!
//! Hosts the [`PackedField`] and [`PackedFieldVec`] traits that abstract
//! lane-parallel arithmetic over a small prime field, plus the concrete
//! `Bipedal{3,5,7}` element / vector / matrix types that implement them.
//!
//! The trait surface is fixed by `dev/plans/d1b_packed_field_api.md`
//! and frozen at the W6 `gate:api-freeze`. The W1-T1 skeleton declares
//! the module tree only; the trait definitions and concrete impls land
//! in the W1 implementation issues (T2-T6) and W3 / W4.
//!
//! [`PackedField`]: <https://example.invalid/placeholder> "added in W1-T2"
//! [`PackedFieldVec`]: <https://example.invalid/placeholder> "added in W1-T2"

pub mod bipedal3;

#[cfg(feature = "f5")]
pub mod bipedal5;

#[cfg(feature = "f7")]
pub mod bipedal7;
