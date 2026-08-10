#![forbid(unsafe_code)]
//! Candidate registry for the wave-parallel permanent HIP study.
//!
//! [`MeasurementPath::ALL`] is the study's sole candidate list. Each path
//! dispatches to a dedicated module so later implementations do not contend on
//! the registry files.

mod f5_candidates;
mod f7_three_plane;
#[cfg(feature = "fixture-oracle")]
pub mod fixtures;
mod fold_gf3;
#[cfg(feature = "fixture-oracle")]
pub mod oracle;
pub mod paths;
mod wave;
mod wave_gf7;

#[cfg(feature = "fixture-oracle")]
pub use paths::EvaluationResult;
pub use paths::{DispatchResult, MeasurementPath, Unsupported};
