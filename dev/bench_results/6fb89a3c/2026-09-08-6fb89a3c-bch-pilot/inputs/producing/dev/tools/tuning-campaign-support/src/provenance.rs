//! Content identities for measurement-producing source and build inputs.
//!
//! A producing-input manifest selects the files whose bytes can affect a
//! campaign. Capturing it hashes the manifest itself and every selected file;
//! repository metadata and files outside the manifest do not enter the
//! identity.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::journal::atomic_write_new;

const SNAPSHOT_PENDING: &str = ".producing-snapshot.pending.json";
const SNAPSHOT_COMPLETE: &str = "producing-snapshot.json";

/// Schema shared by producing-input manifests.
pub const PRODUCING_INPUTS_SCHEMA: &str = "tuning-campaign-producing-inputs-v1";

/// Declarative closure of files that produce measurement evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducingInputs {
    pub schema: String,
    pub behavior_sources: Vec<String>,
    pub lifecycle_sources: Vec<String>,
    pub build_inputs: Vec<String>,
}

/// Exact content snapshot selected by a [`ProducingInputs`] manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducingSnapshot {
    /// Repository-relative manifest path.
    pub manifest_path: String,
    /// Digest of the exact manifest bytes.
    pub manifest_sha256: String,
    /// Per-file behavior-source digests.
    pub behavior_sha256: BTreeMap<String, String>,
    /// Per-file lifecycle-source digests.
    pub lifecycle_sha256: BTreeMap<String, String>,
    /// Per-file complete build-input digests.
    pub build_inputs_sha256: BTreeMap<String, String>,
}

impl ProducingInputs {
    /// Reads and validates a manifest under `root`.
    pub fn read_at(root: &Path, manifest_path: &str) -> io::Result<Self> {
        validate_relative(manifest_path, "producing-input manifest")?;
        let value: Self =
            serde_json::from_slice(&fs::read(root.join(manifest_path))?).map_err(|error| {
                invalid(format!("producing-input manifest does not decode: {error}"))
            })?;
        value.validate_at(root)?;
        Ok(value)
    }

    /// Validates schema, ordering, paths, regular-file identity, and subset
    /// relationships at `root`.
    pub fn validate_at(&self, root: &Path) -> io::Result<()> {
        if self.schema != PRODUCING_INPUTS_SCHEMA {
            return Err(invalid("producing input manifest schema mismatch"));
        }
        for paths in [
            &self.behavior_sources,
            &self.lifecycle_sources,
            &self.build_inputs,
        ] {
            if paths.is_empty() || paths.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(invalid(
                    "source manifest paths must be nonempty, sorted and unique",
                ));
            }
            for path in paths {
                validate_relative(path, "source manifest")?;
                let metadata = fs::symlink_metadata(root.join(path))?;
                if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                    return Err(invalid("source manifest path is not a regular file"));
                }
            }
        }
        if self
            .behavior_sources
            .iter()
            .any(|path| !self.build_inputs.contains(path))
            || self
                .lifecycle_sources
                .iter()
                .any(|path| !self.behavior_sources.contains(path))
        {
            return Err(invalid(
                "lifecycle/behavior manifest is not a subset of producing inputs",
            ));
        }
        Ok(())
    }

    /// Hashes `paths` at `root` into a sorted repository-relative map.
    pub fn hashes_at(root: &Path, paths: &[String]) -> io::Result<BTreeMap<String, String>> {
        paths
            .iter()
            .map(|path| Ok((path.clone(), sha256_hex(&fs::read(root.join(path))?))))
            .collect()
    }

    /// Captures the manifest and every selected file by content.
    pub fn capture(root: &Path, manifest_path: &str) -> io::Result<ProducingSnapshot> {
        let manifest = Self::read_at(root, manifest_path)?;
        Ok(ProducingSnapshot {
            manifest_path: manifest_path.to_owned(),
            manifest_sha256: sha256_hex(&fs::read(root.join(manifest_path))?),
            behavior_sha256: Self::hashes_at(root, &manifest.behavior_sources)?,
            lifecycle_sha256: Self::hashes_at(root, &manifest.lifecycle_sources)?,
            build_inputs_sha256: Self::hashes_at(root, &manifest.build_inputs)?,
        })
    }

    /// Captures every selected input beneath `snapshot_root`, preserving
    /// repository-relative paths. If the manifest snapshot already exists,
    /// the frozen snapshot is reopened without consulting `source_root`.
    pub fn capture_to(
        source_root: &Path,
        manifest_path: &str,
        snapshot_root: &Path,
    ) -> io::Result<ProducingSnapshot> {
        fs::create_dir_all(snapshot_root)?;
        let complete_path = snapshot_root.join(SNAPSHOT_COMPLETE);
        if complete_path.try_exists()? {
            let expected: ProducingSnapshot = serde_json::from_slice(&fs::read(&complete_path)?)
                .map_err(|error| {
                    invalid(format!(
                        "producing snapshot marker does not decode: {error}"
                    ))
                })?;
            if expected.manifest_path != manifest_path {
                return Err(invalid("producing snapshot marker names another manifest"));
            }
            Self::verify_snapshot(snapshot_root, &expected)?;
            return Ok(expected);
        }

        let pending_path = snapshot_root.join(SNAPSHOT_PENDING);
        let expected = if pending_path.try_exists()? {
            serde_json::from_slice(&fs::read(&pending_path)?).map_err(|error| {
                invalid(format!(
                    "pending producing snapshot does not decode: {error}"
                ))
            })?
        } else {
            let captured = Self::capture(source_root, manifest_path)?;
            atomic_write_new(
                &pending_path,
                &serde_json::to_vec(&captured).map_err(|error| invalid(error.to_string()))?,
            )?;
            captured
        };
        if expected.manifest_path != manifest_path {
            return Err(invalid("pending producing snapshot names another manifest"));
        }

        let manifest = Self::read_at(source_root, manifest_path)?;
        let mut paths = Vec::with_capacity(manifest.build_inputs.len() + 1);
        paths.push(manifest_path.to_owned());
        paths.extend(manifest.build_inputs);
        for path in paths {
            let source_bytes = fs::read(source_root.join(&path))?;
            let expected_sha = if path == manifest_path {
                &expected.manifest_sha256
            } else {
                expected
                    .build_inputs_sha256
                    .get(&path)
                    .ok_or_else(|| invalid("pending producing snapshot omits a build input"))?
            };
            if sha256_hex(&source_bytes) != *expected_sha {
                return Err(invalid(format!(
                    "source input {path} changed during snapshot publication"
                )));
            }
            let destination = snapshot_root.join(&path);
            if destination.try_exists()? {
                if sha256_hex(&fs::read(&destination)?) != *expected_sha {
                    return Err(invalid(format!(
                        "partial producing snapshot input {path} differs"
                    )));
                }
            } else {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                atomic_write_new(&destination, &source_bytes)?;
            }
        }
        Self::verify_snapshot(snapshot_root, &expected)?;
        atomic_write_new(
            &complete_path,
            &serde_json::to_vec(&expected).map_err(|error| invalid(error.to_string()))?,
        )?;
        Ok(expected)
    }

    /// Verifies that a portable snapshot still contains exactly the recorded
    /// selected-content identity.
    pub fn verify_snapshot(snapshot_root: &Path, expected: &ProducingSnapshot) -> io::Result<()> {
        let observed = Self::capture(snapshot_root, &expected.manifest_path)?;
        if &observed != expected {
            return Err(invalid("producing-input snapshot content differs"));
        }
        Ok(())
    }
}

impl ProducingSnapshot {
    /// Digest of the canonical selected-content snapshot.
    pub fn identity_sha256(&self) -> io::Result<String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| invalid(format!("producing snapshot does not encode: {error}")))?;
        Ok(sha256_hex(&bytes))
    }

    /// Digest of the lifecycle source map used by the shared resume identity.
    pub fn lifecycle_sha256(&self) -> io::Result<String> {
        let bytes = serde_json::to_vec(&self.lifecycle_sha256)
            .map_err(|error| invalid(format!("lifecycle snapshot does not encode: {error}")))?;
        Ok(sha256_hex(&bytes))
    }
}

fn validate_relative(path: &str, kind: &str) -> io::Result<()> {
    let path_buf = PathBuf::from(path);
    if path.is_empty()
        || path_buf.is_absolute()
        || path_buf
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        || path.contains(['*', '?', '[', ']'])
    {
        return Err(invalid(format!(
            "{kind} contains a nonliteral repository path"
        )));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
