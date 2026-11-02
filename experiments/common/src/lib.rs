//! Identity, repository-root, and durable-publish mechanics shared by
//! permanent-statistics and osd-ebch.

#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
mod build_profile;
mod identity;
mod publish;
mod repository;
mod run;

pub use identity::{ArtifactPath, GitRevision, IdentityError, Sha256Digest};
pub use publish::{create_durable, publish_atomic, DurabilityUnconfirmed};
pub use repository::RepositoryRoot;
pub use run::{
    AcceleratorIdentity, BuildFlags, CpuIdentity, HostFacts, RngAlgorithm, RngIdentity,
    RunIdentity, RunRecord,
};
