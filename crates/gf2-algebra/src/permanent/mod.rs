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
//! W1-T1 skeleton — only the module tree is present. The Ryser driver
//! and reference impl land in W1 (T4-T6); the bipedal fast paths in W3
//! (T8-T11) and W4 (T16-T21).

pub mod bipedal3;
pub mod reference;
pub mod ryser;

#[cfg(feature = "f5")]
pub mod bipedal5;

#[cfg(feature = "f7")]
pub mod bipedal7;
