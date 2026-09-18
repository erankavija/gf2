//! Dense-parity measurement harness for story `2037941f` (jit:e1f9a78f).
//!
//! The harness implements the frozen addendum `2037941f-dense-parity-v1`: its
//! questions, their exact cell matrices, its seeding rule, its cache policies
//! and its untimed correctness oracle. It changes no production code and no
//! production selection.
//!
//! Every numeric setting reaches the harness from the frozen addendum
//! ([`cells`]) or from `tuning_campaign_support::protocol::SHARED_SETTINGS`.
//! The launcher adds none.
//!
//! The interface this crate publishes is described in
//! `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-harness.md`.

pub mod campaign;
pub mod cells;
pub mod external;
pub mod fixture;
pub mod inputs;
pub mod oracle;
pub mod routes;
pub mod smoke;
pub mod wire;

pub use cells::{
    cells, Cache, Cell, M4riShape, MatvecShape, Question, Workload, ADDENDUM_IDENTITY,
    ADDENDUM_PATH, ADDENDUM_SHA256, CAMPAIGN_SEED,
};
