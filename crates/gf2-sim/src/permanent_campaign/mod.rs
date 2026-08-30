//! Permanent-zero-fraction campaign orchestration contracts.
//!
//! The [`schema`] module is the canonical typed description of the published
//! dataset boundary. [`provenance`] adds the two rules that make such a dataset
//! trustworthy: the source-identity guard an emitting binary passes before it
//! writes, and the integrity file a reader re-checks it against. Sampling and
//! statistical estimators live outside this module; this orchestration layer
//! only describes their durable records.

pub mod acceptance;
pub mod coordinator;
#[cfg(test)]
pub(crate) mod fixture;
pub mod launch_cost;
pub mod provenance;
pub mod schedule;
pub mod schema;
