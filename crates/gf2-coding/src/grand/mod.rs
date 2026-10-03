//! Guessing Random Additive Noise Decoding (`@/citation/Duffy2019`,
//! `@/citation/Solomon2020`): decoders that work with any linear block code by
//! testing noise patterns in decreasing likelihood order.  [`OrbGrand`] orders
//! queries by logistic weight; [`SoGrand`] adds per-bit soft output
//! (`@/citation/Yuan2025`).

pub(crate) mod orbgrand;
mod sogrand;

pub use orbgrand::{OneLineIntercept, OrbGrand, OrbGrandConfig, OrbGrandResult, ScoredCodeword};
pub use sogrand::{SisoResult, SoGrand};
