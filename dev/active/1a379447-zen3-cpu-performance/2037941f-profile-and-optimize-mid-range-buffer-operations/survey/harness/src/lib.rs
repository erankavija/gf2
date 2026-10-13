//! Logical-buffer measurement harness for story `2037941f` (jit:bb769456).
//!
//! The harness implements the frozen addendum `2037941f-logical-buffer-v1`:
//! its four questions, their exact cell matrices, its seeding rule, its cache
//! policies and its untimed correctness oracle. It changes no production code
//! and no production selection.
//!
//! Every numeric setting reaches the harness from the frozen addendum
//! ([`cells`]) or from `tuning_campaign_support::protocol::SHARED_SETTINGS`.
//! The launcher adds none.
//!
//! The interface this crate publishes is described in the
//! entry document `logical-harness.md`.

pub mod campaign;
pub mod cells;
pub mod fixture;
pub mod oracle;
pub mod routes;
pub mod wire;

pub use cells::{
    cells, Cache, Cell, Layout, NrTarget, Question, RowShape, Workload, ADDENDUM_IDENTITY,
    ADDENDUM_PATH, ADDENDUM_SHA256, CAMPAIGN_SEED,
};
