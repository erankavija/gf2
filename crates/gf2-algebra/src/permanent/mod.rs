//! Permanent algorithms over small prime fields.
//!
//! Hosts the `Permanent` trait, the generic `permanent_ryser<F>` driver,
//! the `permanent_mod3_reference` cross-check, and the per-prime
//! `permanent_bipedal{3,5,7}` fast paths. See the epic design at
//! `dev/plans/gf2_algebra_permanent.md` §6 / §7.3 / §9 for the algorithm
//! family, and `dev/plans/d1b_packed_field_api.md` for the trait surface
//! frozen at W6.
//!
//! # Status
//!
//! W2 in progress — Ryser driver landed (W2-T7); reference port and
//! bipedal fast paths follow in T8/T9 and W3.
//!
//! # Re-exports
//!
//! [`gray`] is re-exported from [`crate::gray`] so callers can use the
//! permanent-grouped path `gf2_algebra::permanent::gray::gray_code_iter`
//! that the W1-T6 contract names, while the underlying module also
//! remains reachable as `gf2_algebra::gray` per
//! `dev/plans/d1a_gf2_algebra_boundary.md` §4.2.

pub mod bipedal3;
pub mod reference;
pub mod ryser;

pub use bipedal3::permanent_bipedal3;
pub use reference::permanent_mod3_reference;
pub use ryser::permanent_ryser;

/// Re-export of [`crate::gray`] so the canonical W1-T6 API
/// `gf2_algebra::permanent::gray::gray_code_iter` resolves.
pub use crate::gray;

#[cfg(feature = "f5")]
pub mod bipedal5;

#[cfg(feature = "f7")]
pub mod bipedal7;
