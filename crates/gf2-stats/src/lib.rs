//! Statistics primitives for reproducible finite-field campaigns.
//!
//! This crate is the narrow home for the campaign sampler, interval estimators,
//! exact tests, and streaming accumulator. The sampler is available in
//! [`sampler`]; the remaining surfaces intentionally land in their own modules
//! in later changes.

#![deny(unsafe_code)]

pub mod binomial;
pub mod intervals;
pub mod sampler;

mod numerics;
