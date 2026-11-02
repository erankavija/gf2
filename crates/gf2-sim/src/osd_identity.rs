//! OSD receipt identity; named convention exception under `@/issue/779d1926`.
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt, fs, io,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

/// A normalized repository-relative path to a committed artifact.
///
/// Paths use forward slashes, contain no empty, `.` or `..` component, and
/// cannot name an absolute or drive-qualified path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ArtifactPath(String);

impl ArtifactPath {
    /// Returns the validated repository-relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ArtifactPath {
    type Err = ArtifactPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let first_component = value.split('/').next().unwrap_or_default();
        let drive_qualified = first_component.as_bytes().get(1) == Some(&b':')
            && first_component
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic);
        let valid = !value.is_empty()
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains('\\')
            && !drive_qualified
            && !value.chars().any(char::is_control)
            && value
                .split('/')
                .all(|component| !component.is_empty() && component != "." && component != "..");
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(ArtifactPathError(value.to_owned()))
        }
    }
}

impl fmt::Display for ArtifactPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for ArtifactPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned for a path outside the canonical artifact-path grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPathError(String);

impl fmt::Display for ArtifactPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "artifact path {:?} must be a normalized repository-relative path",
            self.0
        )
    }
}

impl std::error::Error for ArtifactPathError {}

/// A canonical lowercase hexadecimal SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Returns the 64-character lowercase hexadecimal digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Sha256Digest {
    type Err = Sha256DigestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if is_lowercase_hex(value, 64) {
            Ok(Self(value.to_owned()))
        } else {
            Err(Sha256DigestError(value.to_owned()))
        }
    }
}

fn is_lowercase_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned for a non-canonical SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha256DigestError(String);

impl fmt::Display for Sha256DigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SHA-256 digest {:?} must contain exactly 64 lowercase hexadecimal characters",
            self.0
        )
    }
}

impl std::error::Error for Sha256DigestError {}

/// A canonical full-length git revision.
///
/// The recorded value is the complete 40-character lowercase hexadecimal
/// object name. An abbreviation is rejected because it resolves only against
/// the repository that produced it, while a published dataset is read
/// elsewhere and must still name exactly one commit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct GitRevision(String);

impl GitRevision {
    /// Returns the 40-character lowercase hexadecimal object name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for GitRevision {
    type Err = GitRevisionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if is_lowercase_hex(value, 40) {
            Ok(Self(value.to_owned()))
        } else {
            Err(GitRevisionError(value.to_owned()))
        }
    }
}

impl fmt::Display for GitRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for GitRevision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned for a revision that is not a full object name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRevisionError(String);

impl fmt::Display for GitRevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "git revision {:?} must contain exactly 40 lowercase hexadecimal characters",
            self.0
        )
    }
}

impl std::error::Error for GitRevisionError {}

/// Content identity for a committed mechanical artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    /// Normalized path from the repository root.
    pub path: ArtifactPath,
    /// SHA-256 of the committed artifact bytes.
    pub sha256: Sha256Digest,
}

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

/// Deterministic random-number generator algorithm used by the campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RngAlgorithm {
    /// ChaCha20 with the implementation version recorded separately.
    ChaCha20,
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
    /// Revision of the linked source closure (`crates/` and `Cargo.lock`),
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
    /// Exact producer invocation as unquoted argument tokens.
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
/// Observes the processor model and topology.
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

fn run_git(directory: &Path, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim_end().to_owned())
        .map_err(io::Error::other)
}
/// Resolves a repository path to its top level.
pub fn repository_top_level(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(run_git(path, &["rev-parse", "--show-toplevel"])?)
}
/// Observes OSD rebuild inputs while preserving its receipt representation.
pub fn observe_provenance(repository: &Path, mut provenance: Provenance) -> io::Result<Provenance> {
    let root = repository_top_level(repository)?;
    provenance.git_revision = run_git(&root, &["rev-parse", "HEAD"])?
        .parse()
        .map_err(io::Error::other)?;
    provenance.binary_sha256 = Some(
        format!("{:x}", Sha256::digest(fs::read("/proc/self/exe")?))
            .parse()
            .map_err(io::Error::other)?,
    );
    provenance.deps_source_revision = Some(
        run_git(
            &root,
            &["log", "-1", "--format=%H", "--", "crates/", "Cargo.lock"],
        )?
        .parse()
        .map_err(io::Error::other)?,
    );
    provenance.deps_source_dirty = Some(
        !run_git(
            &root,
            &[
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--",
                "crates/",
                "Cargo.lock",
            ],
        )?
        .is_empty(),
    );
    Ok(provenance)
}
