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
//! - [`SoGrand`]: Soft-Output GRAND — wraps ORBGRAND to produce per-bit APP LLRs
//!   and extrinsic information for turbo decoding.
//!
//! # References
//!
//! - Duffy, K.R., Li, J., Medard, M. (2019). "Capacity-Achieving Guessing Random
//!   Additive Noise Decoding." *IEEE Trans. Inform. Theory*.
//! - Solomon, A., Duffy, K.R., Medard, M. (2020). "Soft Maximum Likelihood Decoding
//!   using GRAND." *IEEE ISIT*.
//! - Condo, C., et al. (2022). "Fixed Complexity Soft-Output GRAND." *IEEE Trans.
//!   Commun.*

pub(crate) mod orbgrand;
mod sogrand;

pub use orbgrand::{OneLineIntercept, OrbGrand, OrbGrandConfig, OrbGrandResult, ScoredCodeword};
pub use sogrand::{SisoResult, SoGrand};
