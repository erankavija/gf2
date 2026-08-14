#![forbid(unsafe_code)]
//! Candidate registry for the wave-parallel permanent HIP study.
//!
//! [`MeasurementPath::ALL`] is the study's sole candidate list. Each path
//! dispatches to a dedicated module so later implementations do not contend on
//! the registry files.

pub mod device_batch;
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

pub use device_batch::{BatchEvaluation, DeviceBatchKernel, DeviceSpans};
#[cfg(feature = "fixture-oracle")]
pub use paths::EvaluationResult;
pub use paths::{DeviceBatchResult, MeasurementPath, Unsupported};
/// Largest F_3 order whose exhaustive fixture oracle is part of the ordinary
/// wave-candidate evidence. Larger corpus rows remain explicitly unavailable.
#[cfg(feature = "fixture-oracle")]
pub use wave::MAX_HOST_FIXTURE_ORDER as WAVE_GF3_MAX_FIXTURE_ORDER;
