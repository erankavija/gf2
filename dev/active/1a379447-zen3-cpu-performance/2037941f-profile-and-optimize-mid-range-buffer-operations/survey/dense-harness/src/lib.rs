//! Dense-parity measurement harness for story `2037941f` (jit:e1f9a78f).
//!
//! The harness implements the frozen addendum `2037941f-dense-parity-v2`: its
//! questions, their exact cell matrices, its seeding rule, its cache policies
//! and its untimed correctness oracle. It also carries the lane-threshold
//! question of addendum `4337c02e-matvec-lane-threshold-v1`, measured at the
//! allocated `matvec` boundary by the same arms, and the native comparator
//! question of addendum `63bad95d-native-matvec-vs-m4ri-v1`, measured at the
//! comparator boundary with the gf2 arm built at the `native` level. It
//! changes no production code and no production selection.
//!
//! Every numeric setting reaches the harness from a frozen addendum
//! ([`cells`]) or from `tuning_campaign_support::protocol::SHARED_SETTINGS`.
//! The launcher adds none.
//!
//! The interface this crate publishes is described in the
//! entry document `dense-parity-harness.md`.

pub mod campaign;
pub mod cells;
pub mod external;
pub mod fixture;
pub mod inputs;
pub mod oracle;
pub mod routes;
pub mod wire;

pub use cells::{
    cells, Cache, Cell, M4riShape, MatvecShape, Question, Workload, ADDENDUM_IDENTITY,
    ADDENDUM_PATH, ADDENDUM_SHA256, CAMPAIGN_SEED,
};
