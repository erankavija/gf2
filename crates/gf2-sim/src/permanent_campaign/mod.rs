//! Permanent-zero-fraction campaign orchestration contracts.
//!
//! Permanent mathematics belongs to `gf2-algebra`; this module owns campaign
//! orchestration and its durable records.

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
