//! Permanent-zero-fraction campaign orchestration contracts.
//!
//! The [`schema`] module is the canonical typed description of the published
//! dataset boundary. [`provenance`] binds a writer to its live executable bytes
//! and committed manifest, while [`coordinator`] owns exact-cell admission,
//! receipt persistence, sampling, and terminal field projections under one
//! campaign lock. The source revision is provenance context rather than
//! executable input or an API boundary. Reusable permanent mathematics belongs
//! to `gf2-algebra`; this module owns reusable campaign orchestration and its
//! durable records. The `permanent_campaign` binary is a thin compiled caller
//! of this library API.

pub mod acceptance;
pub mod coordinator;
#[cfg(test)]
pub(crate) mod fixture;
pub mod launch_cost;
pub mod provenance;
pub(crate) mod root_fs;
pub mod schedule;
pub mod schema;
pub mod validation;
