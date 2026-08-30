//! Source identity and integrity for published campaign datasets.
//!
//! Two independent guarantees live here. [`approve_emission`] decides whether
//! the running binary may publish into a campaign directory at all: it checks
//! the executable identity named by the frozen manifest and refuses when that
//! manifest differs from its committed content. [`verify_dataset`] decides,
//! later and from the published bytes alone, whether the dataset still matches its
//! [`INTEGRITY_FILE`](super::schema::INTEGRITY_FILE) and whether the revision it
//! names still exists.
//!
//! Emission protects the identity of the executable and the frozen manifest,
//! not repository-wide cleanliness. Runtime source facts are captured in the
//! source-closure fields and the repository-wide revision is retained as
//! context; neither context nor unrelated output is an emission gate.
//!
//! The on-disk integrity format and its `sha256sum -c` verification procedure
//! are documented in
//! `dev/simulation_results/permanent-zero-fraction/README.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::root_fs::CampaignRoot;
use super::schema::{
    field_summary_file, read_field_summary, read_manifest, shard_record_file, ArtifactPath,
    ArtifactPathError, Availability, CampaignId, CampaignManifest, CellTerminalState, DatasetFileClass,
    DatasetLayout, GitRevision, GitRevisionError, Provenance, SchemaError, Sha256Digest,
    Sha256DigestError, DATASET_HOME, INTEGRITY_FILE, MANIFEST_FILE,
};

/// Permission for the running binary to publish into one exact campaign.
///
/// Only [`approve_emission`] constructs this opaque token. It binds the live
/// executable, canonical campaign directory, repository, and committed
/// manifest bytes checked by the guard.
#[derive(Debug, Clone)]
pub struct EmissionApproval {
    binary_sha256: Sha256Digest,
    campaign_root: CampaignRoot,
    repository_root: PathBuf,
    manifest_sha256: Sha256Digest,
    _mapped_executable: Arc<File>,
}

impl EmissionApproval {
    /// Returns the digest of the executable approved to emit.
    pub fn binary_sha256(&self) -> &Sha256Digest {
        &self.binary_sha256
    }

    pub(crate) fn campaign_root(&self) -> &CampaignRoot {
        &self.campaign_root
    }

    pub(crate) fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    pub(crate) fn manifest_sha256(&self) -> &Sha256Digest {
        &self.manifest_sha256
    }
}

/// Result of checking a supplied or omitted executable identity.
///
/// This value is informational and cannot authorize sampler entry. The
/// campaign writer requires [`EmissionApproval`], which only the live
/// executable check returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionInspection {
    binary_sha256: Sha256Digest,
}

impl EmissionInspection {
    /// Returns the executable digest checked or declared by the manifest.
    pub fn binary_sha256(&self) -> &Sha256Digest {
        &self.binary_sha256
    }
}

struct VerifiedEmission {
    binary_sha256: Sha256Digest,
    campaign_root: CampaignRoot,
    repository_root: PathBuf,
    manifest_sha256: Sha256Digest,
}

/// Why a path inside the repository is not one campaign's directory.
///
/// The root names the frozen manifest the guard verifies against, so a root
/// that is not one campaign's directory names no single manifest and leaves
/// the guard nothing to check. The accepted shape is therefore exactly
/// `<repository>/<DATASET_HOME>/<campaign-id>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignPathFault {
    /// The path is not under the dataset home at all.
    OutsideDatasetHome,
    /// The path is the dataset home itself, or an ancestor of it, so it
    /// contains campaigns rather than being one.
    AboveCampaignDirectory,
    /// The path lies deeper than the one campaign-id level below the home.
    BelowCampaignDirectory,
    /// The one component below the home is not a valid campaign id.
    InvalidCampaignId,
}

impl fmt::Display for CampaignPathFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideDatasetHome => {
                write!(
                    f,
                    "a campaign lives under {DATASET_HOME}, and this does not"
                )
            }
            Self::AboveCampaignDirectory => write!(
                f,
                "{DATASET_HOME} and its ancestors contain campaigns rather than being one"
            ),
            Self::BelowCampaignDirectory => write!(
                f,
                "a campaign is one directory below {DATASET_HOME}, and this is deeper"
            ),
            Self::InvalidCampaignId => {
                f.write_str("the directory name below the dataset home is not a campaign id")
            }
        }
    }
}

/// Why a binary may not publish into a campaign directory.
///
/// Every variant refuses: an inconclusive check is a refusal, because a dataset
/// whose source cannot be established is exactly what the guard exists to
/// prevent.
#[derive(Debug)]
pub enum EmissionRefusal {
    /// The running executable's digest could not be determined.
    UnknownBinaryDigest {
        /// Diagnostic explaining why the digest was unavailable.
        message: String,
    },
    /// The running executable differs from the identity frozen in the manifest.
    BinaryDigestMismatch {
        /// Digest claimed by the frozen manifest.
        expected: Sha256Digest,
        /// Digest observed for the running executable.
        actual: Sha256Digest,
    },
    /// The frozen manifest predates binary-scoped provenance and declares no digest.
    UndeclaredBinaryDigest {
        /// Manifest path that declares no binary digest.
        path: PathBuf,
    },
    /// The frozen manifest differs from the committed manifest content.
    ManifestChanged {
        /// Manifest path that differs from committed content.
        path: PathBuf,
    },
    /// The campaign directory is not inside the repository being checked.
    OutsideRepository {
        /// Campaign directory that was requested.
        path: PathBuf,
    },
    /// The path is inside the repository but is not one campaign's directory.
    NotACampaignDirectory {
        /// Campaign directory that was requested.
        path: PathBuf,
        /// What disqualifies it.
        fault: CampaignPathFault,
    },
    /// A `git` invocation could not be run or reported failure.
    Git {
        /// Arguments passed to `git`.
        command: String,
        /// Diagnostic reported by `git`, or by the attempt to run it.
        message: String,
    },
    /// A filesystem operation needed by the check failed.
    Io {
        /// Path being resolved.
        path: PathBuf,
        /// Underlying operating-system error.
        source: std::io::Error,
    },
    /// The frozen manifest is not a schema-conformant dataset manifest.
    Schema(SchemaError),
}

impl fmt::Display for EmissionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBinaryDigest { message } => {
                write!(f, "cannot determine the running executable SHA-256: {message}")
            }
            Self::BinaryDigestMismatch { expected, actual } => write!(
                f,
                "the frozen manifest names binary SHA-256 {expected}, but the running executable is {actual}"
            ),
            Self::UndeclaredBinaryDigest { path } => write!(
                f,
                "frozen campaign manifest {} declares no binary SHA-256",
                path.display()
            ),
            Self::ManifestChanged { path } => write!(
                f,
                "frozen campaign manifest {} differs from its committed content",
                path.display()
            ),
            Self::OutsideRepository { path } => write!(
                f,
                "campaign directory {} is outside the repository being checked",
                path.display()
            ),
            Self::NotACampaignDirectory { path, fault } => {
                write!(f, "{} is not a campaign directory: {fault}", path.display())
            }
            Self::Git { command, message } => {
                write!(f, "`git {command}` failed: {message}")
            }
            Self::Io { path, source } => write!(f, "cannot resolve {}: {source}", path.display()),
            Self::Schema(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for EmissionRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Schema(source) => Some(source),
            _ => None,
        }
    }
}

/// Decides whether the running executable may publish into `campaign_root`.
///
/// Emission is approved when the running executable has a determinable digest,
/// the frozen manifest is committed without modification, and its claimed
/// binary digest equals the running executable's digest. Repository-wide git
/// state is recorded as context by the producer and is not an emission gate.
///
/// `campaign_root` must be one campaign's directory: exactly one
/// [`CampaignId`] level below [`DATASET_HOME`] inside the repository. A root
/// that is merely somewhere inside the repository is refused, because it names
/// no single frozen manifest for the guard to verify against. The frozen
/// campaign directory and manifest must already exist.
///
/// # Errors
///
/// Returns the [`EmissionRefusal`] that blocked publication. An unusable
/// repository, an unreadable path, or a failing `git` invocation refuses too:
/// the guard never approves a check it could not complete.
pub fn approve_emission(campaign_root: &Path) -> Result<EmissionApproval, EmissionRefusal> {
    let (mapped_executable, binary_sha256) = mapped_executable_identity()?;
    let verified = verify_emission(Some(binary_sha256), campaign_root)?;
    Ok(EmissionApproval {
        binary_sha256: verified.binary_sha256,
        campaign_root: verified.campaign_root,
        repository_root: verified.repository_root,
        manifest_sha256: verified.manifest_sha256,
        _mapped_executable: Arc::new(mapped_executable),
    })
}

/// Returns the SHA-256 digest of the running executable.
fn running_binary_sha256() -> Result<Sha256Digest, EmissionRefusal> {
    mapped_executable_identity().map(|(_, digest)| digest)
}

#[cfg(target_os = "linux")]
fn mapped_executable_identity() -> Result<(File, Sha256Digest), EmissionRefusal> {
    let file =
        File::open("/proc/self/exe").map_err(|error| EmissionRefusal::UnknownBinaryDigest {
            message: format!("cannot open mapped executable handle: {error}"),
        })?;
    let mut reader = &file;
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| EmissionRefusal::UnknownBinaryDigest {
            message: format!("cannot hash mapped executable handle: {error}"),
        })?;
    Ok((file, digest_of(&bytes)))
}

#[cfg(not(target_os = "linux"))]
fn mapped_executable_identity() -> Result<(File, Sha256Digest), EmissionRefusal> {
    Err(EmissionRefusal::UnknownBinaryDigest {
        message: "this platform does not provide the required mapped-image handle binding"
            .to_owned(),
    })
}

/// Inspects emission-guard predicates for an optionally supplied digest.
///
/// `Some` checks the supplied digest as the emitting binary's identity. `None`
/// verifies every other emission guard and uses the frozen manifest's own
/// digest as the identity under test; this is the inspection mode for callers
/// that do not have the writer executable available. In either mode the
/// manifest is compared with its committed bytes before it is parsed, so a
/// changed manifest remains a [`EmissionRefusal::ManifestChanged`] refusal.
pub fn inspect_emission_with_binary_digest(
    binary_sha256: Option<Sha256Digest>,
    campaign_root: &Path,
) -> Result<EmissionInspection, EmissionRefusal> {
    let verified = verify_emission(binary_sha256, campaign_root)?;
    Ok(EmissionInspection {
        binary_sha256: verified.binary_sha256,
    })
}

fn verify_emission(
    binary_sha256: Option<Sha256Digest>,
    campaign_root: &Path,
) -> Result<VerifiedEmission, EmissionRefusal> {
    let campaign_anchor =
        CampaignRoot::open(campaign_root).map_err(|source| EmissionRefusal::Io {
            path: campaign_root.to_owned(),
            source,
        })?;
    let anchor = campaign_anchor.path();
    let repository = canonicalize(Path::new(&run_git(
        anchor,
        &["rev-parse", "--show-toplevel"],
    )?))?;

    let prefix = campaign_prefix(anchor, anchor, &repository)?;
    let manifest_path = campaign_root.join(MANIFEST_FILE);
    let manifest_bytes = campaign_anchor
        .read(Path::new(MANIFEST_FILE))
        .map_err(|_| EmissionRefusal::ManifestChanged {
            path: manifest_path.clone(),
        })?;
    let committed = run_git_bytes(
        &repository,
        &["show", &format!("HEAD:{prefix}{MANIFEST_FILE}")],
    )
    .map_err(|_| EmissionRefusal::ManifestChanged {
        path: manifest_path.clone(),
    })?;
    if manifest_bytes != committed {
        return Err(EmissionRefusal::ManifestChanged {
            path: manifest_path,
        });
    }
    let manifest = super::schema::read_manifest_bytes(&manifest_bytes, &manifest_path)
        .map_err(EmissionRefusal::Schema)?;
    let Some(expected) = manifest.provenance.binary_sha256 else {
        return Err(EmissionRefusal::UndeclaredBinaryDigest {
            path: manifest_path,
        });
    };
    let binary_sha256 = if let Some(actual) = binary_sha256 {
        if expected != actual {
            return Err(EmissionRefusal::BinaryDigestMismatch { expected, actual });
        }
        actual
    } else {
        expected
    };
    Ok(VerifiedEmission {
        binary_sha256,
        campaign_root: campaign_anchor,
        repository_root: repository,
        manifest_sha256: digest_of(&manifest_bytes),
    })
}

/// Returns the campaign directory as a repository-relative `/`-terminated prefix.
///
/// The prefix addresses that campaign's committed manifest as
/// `HEAD:<prefix>manifest.json`, the copy the guard compares the on-disk
/// manifest against. Being somewhere inside the repository is not enough: only
/// exactly one [`CampaignId`] directory below [`DATASET_HOME`] names one
/// campaign's frozen manifest, so anything else refuses and names which of
/// those conditions it broke.
fn campaign_prefix(
    anchor: &Path,
    campaign_root: &Path,
    repository: &Path,
) -> Result<String, EmissionRefusal> {
    let outside = || EmissionRefusal::OutsideRepository {
        path: campaign_root.to_owned(),
    };
    let mut campaign = canonicalize(anchor)?;
    if !campaign_root.is_dir() {
        campaign.push(campaign_root.file_name().ok_or_else(outside)?);
    }
    let relative = campaign.strip_prefix(repository).map_err(|_| outside())?;

    let mut components = Vec::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(outside());
        };
        components.push(name.to_str().ok_or_else(outside)?);
    }

    let home: Vec<&str> = DATASET_HOME.split('/').collect();
    let reject = |fault| {
        Err(EmissionRefusal::NotACampaignDirectory {
            path: campaign_root.to_owned(),
            fault,
        })
    };
    if components.len() <= home.len() {
        // An ancestor of the home, or the home itself, contains campaigns
        // rather than being one; anything else short is simply elsewhere.
        return if home.starts_with(&components) {
            reject(CampaignPathFault::AboveCampaignDirectory)
        } else {
            reject(CampaignPathFault::OutsideDatasetHome)
        };
    }
    if !components.starts_with(&home) {
        return reject(CampaignPathFault::OutsideDatasetHome);
    }
    if components.len() > home.len() + 1 {
        return reject(CampaignPathFault::BelowCampaignDirectory);
    }
    if components[home.len()].parse::<CampaignId>().is_err() {
        return reject(CampaignPathFault::InvalidCampaignId);
    }

    let mut prefix = components.join("/");
    prefix.push('/');
    Ok(prefix)
}

fn canonicalize(path: &Path) -> Result<PathBuf, EmissionRefusal> {
    fs::canonicalize(path).map_err(|source| EmissionRefusal::Io {
        path: path.to_owned(),
        source,
    })
}

/// Runs a git command and preserves its stdout bytes.
fn run_git_bytes(directory: &Path, arguments: &[&str]) -> Result<Vec<u8>, EmissionRefusal> {
    let refuse = |message: String| EmissionRefusal::Git {
        command: arguments.join(" "),
        message,
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|source| refuse(source.to_string()))?;
    if !output.status.success() {
        return Err(refuse(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(output.stdout)
}

/// Runs `git` in `directory` and returns its stdout without the trailing newline.
fn run_git(directory: &Path, arguments: &[&str]) -> Result<String, EmissionRefusal> {
    let refuse = |message: String| EmissionRefusal::Git {
        command: arguments.join(" "),
        message,
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        // The guard only reads; refreshing the index on disk would dirty a
        // repository it was asked to inspect.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|source| refuse(source.to_string()))?;
    if !output.status.success() {
        return Err(refuse(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim_end_matches(['\n', '\r']).to_owned())
        .map_err(|source| refuse(source.to_string()))
}

/// Runtime-observed source facts for a measurement artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSourceIdentity {
    /// Repository-wide revision observed at run start, retained as context.
    pub git_revision: GitRevision,
    /// Whether any repository path was dirty at run start, retained as context.
    pub git_dirty: bool,
    /// Revision of the source closure (`crates/` and `Cargo.lock`).
    pub deps_source_revision: GitRevision,
    /// Whether the source closure was dirty at run start.
    pub deps_source_dirty: bool,
}

/// Processor identity and topology observed on the producing host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCpuIdentity {
    /// Processor model string reported by the operating system.
    pub model: String,
    /// Number of distinct physical processor cores.
    pub physical_cores: u32,
    /// Number of logical processor threads.
    pub logical_threads: u32,
}

/// Accelerator runtime and model identity observed on the producing host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAcceleratorIdentity {
    /// Installed ROCm runtime version, or an explicit absent state.
    pub runtime: Availability<String>,
    /// AMD accelerator product name, or an explicit absent state.
    pub model: Availability<String>,
}

/// Observes the producing host's accelerator identity without running a
/// kernel or opening a campaign stream.
///
/// The runtime version comes from ROCm's installation metadata. The model is
/// read from the Linux DRM sysfs inventory and is reported only for an AMD GPU
/// with a non-empty product name. Missing metadata is an explicit absent state,
/// not a hand-written placeholder.
pub fn observe_accelerator_identity() -> RuntimeAcceleratorIdentity {
    let runtime = fs::read_to_string("/opt/rocm/.info/version")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map_or(Availability::NotPresent, |value| Availability::Present { value });
    let model = fs::read_dir("/sys/class/drm")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("card"))
        .find_map(|entry| {
            let device = entry.path().join("device");
            let vendor = fs::read_to_string(device.join("vendor")).ok()?;
            if vendor.trim() != "0x1002" {
                return None;
            }
            fs::read_to_string(device.join("product_name"))
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        })
        .map_or(Availability::NotPresent, |value| Availability::Present { value });
    RuntimeAcceleratorIdentity { runtime, model }
}

/// Observes the producing host's processor model and core/thread topology.
///
/// Linux uses `/proc/cpuinfo`, counting distinct `(physical id, core id)`
/// pairs and processor records. Platforms without that interface use the
/// corresponding `sysctl` model and topology values. Linux falls back to sysfs
/// topology identifiers when `/proc/cpuinfo` omits package or core identifiers.
///
/// # Errors
///
/// Returns [`std::io::ErrorKind::InvalidData`] when the operating system does
/// not expose a non-empty model and a positive, internally consistent topology.
pub fn observe_cpu_identity() -> std::io::Result<RuntimeCpuIdentity> {
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        return parse_proc_cpuinfo(&cpuinfo);
    }

    let model = sysctl_value("machdep.cpu.brand_string")
        .or_else(|| sysctl_value("hw.model"))
        .ok_or_else(|| invalid_cpu_data("runtime CPU model probe is unavailable"))?;
    let physical_cores = sysctl_value("hw.physicalcpu")
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_cpu_data("runtime physical CPU core-count probe is unavailable"))?;
    let logical_threads = sysctl_value("hw.logicalcpu")
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_cpu_data("runtime logical CPU thread-count probe is unavailable"))?;
    validate_cpu_identity(RuntimeCpuIdentity {
        model,
        physical_cores,
        logical_threads,
    })
}

fn parse_proc_cpuinfo(cpuinfo: &str) -> std::io::Result<RuntimeCpuIdentity> {
    let model = cpuinfo
        .lines()
        .filter_map(cpuinfo_field)
        .find_map(|(name, value)| {
            matches!(name, "model name" | "Processor" | "Hardware").then(|| value.to_owned())
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_cpu_data("/proc/cpuinfo does not contain a processor model"))?;
    let logical_threads = cpuinfo
        .lines()
        .filter_map(cpuinfo_field)
        .filter(|(name, _)| *name == "processor")
        .count();
    let physical_core_ids: BTreeSet<_> = cpuinfo
        .split("\n\n")
        .filter_map(|processor| {
            let mut package = None;
            let mut core = None;
            for (name, value) in processor.lines().filter_map(cpuinfo_field) {
                match name {
                    "physical id" => package = Some(value.to_owned()),
                    "core id" => core = Some(value.to_owned()),
                    _ => {}
                }
            }
            package.zip(core)
        })
        .collect();
    let physical_cores = if physical_core_ids.is_empty() {
        linux_sysfs_physical_core_ids()?.len()
    } else {
        physical_core_ids.len()
    };
    validate_cpu_identity(RuntimeCpuIdentity {
        model,
        physical_cores: u32::try_from(physical_cores)
            .map_err(|_| invalid_cpu_data("physical CPU core count exceeds u32"))?,
        logical_threads: u32::try_from(logical_threads)
            .map_err(|_| invalid_cpu_data("logical CPU thread count exceeds u32"))?,
    })
}

fn linux_sysfs_physical_core_ids() -> std::io::Result<BTreeSet<(String, String)>> {
    let mut core_ids = BTreeSet::new();
    for entry in fs::read_dir("/sys/devices/system/cpu")? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(index) = name.strip_prefix("cpu") else {
            continue;
        };
        if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let topology = entry.path().join("topology");
        let package = fs::read_to_string(topology.join("physical_package_id"))?;
        let core = fs::read_to_string(topology.join("core_id"))?;
        core_ids.insert((package.trim().to_owned(), core.trim().to_owned()));
    }
    if core_ids.is_empty() {
        return Err(invalid_cpu_data(
            "Linux CPU topology exposes no physical core identifiers",
        ));
    }
    Ok(core_ids)
}

fn validate_cpu_identity(identity: RuntimeCpuIdentity) -> std::io::Result<RuntimeCpuIdentity> {
    if identity.model.trim().is_empty()
        || identity.physical_cores == 0
        || identity.logical_threads < identity.physical_cores
    {
        return Err(invalid_cpu_data(
            "runtime CPU identity contains an invalid model or topology",
        ));
    }
    Ok(identity)
}

fn cpuinfo_field(line: &str) -> Option<(&str, &str)> {
    line.split_once(':')
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn sysctl_value(name: &str) -> Option<String> {
    Command::new("sysctl")
        .args(["-n", name])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn invalid_cpu_data(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

/// Captures source identity at runtime for a measurement artifact.
pub fn observe_source_identity(
    repository: &Path,
) -> Result<RuntimeSourceIdentity, EmissionRefusal> {
    let repository = repository_top_level(repository)?;
    let git_revision = run_git(&repository, &["rev-parse", "HEAD"])?
        .parse()
        .map_err(|error: GitRevisionError| EmissionRefusal::Git {
            command: ["rev-parse", "HEAD"].join(" "),
            message: error.to_string(),
        })?;
    let deps_source_revision = run_git(
        &repository,
        &["log", "-1", "--format=%H", "--", "crates/", "Cargo.lock"],
    )?
    .parse()
    .map_err(|error: GitRevisionError| EmissionRefusal::Git {
        command: "log -1 --format=%H -- crates/ Cargo.lock".to_owned(),
        message: error.to_string(),
    })?;
    let git_dirty = !run_git(
        &repository,
        &["status", "--porcelain", "--untracked-files=all"],
    )?
    .is_empty();
    let deps_source_dirty = !run_git(
        &repository,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "crates/",
            "Cargo.lock",
        ],
    )?
    .is_empty();
    Ok(RuntimeSourceIdentity {
        git_revision,
        git_dirty,
        deps_source_revision,
        deps_source_dirty,
    })
}

/// Resolves any path inside a Git repository to its top-level directory.
pub fn repository_top_level(path: &Path) -> Result<PathBuf, EmissionRefusal> {
    let anchor = canonicalize(path)?;
    let git_marker = anchor.join(".git");
    if git_marker.is_dir() || git_marker.is_file() {
        return Ok(anchor);
    }
    canonicalize(Path::new(&run_git(
        &anchor,
        &["rev-parse", "--show-toplevel"],
    )?))
}

/// Returns the repository-wide revision observed at runtime by this command.
pub fn runtime_git_revision() -> Result<GitRevision, EmissionRefusal> {
    let repository = canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    observe_source_identity(&repository).map(|identity| identity.git_revision)
}

/// Records runtime-observed identity fields in a provenance value.
pub fn observe_provenance(
    repository: &Path,
    mut provenance: Provenance,
) -> Result<Provenance, EmissionRefusal> {
    let identity = observe_source_identity(repository)?;
    provenance.git_revision = identity.git_revision;
    provenance.binary_sha256 = Some(running_binary_sha256()?);
    provenance.deps_source_revision = Some(identity.deps_source_revision);
    provenance.deps_source_dirty = Some(identity.deps_source_dirty);
    Ok(provenance)
}

/// One raw dataset file and the digest recorded for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityEntry {
    /// Path relative to the campaign directory.
    pub path: ArtifactPath,
    /// SHA-256 over the file's bytes.
    pub sha256: Sha256Digest,
}

/// A malformed line in an integrity file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityFormatError {
    /// One-based line number, or zero for a fault the file has as a whole
    /// rather than at one line.
    pub line: usize,
    /// What the line, or the file, violates.
    pub message: String,
}

impl fmt::Display for IntegrityFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for IntegrityFormatError {}

/// A dataset file that disagrees with its integrity file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityFault {
    /// A path the dataset should hold is absent from it.
    Missing {
        /// Absent path.
        path: ArtifactPath,
    },
    /// A recorded path is present with different content.
    Changed {
        /// Recorded path.
        path: ArtifactPath,
        /// Digest the integrity file records.
        recorded: Sha256Digest,
        /// Digest the file has now.
        actual: Sha256Digest,
    },
    /// A raw dataset file is present but the integrity file omits it.
    Uncovered {
        /// Uncovered path.
        path: ArtifactPath,
    },
    /// The integrity file records a path that is not raw dataset content.
    OutsideRawSet {
        /// Recorded path.
        path: ArtifactPath,
    },
}

impl fmt::Display for IntegrityFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(f, "{path} is missing"),
            Self::Changed {
                path,
                recorded,
                actual,
            } => write!(f, "{path} changed from {recorded} to {actual}"),
            Self::Uncovered { path } => write!(f, "{path} is not covered by the integrity file"),
            Self::OutsideRawSet { path } => {
                write!(f, "{path} is covered but is not raw dataset content")
            }
        }
    }
}

/// Why a dataset's source identity could not be decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnverifiableReason {
    /// The recorded revision names no commit in the repository holding the
    /// dataset.
    UnknownRevision {
        /// Revision the manifest records.
        revision: GitRevision,
    },
    /// The dataset is not inside a readable git repository, so its recorded
    /// revision cannot be resolved at all.
    UnresolvableRepository {
        /// Diagnostic from the attempt to resolve the repository.
        message: String,
    },
}

impl fmt::Display for UnverifiableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRevision { revision } => write!(
                f,
                "recorded revision {revision} names no commit in this repository"
            ),
            Self::UnresolvableRepository { message } => {
                write!(f, "the dataset is not in a readable repository: {message}")
            }
        }
    }
}

/// Outcome of checking a published dataset against its integrity file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatasetVerdict {
    /// Every covered path matched its recorded digest, the raw set is covered
    /// exactly, and the recorded revision resolves.
    Verified,
    /// The dataset and its integrity file disagree.
    Failed {
        /// Every disagreement, ordered by path.
        faults: Vec<IntegrityFault>,
    },
    /// The recorded source revision could not be resolved, so the dataset's
    /// provenance is undecided whatever its bytes say. Any content faults found
    /// alongside are reported here rather than discarded.
    Unverifiable {
        /// Why the revision could not be resolved.
        reason: UnverifiableReason,
        /// Content faults found before provenance was decided.
        faults: Vec<IntegrityFault>,
    },
}

impl fmt::Display for DatasetVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verified => f.write_str("verified"),
            Self::Failed { faults } => {
                f.write_str("integrity check failed:")?;
                for fault in faults {
                    write!(f, " {fault};")?;
                }
                Ok(())
            }
            Self::Unverifiable { reason, faults } => {
                write!(f, "unverifiable: {reason}")?;
                for fault in faults {
                    write!(f, "; {fault}")?;
                }
                Ok(())
            }
        }
    }
}

/// A failure that prevented the integrity layer from reaching a verdict.
///
/// A verdict of [`DatasetVerdict::Failed`] is an answer; these are the cases
/// where no answer could be computed at all.
#[derive(Debug)]
pub enum IntegrityError {
    /// A filesystem operation failed.
    Io {
        /// Path being accessed.
        path: PathBuf,
        /// Underlying operating-system error.
        source: std::io::Error,
    },
    /// A raw file the manifest requires is absent while generating coverage.
    MissingRawFile {
        /// Absent path.
        path: PathBuf,
    },
    /// An integrity file could not be parsed.
    Format {
        /// Integrity file path.
        path: PathBuf,
        /// Offending line and reason.
        source: IntegrityFormatError,
    },
    /// The root manifest could not be read or did not conform.
    Schema(SchemaError),
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::MissingRawFile { path } => write!(
                f,
                "raw dataset file {} is required by the manifest but absent",
                path.display()
            ),
            Self::Format { path, source } => {
                write!(f, "{} is not an integrity file: {source}", path.display())
            }
            Self::Schema(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for IntegrityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Format { source, .. } => Some(source),
            Self::Schema(source) => Some(source),
            Self::MissingRawFile { .. } => None,
        }
    }
}

impl From<SchemaError> for IntegrityError {
    fn from(source: SchemaError) -> Self {
        Self::Schema(source)
    }
}

/// Renders the integrity file covering exactly the raw data present in `root`.
///
/// The returned text is what finalization writes to
/// [`INTEGRITY_FILE`](super::schema::INTEGRITY_FILE). Coverage is the raw-data
/// half of [`DatasetLayout::from_manifest`]: the root manifest, every executed
/// shard record, every field summary, and the pooled summary. Derived artefacts
/// and the integrity file itself are excluded, so the file can close. Shard
/// paths a halted cell never executed are absent from the dataset and are
/// skipped; every other planned raw path must exist.
///
/// Entries are sorted by path, so the same dataset always renders the same
/// bytes. Each file is read whole, which is bounded by the dataset's own size.
///
/// # Errors
///
/// Returns [`IntegrityError::MissingRawFile`] when a raw path the manifest
/// requires is absent without a halted cell to account for it,
/// [`IntegrityError::Schema`] when a field summary cannot be read — an
/// undecidable halt state exempts nothing — and [`IntegrityError::Io`] when a
/// file cannot be read.
pub fn generate_integrity_file(
    root: &Path,
    manifest: &CampaignManifest,
) -> Result<String, IntegrityError> {
    let unexecuted = unexecuted_shard_paths(root, manifest)?;
    let mut covered = BTreeMap::new();
    for file in DatasetLayout::from_manifest(manifest).required_files() {
        if file.class != DatasetFileClass::RawData {
            continue;
        }
        let path = root.join(&file.relative_path);
        if !path.is_file() {
            if unexecuted.contains(&file.relative_path) {
                continue;
            }
            return Err(IntegrityError::MissingRawFile { path });
        }
        covered.insert(
            dataset_path(root, &file.relative_path)?,
            file_digest(&path)?,
        );
    }
    let entries: Vec<_> = covered
        .into_iter()
        .map(|(path, sha256)| IntegrityEntry { path, sha256 })
        .collect();
    Ok(encode_integrity_file(&entries))
}

/// Returns the shard paths a halted cell may legitimately never have written.
///
/// What makes an absent shard legitimate is not its path but its cell's
/// recorded terminal state: a halted cell may hold any subset of its planned
/// shards, while a completed cell requires every one of them. The set is
/// therefore derived from the field summaries the manifest declares, and it
/// names exact paths rather than a prefix, so a lost shard of a completed cell
/// can never fall through it.
///
/// Every declared field summary must be present and readable. An unreadable
/// summary decides nothing about which shards are legitimately absent, and
/// treating it as an exemption would reopen the same hole through a wider
/// door, so this fails closed.
fn unexecuted_shard_paths(
    root: &Path,
    manifest: &CampaignManifest,
) -> Result<BTreeSet<String>, IntegrityError> {
    let fields: BTreeSet<_> = manifest.cells.iter().map(|cell| cell.q).collect();
    let mut halted = BTreeSet::new();
    for q in fields {
        let path = root.join(field_summary_file(q));
        if !path.is_file() {
            return Err(IntegrityError::MissingRawFile { path });
        }
        let summary = read_field_summary(root, q)?;
        for item in &summary.quarantined {
            let known = manifest.cells.iter().any(|cell| {
                cell.q == item.q
                    && cell.n == item.n
                    && cell
                        .shards
                        .iter()
                        .any(|shard| shard.shard_id == item.shard_id)
            });
            if !known {
                return Err(IntegrityError::Schema(SchemaError::InvalidValue {
                    path: path.clone(),
                    message: "quarantined shard is absent from the manifest".to_owned(),
                }));
            }
        }
        for row in summary.rows {
            if matches!(row.terminal_state, CellTerminalState::Halted { .. }) {
                halted.insert((row.q, row.n));
            }
        }
    }
    Ok(manifest
        .cells
        .iter()
        .filter(|cell| halted.contains(&(cell.q, cell.n)))
        .flat_map(|cell| {
            cell.shards
                .iter()
                .map(|shard| shard_record_file(cell.q, cell.n, shard.shard_id))
        })
        .collect())
}

/// Renders integrity entries in the `sha256sum` check-file format.
///
/// Each line is the lowercase hexadecimal digest, two spaces, and the path
/// relative to the campaign directory, so `sha256sum -c` accepts the file
/// unchanged when run from that directory. Entries are emitted in the order
/// given; [`generate_integrity_file`] sorts them by path first.
pub fn encode_integrity_file(entries: &[IntegrityEntry]) -> String {
    let mut text = String::new();
    for entry in entries {
        text.push_str(entry.sha256.as_str());
        text.push_str("  ");
        text.push_str(entry.path.as_str());
        text.push('\n');
    }
    text
}

/// Parses an integrity file in the `sha256sum` check-file format.
///
/// Both coreutils separators are accepted: two spaces for text mode and a space
/// followed by `*` for binary mode, which produce identical digests. Blank
/// lines are ignored and a trailing carriage return is tolerated. A recorded
/// path must be a normalized relative path, so verifying an untrusted dataset
/// cannot be steered outside its own directory.
///
/// # Errors
///
/// Returns the first line that carries no digest, no separator, a
/// non-canonical digest, an unnormalized path, or a path already recorded.
pub fn decode_integrity_file(text: &str) -> Result<Vec<IntegrityEntry>, IntegrityFormatError> {
    let mut entries = Vec::new();
    let mut recorded = BTreeSet::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let malformed = |message: String| IntegrityFormatError { line, message };
        let record = raw.trim_end_matches('\r');
        if record.is_empty() {
            continue;
        }
        let (digest, rest) = record
            .split_at_checked(64)
            .ok_or_else(|| malformed("line is shorter than a SHA-256 digest".to_owned()))?;
        let sha256: Sha256Digest = digest
            .parse()
            .map_err(|error: Sha256DigestError| malformed(error.to_string()))?;
        let path = rest
            .strip_prefix("  ")
            .or_else(|| rest.strip_prefix(" *"))
            .ok_or_else(|| {
                malformed(
                    "digest and path must be separated by two spaces, or by a space and `*`"
                        .to_owned(),
                )
            })?;
        let path: ArtifactPath = path
            .parse()
            .map_err(|error: ArtifactPathError| malformed(error.to_string()))?;
        if !recorded.insert(path.clone()) {
            return Err(malformed(format!("{path} is recorded more than once")));
        }
        entries.push(IntegrityEntry { path, sha256 });
    }
    Ok(entries)
}

/// Recomputes the root manifest's content hash from `manifest.json` alone.
///
/// The manifest stores no digest of itself, so this value is derived from the
/// file's bytes and never from a field inside the structure it covers. The
/// recorded counterpart lives in the integrity sidecar and is read by
/// [`recorded_manifest_hash`]; a reader compares the two.
///
/// # Errors
///
/// Returns [`IntegrityError::Io`] when the manifest cannot be read.
pub fn manifest_content_hash(root: &Path) -> Result<Sha256Digest, IntegrityError> {
    file_digest(&root.join(MANIFEST_FILE))
}

/// Reads the root manifest's content hash from the integrity sidecar.
///
/// # Errors
///
/// Returns [`IntegrityError::Io`] when the integrity file cannot be read, and
/// [`IntegrityError::Format`] when it cannot be parsed or does not cover the
/// manifest.
pub fn recorded_manifest_hash(root: &Path) -> Result<Sha256Digest, IntegrityError> {
    read_integrity_file(root)?
        .into_iter()
        .find(|entry| entry.path.as_str() == MANIFEST_FILE)
        .map(|entry| entry.sha256)
        .ok_or_else(|| IntegrityError::Format {
            path: root.join(INTEGRITY_FILE),
            source: IntegrityFormatError {
                line: 0,
                message: format!("the integrity file does not cover {MANIFEST_FILE}"),
            },
        })
}

/// Re-checks a published dataset against its integrity file and its source.
///
/// The manifest is authenticated first: it declares the layout every other
/// check depends on, so a manifest that is missing, uncovered, or changed is
/// reported on its own rather than used to derive a file set that can no longer
/// be trusted. Otherwise every recorded path is hashed and compared, every
/// present raw path is required to be covered, and the revision the manifest
/// records is resolved in the repository holding the dataset.
///
/// Each file is read whole, which is bounded by the dataset's own size.
///
/// # Errors
///
/// Returns [`IntegrityError`] when no verdict could be computed — an unreadable
/// file, an unparsable integrity file, or a manifest that does not conform.
pub fn verify_dataset(root: &Path) -> Result<DatasetVerdict, IntegrityError> {
    let recorded: BTreeMap<_, _> = read_integrity_file(root)?
        .into_iter()
        .map(|entry| (entry.path, entry.sha256))
        .collect();

    if let Some(fault) = manifest_fault(root, &recorded)? {
        return Ok(DatasetVerdict::Failed {
            faults: vec![fault],
        });
    }
    let manifest = read_manifest(root)?;
    let mut raw = BTreeSet::new();
    for file in DatasetLayout::from_manifest(&manifest).required_files() {
        if file.class == DatasetFileClass::RawData {
            raw.insert(dataset_path(root, &file.relative_path)?);
        }
    }

    let mut faults = Vec::new();
    for (path, digest) in &recorded {
        if !raw.contains(path) {
            faults.push(IntegrityFault::OutsideRawSet { path: path.clone() });
            continue;
        }
        let file = root.join(path.as_str());
        if !file.is_file() {
            faults.push(IntegrityFault::Missing { path: path.clone() });
            continue;
        }
        let actual = file_digest(&file)?;
        if &actual != digest {
            faults.push(IntegrityFault::Changed {
                path: path.clone(),
                recorded: digest.clone(),
                actual,
            });
        }
    }
    // A path the manifest declares but the integrity file never lists is a
    // fault in its own right, whether or not it is still on disk. Without this
    // a sidecar that simply omitted a lost shard would verify clean. An
    // undecidable halt state exempts nothing, so a summary that cannot be read
    // — which is itself reported above as a changed or missing raw file —
    // leaves every absent shard reported rather than excused.
    let unexecuted = unexecuted_shard_paths(root, &manifest).unwrap_or_default();
    for path in &raw {
        if recorded.contains_key(path) {
            continue;
        }
        if root.join(path.as_str()).is_file() {
            faults.push(IntegrityFault::Uncovered { path: path.clone() });
        } else if !unexecuted.contains(path.as_str()) {
            faults.push(IntegrityFault::Missing { path: path.clone() });
        }
    }
    faults.sort_by(|left, right| fault_path(left).cmp(fault_path(right)));

    let revision = &manifest.provenance.git_revision;
    Ok(match resolve_revision(root, revision) {
        RevisionStatus::Resolved if faults.is_empty() => DatasetVerdict::Verified,
        RevisionStatus::Resolved => DatasetVerdict::Failed { faults },
        RevisionStatus::Absent => DatasetVerdict::Unverifiable {
            reason: UnverifiableReason::UnknownRevision {
                revision: revision.clone(),
            },
            faults,
        },
        RevisionStatus::Unresolvable(message) => DatasetVerdict::Unverifiable {
            reason: UnverifiableReason::UnresolvableRepository { message },
            faults,
        },
    })
}

/// Returns the fault that makes the root manifest untrustworthy, if any.
fn manifest_fault(
    root: &Path,
    recorded: &BTreeMap<ArtifactPath, Sha256Digest>,
) -> Result<Option<IntegrityFault>, IntegrityError> {
    let path = dataset_path(root, MANIFEST_FILE)?;
    if !root.join(MANIFEST_FILE).is_file() {
        return Ok(Some(IntegrityFault::Missing { path }));
    }
    let Some(digest) = recorded.get(&path) else {
        return Ok(Some(IntegrityFault::Uncovered { path }));
    };
    let actual = manifest_content_hash(root)?;
    Ok((&actual != digest).then(|| IntegrityFault::Changed {
        path,
        recorded: digest.clone(),
        actual,
    }))
}

/// Whether the repository holding a dataset still has its recorded revision.
enum RevisionStatus {
    /// The revision names a commit in the repository.
    Resolved,
    /// The repository is readable and has no such commit.
    Absent,
    /// No repository could be resolved from the dataset's location.
    Unresolvable(String),
}

fn resolve_revision(root: &Path, revision: &GitRevision) -> RevisionStatus {
    if let Err(message) = git_probe(root, &["rev-parse", "--show-toplevel"]) {
        return RevisionStatus::Unresolvable(message);
    }
    match git_probe(root, &["cat-file", "-e", &format!("{revision}^{{commit}}")]) {
        Ok(()) => RevisionStatus::Resolved,
        Err(_) => RevisionStatus::Absent,
    }
}

/// Runs `git` for its exit status alone, reporting the diagnostic on failure.
fn git_probe(directory: &Path, arguments: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|source| source.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn fault_path(fault: &IntegrityFault) -> &ArtifactPath {
    match fault {
        IntegrityFault::Missing { path }
        | IntegrityFault::Changed { path, .. }
        | IntegrityFault::Uncovered { path }
        | IntegrityFault::OutsideRawSet { path } => path,
    }
}

fn read_integrity_file(root: &Path) -> Result<Vec<IntegrityEntry>, IntegrityError> {
    let path = root.join(INTEGRITY_FILE);
    let text = fs::read_to_string(&path).map_err(|source| IntegrityError::Io {
        path: path.clone(),
        source,
    })?;
    decode_integrity_file(&text).map_err(|source| IntegrityError::Format { path, source })
}

/// Parses a layout-declared path into the portable dataset path grammar.
fn dataset_path(root: &Path, relative: &str) -> Result<ArtifactPath, IntegrityError> {
    relative
        .parse()
        .map_err(|error: ArtifactPathError| IntegrityError::Format {
            path: root.join(MANIFEST_FILE),
            source: IntegrityFormatError {
                line: 0,
                message: error.to_string(),
            },
        })
}

fn file_digest(path: &Path) -> Result<Sha256Digest, IntegrityError> {
    let bytes = fs::read(path).map_err(|source| IntegrityError::Io {
        path: path.to_owned(),
        source,
    })?;
    Ok(digest_of(&bytes))
}

fn digest_of(bytes: &[u8]) -> Sha256Digest {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    text.parse()
        .expect("64 lowercase hexadecimal characters are a canonical digest")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::fixture::{
        manifest_at_revision, unique_temp_dir, write_fixture_at_revision, write_halted_fixture,
        write_integrity_file, write_version_1_fixture, TestDir, FIXTURE_CAMPAIGN_ID,
    };
    use super::super::schema::POOLED_SUMMARY_FILE;
    use super::*;

    const SOURCE_FILE: &str = "crates/gf2-sim/src/permanent_campaign/schema.rs";
    const DEPENDENCY_MANIFEST: &str = "crates/gf2-sim/Cargo.toml";
    const FIRST_SHARD: &str = "shards/q3/n04/shard-000000.json";
    const SECOND_SHARD: &str = "shards/q3/n04/shard-000001.json";
    const DERIVED_REPORT: &str = "derived/report.md";

    /// Returns the frozen protocol's path, derived from the one dataset home.
    fn protocol_document() -> String {
        format!("{DATASET_HOME}/protocol.md")
    }

    /// A throwaway repository with the shape the guard reasons about.
    ///
    /// The guard is driven against these, never against the repository the
    /// tests are running inside, so no test depends on or mutates this
    /// checkout's working tree.
    struct TestRepo {
        root: PathBuf,
    }

    impl TestRepo {
        fn new() -> Self {
            let root = unique_temp_dir("gf2-sim-provenance");
            fs::create_dir_all(&root).expect("create throwaway repository");
            let repo = Self { root };
            repo.git(&["init", "--quiet", "--initial-branch=main"]);
            repo.write(SOURCE_FILE, "pub fn permanent() {}\n");
            repo.write(DEPENDENCY_MANIFEST, "[package]\nname = \"gf2-sim\"\n");
            repo.write(&protocol_document(), "# frozen preregistration\n");
            repo.commit_all("seed the throwaway repository");
            repo
        }

        fn git(&self, arguments: &[&str]) -> String {
            let output = Command::new("git")
                .arg("-C")
                .arg(&self.root)
                .args(arguments)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_AUTHOR_NAME", "fixture")
                .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
                .env("GIT_COMMITTER_NAME", "fixture")
                .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
                .output()
                .expect("git drives the source-identity rule");
            assert!(
                output.status.success(),
                "git {arguments:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)
                .expect("git output is UTF-8")
                .trim()
                .to_owned()
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path(relative);
            fs::create_dir_all(path.parent().expect("relative path has a parent")).unwrap();
            fs::write(path, contents).unwrap();
        }

        fn commit_all(&self, message: &str) {
            self.git(&["add", "--all"]);
            self.git(&["commit", "--quiet", "--no-gpg-sign", "-m", message]);
        }

        fn head(&self) -> GitRevision {
            self.git(&["rev-parse", "HEAD"])
                .parse()
                .expect("HEAD is a full object name")
        }

        fn campaign_root(&self) -> PathBuf {
            self.path(&format!("{DATASET_HOME}/{FIXTURE_CAMPAIGN_ID}"))
        }

        /// Writes the conforming fixture dataset under the campaign area.
        fn write_dataset(&self) -> PathBuf {
            let root = self.campaign_root();
            fs::create_dir_all(&root).unwrap();
            write_fixture_at_revision(&root, &self.head());
            let mut manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
            manifest["provenance"]["binary_sha256"] =
                serde_json::json!(running_binary_sha256().unwrap().as_str());
            fs::write(
                root.join(MANIFEST_FILE),
                serde_json::to_vec_pretty(&manifest).unwrap(),
            )
            .unwrap();
            let manifest = read_manifest(&root).unwrap();
            write_integrity_file(&root, &manifest);
            root
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn approve(repo: &TestRepo, campaign: &Path) -> Result<EmissionApproval, EmissionRefusal> {
        let _ = repo;
        approve_emission(campaign)
    }

    fn approve_with_running_binary(
        _repo: &TestRepo,
        campaign: &Path,
    ) -> Result<EmissionApproval, EmissionRefusal> {
        approve_emission(campaign)
    }

    fn prepare_manifest_for_running_binary(repo: &TestRepo, campaign: &Path) {
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(campaign.join(MANIFEST_FILE)).unwrap()).unwrap();
        manifest["provenance"]["binary_sha256"] =
            serde_json::json!(running_binary_sha256().unwrap().as_str());
        fs::write(
            campaign.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        repo.commit_all("freeze the emitting binary identity");
    }

    #[test]
    fn emission_survives_unrelated_commit() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        prepare_manifest_for_running_binary(&repo, &campaign);

        approve_with_running_binary(&repo, &campaign).expect("the frozen emitter is approved");
        repo.write("README.md", "unrelated documentation\n");
        repo.commit_all("document unrelated work");
        approve_with_running_binary(&repo, &campaign)
            .expect("an unrelated commit cannot change the emitter");
    }

    #[test]
    fn source_identity_is_the_same_from_a_repository_subdirectory() {
        let repo = TestRepo::new();
        let root = observe_source_identity(&repo.root).expect("root identity is observable");
        let subdirectory = repo.path("crates/gf2-sim");
        let nested = observe_source_identity(&subdirectory)
            .expect("identity from a repository subdirectory is observable");

        assert_eq!(root.deps_source_revision, nested.deps_source_revision);
        assert_eq!(root.deps_source_dirty, nested.deps_source_dirty);
    }

    #[test]
    fn emission_survives_unrelated_tracked_edit() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        prepare_manifest_for_running_binary(&repo, &campaign);
        let template = read_manifest(&campaign).unwrap().provenance;
        let before = observe_provenance(&repo.root, template.clone()).unwrap();

        repo.write("README.md", "unrelated tracked edit\n");
        let after = observe_provenance(&repo.root, template).unwrap();

        // An uncommitted edit does not move HEAD, so the context revision is unchanged too.
        assert_eq!(before, after);
        approve_with_running_binary(&repo, &campaign)
            .expect("an unrelated tracked edit cannot change the emitter");
    }

    #[test]
    fn provenance_context_revision_is_the_only_field_moved_by_unrelated_commit() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        prepare_manifest_for_running_binary(&repo, &campaign);
        let template = read_manifest(&campaign).unwrap().provenance;
        let before = observe_provenance(&repo.root, template.clone()).unwrap();

        repo.write("README.md", "unrelated documentation\n");
        repo.commit_all("advance unrelated context");
        let after = observe_provenance(&repo.root, template).unwrap();

        assert_eq!(before.binary_sha256, after.binary_sha256);
        assert_eq!(before.deps_source_revision, after.deps_source_revision);
        assert_eq!(before.deps_source_dirty, after.deps_source_dirty);
        assert_ne!(before.git_revision, after.git_revision);
    }

    #[test]
    fn emission_uses_manifest_digest_when_binary_identity_is_not_supplied() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let expected = "0000000000000000000000000000000000000000000000000000000000000000"
            .parse::<Sha256Digest>()
            .unwrap();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(campaign.join(MANIFEST_FILE)).unwrap()).unwrap();
        manifest["provenance"]["binary_sha256"] = serde_json::json!(expected.as_str());
        fs::write(
            campaign.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        repo.commit_all("freeze a different emitter identity");

        let inspection = inspect_emission_with_binary_digest(None, &campaign)
            .expect("inspection verifies every available guard predicate");
        assert_eq!(inspection.binary_sha256(), &expected);
    }

    #[test]
    fn emission_refuses_a_manifest_declaring_the_placeholder_digest() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(campaign.join(MANIFEST_FILE)).unwrap()).unwrap();
        manifest["provenance"]["binary_sha256"] =
            serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
        fs::write(
            campaign.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        repo.commit_all("freeze the placeholder emitter identity");

        let actual = running_binary_sha256().unwrap();
        let refusal = inspect_emission_with_binary_digest(Some(actual.clone()), &campaign)
            .expect_err("the placeholder digest can never name a real executable");
        match refusal {
            EmissionRefusal::BinaryDigestMismatch {
                expected,
                actual: found,
            } => {
                assert_eq!(expected.as_str(), "0".repeat(64));
                assert_eq!(found, actual);
            }
            other => panic!("unexpected refusal: {other}"),
        }
    }

    #[test]
    fn emission_refuses_a_named_emitter_with_a_mismatching_digest() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let expected = digest_of(b"pinned emitter");
        let emitter_path = repo.path("different-emitter");
        fs::write(&emitter_path, b"different emitter").unwrap();
        let actual = digest_of(&fs::read(&emitter_path).unwrap());
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(campaign.join(MANIFEST_FILE)).unwrap()).unwrap();
        manifest["provenance"]["binary_sha256"] = serde_json::json!(expected.as_str());
        fs::write(
            campaign.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        repo.commit_all("freeze the named emitter identity");

        let refusal = inspect_emission_with_binary_digest(Some(actual.clone()), &campaign)
            .expect_err("a named emitter with a different digest must refuse");
        assert!(matches!(
            refusal,
            EmissionRefusal::BinaryDigestMismatch {
                expected: found_expected,
                actual: found_actual,
            } if found_expected == expected && found_actual == actual
        ));
        let refusal = inspect_emission_with_binary_digest(Some(actual), &campaign)
            .expect_err("the mismatch refusal must name the pinned and observed digests");
        let message = refusal.to_string();
        assert!(message.contains(expected.as_str()), "{message}");
        assert!(
            message.contains(digest_of(b"different emitter").as_str()),
            "{message}"
        );
    }

    #[test]
    fn inspection_verifies_a_named_emitter_with_the_pinned_digest() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let emitter_path = repo.path("matching-emitter");
        fs::write(&emitter_path, b"the pinned emitter").unwrap();
        let expected = digest_of(&fs::read(&emitter_path).unwrap());
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(campaign.join(MANIFEST_FILE)).unwrap()).unwrap();
        manifest["provenance"]["binary_sha256"] = serde_json::json!(expected.as_str());
        fs::write(
            campaign.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        repo.commit_all("freeze the matching emitter identity");

        let inspection = inspect_emission_with_binary_digest(Some(expected.clone()), &campaign)
            .expect("a named emitter with the pinned digest verifies");
        assert_eq!(inspection.binary_sha256(), &expected);
    }

    #[test]
    fn emission_approves_a_manifest_naming_the_running_binary() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        prepare_manifest_for_running_binary(&repo, &campaign);

        let approval =
            approve_emission(&campaign).expect("the manifest names the running executable");
        assert_eq!(approval.binary_sha256(), &running_binary_sha256().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn emission_refuses_a_symlinked_campaign_ancestor() {
        use std::os::unix::fs::symlink;

        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        prepare_manifest_for_running_binary(&repo, &campaign);
        let dataset_root = campaign.parent().unwrap();
        let held_dataset = repo.path("held-dataset-root");
        fs::rename(dataset_root, &held_dataset).unwrap();
        symlink(&held_dataset, dataset_root).unwrap();

        assert!(approve_emission(&campaign).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn approval_hashes_the_mapped_executable_handle() {
        const CHILD_CAMPAIGN: &str = "GF2_MAPPED_EXECUTABLE_CAMPAIGN";
        const CHILD_LAUNCH_PATH: &str = "GF2_MAPPED_EXECUTABLE_LAUNCH_PATH";

        if let (Ok(campaign), Ok(launch_path)) = (
            std::env::var(CHILD_CAMPAIGN),
            std::env::var(CHILD_LAUNCH_PATH),
        ) {
            let replacement = PathBuf::from(&launch_path).with_extension("replacement");
            fs::write(&replacement, b"replacement executable pathname\n").unwrap();
            fs::rename(&replacement, &launch_path).unwrap();
            let approval = approve_emission(Path::new(&campaign))
                .expect("approval hashes the process's mapped image after pathname replacement");
            assert_eq!(approval.binary_sha256(), &running_binary_sha256().unwrap());
            return;
        }

        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        prepare_manifest_for_running_binary(&repo, &campaign);
        let launch_path = repo.path("mapped-executable-test");
        fs::copy(std::env::current_exe().unwrap(), &launch_path).unwrap();
        let output = Command::new(&launch_path)
            .args([
                "--exact",
                "permanent_campaign::provenance::tests::approval_hashes_the_mapped_executable_handle",
                "--nocapture",
            ])
            .env(CHILD_CAMPAIGN, &campaign)
            .env(CHILD_LAUNCH_PATH, &launch_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child stdout:\n{}\nchild stderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn emission_refuses_changed_frozen_manifest() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        prepare_manifest_for_running_binary(&repo, &campaign);
        fs::write(campaign.join(MANIFEST_FILE), b"changed frozen manifest\n").unwrap();

        let refusal = approve(&repo, &campaign).expect_err("the frozen manifest must be immutable");
        assert!(matches!(refusal, EmissionRefusal::ManifestChanged { .. }));
        let refusal = inspect_emission_with_binary_digest(None, &campaign)
            .expect_err("inspection must also reject the changed frozen manifest");
        assert!(matches!(refusal, EmissionRefusal::ManifestChanged { .. }));
    }

    #[test]
    fn dataset_verdict_unchanged_after_schema_migration() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        assert_eq!(verify_dataset(&campaign).unwrap(), DatasetVerdict::Verified);
    }

    #[test]
    fn verification_accepts_a_version_1_dataset() {
        let repo = TestRepo::new();
        let campaign = repo.campaign_root();
        fs::create_dir_all(&campaign).unwrap();
        write_version_1_fixture(&campaign, &repo.head());

        assert_eq!(verify_dataset(&campaign).unwrap(), DatasetVerdict::Verified);
    }

    #[test]
    fn conformance_accepts_a_version_1_dataset() {
        let repo = TestRepo::new();
        let campaign = repo.campaign_root();
        fs::create_dir_all(&campaign).unwrap();
        write_version_1_fixture(&campaign, &repo.head());

        super::super::schema::conform_dataset(&campaign)
            .expect("a complete version-1 dataset conforms");
    }

    #[test]
    fn version_1_manifest_carrying_binary_digest_is_invalid() {
        let repo = TestRepo::new();
        let campaign = repo.campaign_root();
        fs::create_dir_all(&campaign).unwrap();
        write_version_1_fixture(&campaign, &repo.head());
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(campaign.join(MANIFEST_FILE)).unwrap()).unwrap();
        manifest["provenance"]["binary_sha256"] = serde_json::json!("0".repeat(64));
        fs::write(
            campaign.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let error = read_manifest(&campaign).expect_err("version 1 rejects a version-2 field");
        assert!(
            matches!(&error, SchemaError::InvalidValue { message, .. }
                if message.contains("schema version 1 provenance must omit")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn version_2_manifest_omitting_binary_digest_is_invalid() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(campaign.join(MANIFEST_FILE)).unwrap()).unwrap();
        manifest["provenance"]
            .as_object_mut()
            .unwrap()
            .remove("binary_sha256");
        fs::write(
            campaign.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let error = read_manifest(&campaign).expect_err("version 2 requires the emitter digest");
        assert!(
            matches!(&error, SchemaError::InvalidValue { message, .. }
                if message.contains("schema version 2 provenance must include")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn dataset_mixing_schema_versions_is_refused() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let summary_path = campaign.join(field_summary_file(3));
        let mut summary: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary_path).unwrap()).unwrap();
        // The summary is internally consistent at version 1, so it is the
        // disagreement with the version-2 manifest that must refuse it.
        summary["schema_version"] = serde_json::json!(1);
        for row in summary["rows"].as_array_mut().unwrap() {
            row["schema_version"] = serde_json::json!(1);
        }
        fs::write(&summary_path, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();

        let error = super::super::schema::conform_dataset(&campaign)
            .expect_err("a dataset may not mix schema versions");
        assert!(
            matches!(&error, SchemaError::InvalidValue { message, .. }
                if message.contains("differs from manifest version")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn field_summary_disagreeing_with_its_own_rows_is_refused() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let summary_path = campaign.join(field_summary_file(3));
        let mut summary: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary_path).unwrap()).unwrap();
        summary["schema_version"] = serde_json::json!(1);
        fs::write(&summary_path, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();

        let error = read_field_summary(&campaign, 3)
            .expect_err("a field summary may not disagree with its own rows");
        assert!(
            matches!(&error, SchemaError::InvalidValue { message, .. }
                if message.contains("differs from field summary version")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn emission_refuses_a_version_1_campaign() {
        let repo = TestRepo::new();
        let campaign = repo.campaign_root();
        fs::create_dir_all(&campaign).unwrap();
        write_version_1_fixture(&campaign, &repo.head());
        repo.commit_all("publish a version-1 dataset");

        let refusal = approve(&repo, &campaign).expect_err("version 1 has no emitter digest");
        assert!(matches!(
            refusal,
            EmissionRefusal::UndeclaredBinaryDigest { .. }
        ));
    }

    /// REQ-02, REQ-03: the campaign's own output never refuses its own writer.
    ///
    /// Emission reads the frozen manifest and the running binary, so writing a
    /// campaign's own raw and derived files cannot refuse the writer that
    /// produces them. The derived report is committed and then changed, and a
    /// second shard added, so the test fails if a repository-state sweep is
    /// ever reintroduced ahead of the guard.
    #[test]
    fn emission_admits_a_tree_dirtied_only_by_campaign_outputs() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        fs::create_dir_all(campaign.join("derived")).unwrap();
        fs::write(campaign.join(DERIVED_REPORT), b"# derived\n").unwrap();
        repo.commit_all("publish the first dataset files");

        fs::write(campaign.join(SECOND_SHARD), b"{}\n").unwrap();
        fs::write(campaign.join(DERIVED_REPORT), b"# derived, revised\n").unwrap();
        fs::write(campaign.join("shards/q3/n04/shard-000002.json"), b"{}\n").unwrap();

        approve(&repo, &campaign).expect("a tree dirtied only by campaign output emits");
    }

    /// REQ-02: only one campaign's own directory may be emitted into.
    ///
    /// Each rejected root names no single frozen manifest, which is what the
    /// guard needs to decide emission. A tracked source file is modified
    /// throughout to show the refusal follows from the root's shape alone and
    /// not from the state of the tree around it.
    #[test]
    fn emission_refuses_non_campaign_root() {
        let repo = TestRepo::new();
        repo.write_dataset();
        repo.commit_all("publish the dataset");
        repo.write(SOURCE_FILE, "changed after the build\n");
        fs::create_dir_all(repo.path(&format!("{DATASET_HOME}/Not_A_Campaign"))).unwrap();

        let campaign = format!("{DATASET_HOME}/{FIXTURE_CAMPAIGN_ID}");
        let rejected = [
            (String::new(), CampaignPathFault::AboveCampaignDirectory),
            ("dev".to_owned(), CampaignPathFault::AboveCampaignDirectory),
            (
                "dev/simulation_results".to_owned(),
                CampaignPathFault::AboveCampaignDirectory,
            ),
            (
                DATASET_HOME.to_owned(),
                CampaignPathFault::AboveCampaignDirectory,
            ),
            (
                "crates/gf2-sim".to_owned(),
                CampaignPathFault::OutsideDatasetHome,
            ),
            (
                format!("{campaign}/shards"),
                CampaignPathFault::BelowCampaignDirectory,
            ),
            (
                format!("{DATASET_HOME}/Not_A_Campaign"),
                CampaignPathFault::InvalidCampaignId,
            ),
        ];

        for (relative, expected) in rejected {
            let root = repo.path(&relative);
            let refusal =
                approve(&repo, &root).expect_err(&format!("{relative:?} must not be emitted into"));
            match refusal {
                EmissionRefusal::NotACampaignDirectory { ref path, fault } => {
                    assert_eq!(fault, expected, "{relative:?}: {refusal}");
                    assert_eq!(path, &root, "the refusal must name the path: {refusal}");
                }
                other => panic!("{relative:?} must refuse as a path fault: {other}"),
            }
        }

        // The legitimate root still approves, once its own output is the only
        // thing dirtying the tree.
        repo.git(&["checkout", "--", SOURCE_FILE]);
        let root = repo.path(&campaign);
        fs::write(root.join(SECOND_SHARD), b"{}\n").unwrap();
        approve(&repo, &root).expect("one campaign's own directory emits");
    }

    /// REQ-02: a campaign directory outside the repository cannot be approved.
    #[test]
    fn emission_refuses_a_campaign_directory_outside_the_repository() {
        let outside = TestDir::new();

        let refusal = inspect_emission_with_binary_digest(
            Some(running_binary_sha256().unwrap()),
            outside.root(),
        )
        .expect_err("a campaign outside any repository must refuse");
        assert!(
            matches!(
                refusal,
                EmissionRefusal::Git { .. } | EmissionRefusal::OutsideRepository { .. }
            ),
            "{refusal}"
        );
    }

    /// REQ-04: the manifest hash is recomputable from the manifest alone.
    #[test]
    fn root_manifest_hash_is_recomputable_from_the_manifest_alone() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();

        let recorded = recorded_manifest_hash(&campaign).expect("the sidecar covers the manifest");
        assert_eq!(
            manifest_content_hash(&campaign).expect("the manifest is readable"),
            recorded
        );

        let manifest = fs::read_to_string(campaign.join(MANIFEST_FILE)).unwrap();
        assert!(
            !manifest.contains(recorded.as_str()),
            "the manifest must not contain the hash taken over it"
        );

        fs::write(campaign.join(MANIFEST_FILE), format!("{manifest}\n")).unwrap();
        assert_ne!(
            manifest_content_hash(&campaign).expect("the manifest is readable"),
            recorded,
            "an edited manifest must stop matching its recorded hash"
        );
    }

    /// REQ-05: coverage is exactly the raw data set.
    #[test]
    fn integrity_file_covers_exactly_the_raw_data_set() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        fs::create_dir_all(campaign.join("derived")).unwrap();
        fs::write(campaign.join("derived/report.md"), b"# derived\n").unwrap();

        let text = fs::read_to_string(campaign.join(INTEGRITY_FILE)).unwrap();
        let covered: BTreeSet<_> = decode_integrity_file(&text)
            .expect("the generated file parses")
            .into_iter()
            .map(|entry| entry.path.as_str().to_owned())
            .collect();
        let raw: BTreeSet<_> = DatasetLayout::from_manifest(&manifest_at_revision(&repo.head()))
            .required_files()
            .iter()
            .filter(|file| file.class == DatasetFileClass::RawData)
            .map(|file| file.relative_path.clone())
            .collect();

        assert_eq!(covered, raw);
        assert!(!covered.contains(INTEGRITY_FILE));
        assert!(!covered.iter().any(|path| path.starts_with("derived/")));
    }

    /// REQ-05: the format is the coreutils check-file format.
    #[test]
    fn integrity_file_verifies_with_external_sha256sum_tooling() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        let text = fs::read_to_string(campaign.join(INTEGRITY_FILE)).unwrap();

        for line in text.lines() {
            let (digest, path) = line
                .split_once("  ")
                .unwrap_or_else(|| panic!("{line:?} lacks the two-space coreutils separator"));
            assert!(digest.parse::<Sha256Digest>().is_ok(), "{line:?}");
            assert!(path.parse::<ArtifactPath>().is_ok(), "{line:?}");
        }

        // Skipped where coreutils is absent; the structural assertions above
        // still pin the format.
        if let Ok(output) = Command::new("sha256sum")
            .arg("-c")
            .arg(INTEGRITY_FILE)
            .current_dir(&campaign)
            .output()
        {
            assert!(
                output.status.success(),
                "sha256sum -c rejected the integrity file: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    /// REQ-06: an untouched dataset verifies.
    #[test]
    fn verification_accepts_a_dataset_that_still_matches_its_integrity_file() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        repo.commit_all("publish the dataset");

        assert_eq!(
            verify_dataset(&campaign).expect("verification reaches a verdict"),
            DatasetVerdict::Verified
        );
    }

    /// REQ-06: a missing file and a changed file are distinguished.
    #[test]
    fn verification_distinguishes_a_missing_file_from_a_changed_one() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        fs::remove_file(campaign.join(FIRST_SHARD)).unwrap();
        let pooled = fs::read_to_string(campaign.join(POOLED_SUMMARY_FILE)).unwrap();
        fs::write(campaign.join(POOLED_SUMMARY_FILE), format!("{pooled}\n")).unwrap();

        let verdict = verify_dataset(&campaign).expect("verification reaches a verdict");
        let DatasetVerdict::Failed { faults } = &verdict else {
            panic!("a damaged dataset must not verify: {verdict}");
        };
        assert_eq!(faults.len(), 2, "{verdict}");
        assert!(
            faults.iter().any(|fault| matches!(
                fault,
                IntegrityFault::Missing { path } if path.as_str() == FIRST_SHARD
            )),
            "{verdict}"
        );
        assert!(
            faults.iter().any(|fault| matches!(
                fault,
                IntegrityFault::Changed { path, recorded, actual }
                    if path.as_str() == POOLED_SUMMARY_FILE && recorded != actual
            )),
            "{verdict}"
        );
    }

    /// REQ-05, REQ-06: coverage drift in either direction is reported.
    #[test]
    fn verification_reports_uncovered_and_non_raw_entries() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        fs::create_dir_all(campaign.join("derived")).unwrap();
        fs::write(campaign.join("derived/report.md"), b"# derived\n").unwrap();

        let text = fs::read_to_string(campaign.join(INTEGRITY_FILE)).unwrap();
        let mut entries: Vec<_> = decode_integrity_file(&text)
            .expect("the generated file parses")
            .into_iter()
            .filter(|entry| entry.path.as_str() != "summaries/q3.json")
            .collect();
        entries.push(IntegrityEntry {
            path: "derived/report.md".parse().unwrap(),
            sha256: file_digest(&campaign.join("derived/report.md")).unwrap(),
        });
        entries.push(IntegrityEntry {
            path: INTEGRITY_FILE.parse().unwrap(),
            sha256: file_digest(&campaign.join(MANIFEST_FILE)).unwrap(),
        });
        fs::write(
            campaign.join(INTEGRITY_FILE),
            encode_integrity_file(&entries),
        )
        .unwrap();

        let verdict = verify_dataset(&campaign).expect("verification reaches a verdict");
        let DatasetVerdict::Failed { faults } = &verdict else {
            panic!("drifted coverage must not verify: {verdict}");
        };
        assert!(
            faults.contains(&IntegrityFault::Uncovered {
                path: "summaries/q3.json".parse().unwrap()
            }),
            "{verdict}"
        );
        assert!(
            faults.contains(&IntegrityFault::OutsideRawSet {
                path: "derived/report.md".parse().unwrap()
            }),
            "{verdict}"
        );
        assert!(
            faults.contains(&IntegrityFault::OutsideRawSet {
                path: INTEGRITY_FILE.parse().unwrap()
            }),
            "an integrity file that covers itself cannot close: {verdict}"
        );
    }

    /// REQ-05, REQ-06: a completed cell's shard cannot leave coverage quietly.
    #[test]
    fn a_lost_shard_of_a_completed_cell_refuses_generation_and_fails_verification() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        fs::remove_file(campaign.join(SECOND_SHARD)).unwrap();

        let error = generate_integrity_file(&campaign, &manifest_at_revision(&repo.head()))
            .expect_err("a completed cell's shard must exist to be covered");
        assert!(
            matches!(&error, IntegrityError::MissingRawFile { path } if path.ends_with(SECOND_SHARD)),
            "{error}"
        );

        // A sidecar that simply omits the lost shard must not verify clean
        // either, which is what a regenerated file would have looked like.
        let text = fs::read_to_string(campaign.join(INTEGRITY_FILE)).unwrap();
        let entries: Vec<_> = decode_integrity_file(&text)
            .expect("the generated file parses")
            .into_iter()
            .filter(|entry| entry.path.as_str() != SECOND_SHARD)
            .collect();
        fs::write(
            campaign.join(INTEGRITY_FILE),
            encode_integrity_file(&entries),
        )
        .unwrap();

        let verdict = verify_dataset(&campaign).expect("verification reaches a verdict");
        let DatasetVerdict::Failed { faults } = &verdict else {
            panic!("a dataset missing published data must not verify: {verdict}");
        };
        assert_eq!(
            faults,
            &[IntegrityFault::Missing {
                path: SECOND_SHARD.parse().unwrap()
            }],
            "{verdict}"
        );
    }

    /// REQ-05, REQ-06: a halted cell's unexecuted shards stay legitimately absent.
    #[test]
    fn a_halted_cell_omits_its_unexecuted_shards_and_still_verifies() {
        let repo = TestRepo::new();
        let campaign = repo.campaign_root();
        fs::create_dir_all(&campaign).unwrap();
        write_halted_fixture(&campaign, &repo.head());
        assert!(!campaign.join(SECOND_SHARD).exists());

        let text = generate_integrity_file(&campaign, &manifest_at_revision(&repo.head()))
            .expect("a halted cell's unexecuted shard is legitimately absent");
        assert!(text.contains(FIRST_SHARD), "{text}");
        assert!(!text.contains(SECOND_SHARD), "{text}");

        assert_eq!(
            verify_dataset(&campaign).expect("verification reaches a verdict"),
            DatasetVerdict::Verified
        );
    }

    /// REQ-05: an undecidable halt state exempts nothing.
    #[test]
    fn generation_refuses_when_a_field_summary_cannot_decide_the_halt_state() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        fs::remove_file(campaign.join(SECOND_SHARD)).unwrap();
        fs::write(campaign.join("summaries/q3.json"), b"{}\n").unwrap();

        let error = generate_integrity_file(&campaign, &manifest_at_revision(&repo.head()))
            .expect_err("an unreadable summary cannot excuse an absent shard");
        assert!(matches!(error, IntegrityError::Schema(_)), "{error}");
    }

    /// REQ-07: a revision absent from the repository does not pass silently.
    #[test]
    fn verification_reports_an_absent_recorded_revision_as_unverifiable() {
        let repo = TestRepo::new();
        let campaign = repo.campaign_root();
        fs::create_dir_all(&campaign).unwrap();
        let absent: GitRevision = "0000000000000000000000000000000000000001".parse().unwrap();
        write_fixture_at_revision(&campaign, &absent);
        repo.commit_all("publish a dataset naming an absent revision");

        let verdict = verify_dataset(&campaign).expect("verification reaches a verdict");
        match verdict {
            DatasetVerdict::Unverifiable {
                reason: UnverifiableReason::UnknownRevision { ref revision },
                ref faults,
            } => {
                assert_eq!(revision, &absent);
                assert!(faults.is_empty(), "the bytes still match: {verdict}");
            }
            other => panic!("an absent recorded revision must not verify: {other}"),
        }
    }

    /// REQ-07: outside a repository the recorded revision cannot be resolved.
    #[test]
    fn verification_outside_a_repository_reports_unresolvable_provenance() {
        let fixture = TestDir::new();
        let absent: GitRevision = "0000000000000000000000000000000000000002".parse().unwrap();
        write_fixture_at_revision(fixture.root(), &absent);

        let verdict = verify_dataset(fixture.root()).expect("verification reaches a verdict");
        assert!(
            matches!(verdict, DatasetVerdict::Unverifiable { .. }),
            "a dataset whose revision cannot be resolved must not verify: {verdict}"
        );
    }
}
