//! Published OSD receipt representation, retained under `@/issue/779d1926`.
//! Identity tokens come from `experiments_common`; invocation is metadata.

use experiments_common::{GitRevision, RngAlgorithm, Sha256Digest};
use serde::{Deserialize, Deserializer, Serialize};

/// Explicit availability state for optional mechanical provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability<T> {
    /// The value was available and recorded.
    Present {
        /// Recorded version or hardware model.
        value: T,
    },
    /// No matching runtime or hardware was present.
    NotPresent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AvailabilityWire<T> {
    state: AvailabilityState,
    value: Option<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AvailabilityState {
    Present,
    NotPresent,
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Availability<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = AvailabilityWire::deserialize(deserializer)?;
        match (wire.state, wire.value) {
            (AvailabilityState::Present, Some(value)) => Ok(Self::Present { value }),
            (AvailabilityState::NotPresent, None) => Ok(Self::NotPresent),
            (AvailabilityState::Present, None) => {
                Err(serde::de::Error::custom("present state requires value"))
            }
            (AvailabilityState::NotPresent, Some(_)) => {
                Err(serde::de::Error::custom("not_present state forbids value"))
            }
        }
    }
}

/// Mechanical provenance required to regenerate and audit the dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Repository-wide revision observed at run start, recorded as context
    /// rather than as the dataset's identity.
    pub git_revision: GitRevision,
    /// SHA-256 digest of the executable that emitted the dataset, present from
    /// schema version 2 onward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_sha256: Option<Sha256Digest>,
    /// Revision of the linked source closure (`crates/`, `experiments/` and `Cargo.lock`),
    /// present from schema version 2 onward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deps_source_revision: Option<GitRevision>,
    /// Whether the linked source closure was dirty at run start, present from
    /// schema version 2 onward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deps_source_dirty: Option<bool>,
    /// Complete compiler version string.
    pub compiler_version: String,
    /// Closed RNG algorithm identity.
    pub rng_algorithm: RngAlgorithm,
    /// Exact RNG crate or implementation version.
    pub rng_version: String,
    /// Exact producer invocation as unquoted argument tokens; non-identity metadata.
    pub invocation: Vec<String>,
    /// Accelerator runtime version, or an explicit absent state.
    pub accelerator_runtime: Availability<String>,
    /// Processor model.
    pub cpu_model: String,
    /// Runtime-observed number of physical processor cores, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_physical_cores: Option<u32>,
    /// Runtime-observed number of logical processor threads, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_logical_threads: Option<u32>,
    /// Accelerator model, or an explicit absent state.
    pub gpu_model: Availability<String>,
}
