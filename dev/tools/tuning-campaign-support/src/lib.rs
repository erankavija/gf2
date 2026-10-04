//! Crate-neutral mechanics shared by tuning campaign producers and by the
//! Zen 3 benchmark protocol tooling. Owner crates hold all tuning policy and
//! semantic validation; this crate depends on no gf2 selector type.

#![forbid(unsafe_code)]

pub mod abtest;
pub mod arm;
pub mod campaign;
pub mod host;
pub mod journal;
pub mod process;
pub mod protocol;
pub mod provenance;
pub mod receipt;
pub mod repository;
pub mod schema;
pub mod seed;
pub mod statistics;
pub mod timing;
pub mod transport;

#[cfg(feature = "test-support")]
pub mod scratch;

pub mod trial_ledger;
