//! Generic statistical methods for interval estimation, exact binomial tests,
//! and weighted-run reduction.

#![deny(unsafe_code)]

pub mod binomial;
pub mod intervals;
pub mod weighted;

mod numerics;
