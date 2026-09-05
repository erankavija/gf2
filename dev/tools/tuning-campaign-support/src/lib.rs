//! Crate-neutral mechanics shared by tuning campaign producers.
//!
//! This development-only crate deliberately knows nothing about gf2 selector
//! types or installation. Owner crates retain all tuning policy and semantic
//! validation; this crate supplies the byte framing, timing, deterministic
//! seed, empirical selection, journal, and checkpoint primitives around it.

#![forbid(unsafe_code)]

pub mod campaign;
pub mod journal;
pub mod seed;
pub mod statistics;
pub mod timing;
pub mod transport;
