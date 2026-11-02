//! Permanent sampling, validation, and permanental-rank importance trajectories.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod rare_event;

mod execution_log;

/// RNG implementation shared by permanent experiments.
pub fn sampler_rng() -> experiments_common::RngIdentity {
    experiments_common::RngIdentity {
        algorithm: experiments_common::RngAlgorithm::ChaCha20,
        version: "rand_chacha 0.9.0".into(),
    }
}

pub mod campaign;
