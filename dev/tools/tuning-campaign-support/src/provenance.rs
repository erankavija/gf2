//! Content identities for measurement-producing source and build inputs. A
//! producing-input manifest selects the files whose bytes can affect a
//! campaign. Capturing it hashes the manifest itself and every selected file;
//! repository metadata and files outside the manifest do not enter the
//! identity.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Schema shared by producing-input manifests.
pub const PRODUCING_INPUTS_SCHEMA: &str = "tuning-campaign-producing-inputs-v1";

/// Repository-relative producing-input manifest selected by a runner plan.
///
/// Construction rejects absolute paths, traversal and glob patterns. The manifest
/// is read and its selected files validated by [`ProducingInputs`] at capture time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProducingManifestPath(String);

impl TryFrom<String> for ProducingManifestPath {
    type Error = io::Error;

    fn try_from(path: String) -> io::Result<Self> {
        validate_relative(&path, "producing-input manifest")?;
        Ok(Self(path))
    }
}

impl From<ProducingManifestPath> for String {
    fn from(path: ProducingManifestPath) -> Self {
        path.0
    }
}

impl ProducingManifestPath {
    /// Returns the validated repository-relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Declarative closure of files that produce measurement evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducingInputs {
    pub schema: String,
    pub behavior_sources: Vec<String>,
    pub lifecycle_sources: Vec<String>,
    pub build_inputs: Vec<String>,
}

/// Input digests selected by a [`ProducingInputs`] manifest.
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
}

impl ProducingSnapshot {
    /// Digest of the canonical input-digest record.
    pub fn identity_sha256(&self) -> io::Result<String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| invalid(format!("producing digests do not encode: {error}")))?;
        Ok(sha256_hex(&bytes))
    }

    /// Digest of the lifecycle source map used by the shared resume identity.
    pub fn lifecycle_sha256(&self) -> io::Result<String> {
        let bytes = serde_json::to_vec(&self.lifecycle_sha256)
            .map_err(|error| invalid(format!("lifecycle digests do not encode: {error}")))?;
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
