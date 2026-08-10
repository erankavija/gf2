#![forbid(unsafe_code)]
//! Candidate registry for the wave-parallel permanent HIP study.
//!
//! [`MeasurementPath::ALL`] is the study's sole candidate list. Each path
//! dispatches to a dedicated module so later implementations do not contend on
//! the registry files.

mod f5_candidates;
mod f7_three_plane;
pub mod fixtures;
mod fold_gf3;
pub mod oracle;
pub mod paths;
mod wave;
mod wave_gf7;

pub use paths::{DispatchResult, EvaluationResult, MeasurementPath, Unsupported};
