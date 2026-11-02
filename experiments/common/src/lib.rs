//! Mechanics shared by the experiments under `experiments/`: identity types,
//! the run identity of `@/inv/behavioral-evidence-validity`, one repository
//! root per `@/inv/no-dev-path-coupling`, and durable file publish. Every item
//! serves both `@/issue/fd1ef344` and `@/issue/779d1926`.

#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
mod build_profile;
mod identity;
mod publish;
mod repository;
mod run;

pub use identity::{ArtifactIdentity, ArtifactPath, GitRevision, IdentityError, Sha256Digest};
pub use publish::{create_durable, is_interrupted_publish, publish_atomic, DurabilityUnconfirmed};
pub use repository::RepositoryRoot;
pub use run::{
    AcceleratorIdentity, BuildFlags, CpuIdentity, HostFacts, RngAlgorithm, RngIdentity,
    RunIdentity, RunRecord,
};
