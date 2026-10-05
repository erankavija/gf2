//! Statistics primitives for reproducible finite-field campaigns: the campaign
//! sampler ([`sampler`]), interval estimators ([`intervals`]), exact binomial
//! tests ([`binomial`]), the streaming shard accumulator ([`accumulator`]), and
//! the exact reduction of weighted runs over exponent histograms ([`weighted`]).

#![deny(unsafe_code)]

pub mod accumulator;
pub mod binomial;
pub mod intervals;
pub mod sampler;
pub mod weighted;

mod numerics;
