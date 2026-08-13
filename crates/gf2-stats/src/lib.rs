//! Statistics primitives for reproducible finite-field campaigns.
//!
//! This crate is the narrow home for the campaign sampler ([`sampler`]),
//! interval estimators ([`intervals`]), exact binomial tests ([`binomial`]),
//! and the streaming shard accumulator ([`accumulator`]).

#![deny(unsafe_code)]

pub mod accumulator;
pub mod binomial;
pub mod intervals;
pub mod sampler;

mod numerics;
