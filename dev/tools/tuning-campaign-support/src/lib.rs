//! Crate-neutral mechanics shared by tuning campaign producers and by the
//! Zen 3 benchmark protocol tooling.
//!
//! This development-only crate deliberately knows nothing about gf2 selector
//! types or installation. Owner crates retain all tuning policy and semantic
//! validation; this crate supplies the byte framing, timing, deterministic
//! seed, empirical selection, journal, checkpoint, host-observation, process
//! and paired A/B statistics primitives around it.

#![forbid(unsafe_code)]

pub mod abtest;
pub mod campaign;
pub mod host;
pub mod journal;
pub mod process;
pub mod protocol;
pub mod provenance;
pub mod receipt;
pub mod schema;
pub mod seed;
pub mod statistics;
pub mod timing;
pub mod transport;

/// Protocol-v2 append-only family attempt ledger.
pub mod trial_ledger;
