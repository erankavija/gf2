//! Guessing Random Additive Noise Decoding (GRAND) family of decoders.
//!
//! This module provides implementations of the GRAND decoder family,
//! which are universal maximum-likelihood decoders for short block codes.
//! Unlike traditional decoders that are code-specific, GRAND decoders
//! work with any linear block code by testing noise patterns in decreasing
//! likelihood order.
//!
//! # Available Decoders
//!
//! - [`OrbGrand`]: Ordered Reliability Bits GRAND — uses soft information (LLRs)
//!   to order noise pattern queries by logistic weight, achieving near-ML performance.
//!
//! # References
//!
//! - Duffy, K.R., Li, J., Medard, M. (2019). "Capacity-Achieving Guessing Random
//!   Additive Noise Decoding." *IEEE Trans. Inform. Theory*.
//! - Solomon, A., Duffy, K.R., Medard, M. (2020). "Soft Maximum Likelihood Decoding
//!   using GRAND." *IEEE ISIT*.

mod orbgrand;

pub use orbgrand::{OrbGrand, OrbGrandConfig, OrbGrandResult, ScoredCodeword};
