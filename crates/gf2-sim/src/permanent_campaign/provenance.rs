//! Source identity and integrity for published campaign datasets.
//!
//! Two independent guarantees live here. [`approve_emission`] decides whether
//! the running binary may publish into a campaign directory at all: it compares
//! the revision embedded into the build against the repository's current
//! `HEAD`, then refuses when any tracked file differs outside the campaign's own
//! output subtree. [`verify_dataset`] decides, later and from the published
//! bytes alone, whether the dataset still matches its
//! [`INTEGRITY_FILE`](super::schema::INTEGRITY_FILE) and whether the revision it
//! names still exists.
//!
//! The emission rule is deliberately narrower than "the tree is clean". A
//! published dataset lives inside this repository, so a clean-tree rule would
//! refuse the second shard of every campaign: the first shard already dirtied
//! the tree. What emission protects is the identity of the source behind the
//! numbers, not the absence of output. The frozen root manifest is the one
//! exception inside that subtree, because it fixes what the numbers claim
//! rather than recording them.
//!
//! The on-disk integrity format and its `sha256sum -c` verification procedure
//! are documented in
//! `dev/simulation_results/permanent-zero-fraction/README.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use super::schema::{
    field_summary_file, read_field_summary, read_manifest, shard_record_file, ArtifactPath,
    ArtifactPathError, CampaignManifest, CellTerminalState, DatasetFileClass, DatasetLayout,
    GitRevision, GitRevisionError, SchemaError, Sha256Digest, Sha256DigestError, INTEGRITY_FILE,
    MANIFEST_FILE,
};

/// Source revision recorded into this build of `gf2-sim` by its build script.
const BUILD_GIT_REVISION: &str = env!("GF2_SIM_BUILD_GIT_REVISION");

/// Source revision a binary carries from the build that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildRevision {
    /// The build recorded the revision its source came from.
    Recorded(GitRevision),
    /// The build could not determine a revision, so emission has nothing to
    /// check its numbers against and refuses.
    Unavailable,
}

impl fmt::Display for BuildRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recorded(revision) => revision.fmt(f),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}

/// Returns the source revision embedded into this build.
///
/// The value is fixed when `gf2-sim` is compiled, by the crate's build script.
/// A build made outside a git checkout, or without `git` on `PATH`, records no
/// revision and yields [`BuildRevision::Unavailable`]; emission then refuses
/// rather than publishing numbers whose source cannot be named.
///
/// The build script does not follow `HEAD` unless `GF2_SIM_TRACK_HEAD` is set
/// to a value other than `0`, so after a later commit this value is stale and
/// [`approve_emission`] refuses with a revision mismatch. That refusal is the
/// fail-closed direction — a stale binary cannot publish under a source it was
/// not built from — and a publisher's build sets the variable. The build
/// script documents the trade-off it protects.
pub fn build_revision() -> BuildRevision {
    match BUILD_GIT_REVISION.parse() {
        Ok(revision) => BuildRevision::Recorded(revision),
        Err(_) => BuildRevision::Unavailable,
    }
}

/// Permission for one binary to publish into one campaign directory.
///
/// The token is produced only by [`approve_emission`] and its explicit-revision
/// form, and it carries the revision both the build and the repository agreed
/// on, so a writer records exactly the revision that was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionApproval {
    revision: GitRevision,
}

impl EmissionApproval {
    /// Returns the source revision the emitting build was checked against.
    pub fn revision(&self) -> &GitRevision {
        &self.revision
    }
}

/// How a tracked file differs from the revision the binary was built at.
///
/// Rename and copy detection is disabled when the working tree is inspected, so
/// a rename appears as a deletion and an addition rather than a single entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceChangeKind {
    /// The path is new in the index.
    Added,
    /// The path's content differs.
    Modified,
    /// The path is gone.
    Deleted,
    /// The path changed between file, symlink, or submodule.
    TypeChanged,
    /// The path has an unresolved merge conflict.
    Unmerged,
}

impl fmt::Display for SourceChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::TypeChanged => "type-changed",
            Self::Unmerged => "unmerged",
        })
    }
}

/// One tracked file that differs from the built revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChange {
    /// Path relative to the repository root.
    pub path: PathBuf,
    /// How the path differs.
    pub kind: SourceChangeKind,
}

impl fmt::Display for SourceChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.path.display(), self.kind)
    }
}

/// Why a binary may not publish into a campaign directory.
///
/// Every variant refuses: an inconclusive check is a refusal, because a dataset
/// whose source cannot be established is exactly what the guard exists to
/// prevent.
#[derive(Debug)]
pub enum EmissionRefusal {
    /// The build recorded no source revision.
    UnknownBuildRevision,
    /// The build's revision is not the repository's current `HEAD`.
    RevisionMismatch {
        /// Revision the binary was built from.
        built: GitRevision,
        /// Revision the repository currently has checked out.
        head: GitRevision,
    },
    /// Tracked files differ outside the campaign's own output subtree.
    SourceChanged {
        /// Every differing path, ordered by path.
        changes: Vec<SourceChange>,
    },
    /// The campaign directory is not inside the repository being checked.
    OutsideRepository {
        /// Campaign directory that was requested.
        path: PathBuf,
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
}

impl fmt::Display for EmissionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBuildRevision => f.write_str(
                "the emitting build recorded no source revision, so the dataset it would write \
                 cannot be traced to a source",
            ),
            Self::RevisionMismatch { built, head } => write!(
                f,
                "the emitting build is at revision {built} but the repository is at {head}"
            ),
            Self::SourceChanged { changes } => {
                f.write_str("tracked files differ outside the campaign output subtree:")?;
                for change in changes {
                    write!(f, " {change}")?;
                }
                Ok(())
            }
            Self::OutsideRepository { path } => write!(
                f,
                "campaign directory {} is outside the repository being checked",
                path.display()
            ),
            Self::Git { command, message } => {
                write!(f, "`git {command}` failed: {message}")
            }
            Self::Io { path, source } => write!(f, "cannot resolve {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for EmissionRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Decides whether this build may publish into `campaign_root`.
///
/// Emission is approved when the revision embedded by the build script equals
/// the repository's `HEAD` and no tracked file differs outside
/// `campaign_root`. The campaign's own raw and derived output is expected to
/// dirty the tree and is therefore exempt, with one exception: the frozen
/// [`MANIFEST_FILE`] inside `campaign_root` refuses when it differs, because it
/// declares the identity the numbers are published under. Untracked paths never
/// refuse; the compiled source is fixed by `HEAD`, and a file git does not track
/// is not part of it.
///
/// `campaign_root` may name a directory that does not exist yet, provided its
/// parent does; the first emission of a campaign creates it.
///
/// # Errors
///
/// Returns the [`EmissionRefusal`] that blocked publication. An unusable
/// repository, an unreadable path, or a failing `git` invocation refuses too:
/// the guard never approves a check it could not complete.
pub fn approve_emission(campaign_root: &Path) -> Result<EmissionApproval, EmissionRefusal> {
    approve_emission_from(&build_revision(), campaign_root)
}

/// Decides whether a binary built at `built` may publish into `campaign_root`.
///
/// [`approve_emission`] is this function applied to the revision embedded in
/// this build. Passing the revision explicitly lets a test drive the rule
/// against a throwaway repository, and lets a separately built emitter supply
/// the revision its own build recorded.
///
/// # Errors
///
/// As [`approve_emission`].
pub fn approve_emission_from(
    built: &BuildRevision,
    campaign_root: &Path,
) -> Result<EmissionApproval, EmissionRefusal> {
    let BuildRevision::Recorded(built) = built else {
        return Err(EmissionRefusal::UnknownBuildRevision);
    };

    let anchor = if campaign_root.is_dir() {
        campaign_root.to_owned()
    } else {
        campaign_root
            .parent()
            .ok_or_else(|| EmissionRefusal::OutsideRepository {
                path: campaign_root.to_owned(),
            })?
            .to_owned()
    };
    let repository = canonicalize(Path::new(&run_git(
        &anchor,
        &["rev-parse", "--show-toplevel"],
    )?))?;

    let head: GitRevision = run_git(&repository, &["rev-parse", "HEAD"])?
        .parse()
        .map_err(|error: GitRevisionError| EmissionRefusal::Git {
            command: "rev-parse HEAD".to_owned(),
            message: error.to_string(),
        })?;
    if &head != built {
        return Err(EmissionRefusal::RevisionMismatch {
            built: built.clone(),
            head,
        });
    }

    let exempt_prefix = campaign_prefix(&anchor, campaign_root, &repository)?;
    let arguments = ["status", "--porcelain=v1", "-z", "--no-renames"];
    let status = run_git(&repository, &arguments)?;
    let mut changes = Vec::new();
    for record in status.split('\0').filter(|record| !record.is_empty()) {
        let Some((codes, path)) = split_status_record(record) else {
            return Err(EmissionRefusal::Git {
                command: arguments.join(" "),
                message: format!("unparsable status record {record:?}"),
            });
        };
        // Untracked and ignored paths never refuse: the source the binary was
        // compiled from is fixed by HEAD, and a file git does not track is not
        // part of it.
        if codes == "??" || codes == "!!" {
            continue;
        }
        let exempt = path
            .strip_prefix(exempt_prefix.as_str())
            .is_some_and(|inside| inside != MANIFEST_FILE);
        if exempt {
            continue;
        }
        changes.push(SourceChange {
            path: PathBuf::from(path),
            kind: change_kind(codes).ok_or_else(|| EmissionRefusal::Git {
                command: arguments.join(" "),
                message: format!("unrecognized status code {codes:?} for {path}"),
            })?,
        });
    }
    if changes.is_empty() {
        Ok(EmissionApproval { revision: head })
    } else {
        changes.sort_by(|left, right| left.path.cmp(&right.path));
        Err(EmissionRefusal::SourceChanged { changes })
    }
}

/// Returns the campaign subtree as a repository-relative `/`-terminated prefix.
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
    let mut prefix = String::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(outside());
        };
        prefix.push_str(name.to_str().ok_or_else(outside)?);
        prefix.push('/');
    }
    Ok(prefix)
}

/// Splits an `XY <path>` porcelain record into its status codes and path.
fn split_status_record(record: &str) -> Option<(&str, &str)> {
    let bytes = record.as_bytes();
    (bytes.len() > 3 && bytes[2] == b' ').then(|| (&record[..2], &record[3..]))
}

/// Maps a porcelain status pair to how the path differs.
fn change_kind(codes: &str) -> Option<SourceChangeKind> {
    let bytes = codes.as_bytes();
    if bytes.contains(&b'U') || codes == "AA" || codes == "DD" {
        return Some(SourceChangeKind::Unmerged);
    }
    let code = if bytes[0] == b' ' { bytes[1] } else { bytes[0] };
    match code {
        b'A' => Some(SourceChangeKind::Added),
        b'M' => Some(SourceChangeKind::Modified),
        b'D' => Some(SourceChangeKind::Deleted),
        b'T' => Some(SourceChangeKind::TypeChanged),
        _ => None,
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, EmissionRefusal> {
    fs::canonicalize(path).map_err(|source| EmissionRefusal::Io {
        path: path.to_owned(),
        source,
    })
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
        for row in read_field_summary(root, q)?.rows {
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
        TestDir, FIXTURE_CAMPAIGN_ID,
    };
    use super::super::schema::POOLED_SUMMARY_FILE;
    use super::*;

    const CAMPAIGN_AREA: &str = "dev/simulation_results/permanent-zero-fraction";
    const SOURCE_FILE: &str = "crates/gf2-sim/src/permanent_campaign/schema.rs";
    const DEPENDENCY_MANIFEST: &str = "crates/gf2-sim/Cargo.toml";
    const PROTOCOL_DOCUMENT: &str = "dev/simulation_results/permanent-zero-fraction/protocol.md";
    const FIRST_SHARD: &str = "shards/q3/n04/shard-000000.json";
    const SECOND_SHARD: &str = "shards/q3/n04/shard-000001.json";

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
            repo.write(PROTOCOL_DOCUMENT, "# frozen preregistration\n");
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
            self.path(&format!("{CAMPAIGN_AREA}/{FIXTURE_CAMPAIGN_ID}"))
        }

        /// Writes the conforming fixture dataset under the campaign area.
        fn write_dataset(&self) -> PathBuf {
            let root = self.campaign_root();
            fs::create_dir_all(&root).unwrap();
            write_fixture_at_revision(&root, &self.head());
            root
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn approve(repo: &TestRepo, campaign: &Path) -> Result<EmissionApproval, EmissionRefusal> {
        approve_emission_from(&BuildRevision::Recorded(repo.head()), campaign)
    }

    /// REQ-01: the embedded revision exists and is what the build recorded.
    ///
    /// This asserts that the build recorded a revision, not that the recorded
    /// revision is current. The build script follows `HEAD` only under
    /// `GF2_SIM_TRACK_HEAD`, so an ordinary build carries the revision it was
    /// compiled at and goes stale; equality with `HEAD` is the guard's job, and
    /// `emission_requires_the_built_revision_to_equal_head` drives both sides
    /// of it against a throwaway repository.
    #[test]
    fn build_revision_is_recorded_when_the_crate_is_built_from_a_checkout() {
        let built_from_checkout = Command::new("git")
            .args(["-C", env!("CARGO_MANIFEST_DIR"), "rev-parse", "--verify"])
            .arg("HEAD")
            .output()
            .is_ok_and(|output| output.status.success());
        match build_revision() {
            BuildRevision::Recorded(revision) => assert_eq!(revision.as_str().len(), 40),
            BuildRevision::Unavailable => assert!(
                !built_from_checkout,
                "a build inside a git checkout must record its source revision"
            ),
        }
    }

    /// REQ-01: emission requires an embedded revision equal to `HEAD`.
    #[test]
    fn emission_requires_the_built_revision_to_equal_head() {
        let repo = TestRepo::new();
        let campaign = repo.campaign_root();

        let refusal = approve_emission_from(&BuildRevision::Unavailable, &campaign)
            .expect_err("a build with no recorded revision must refuse");
        assert!(
            matches!(refusal, EmissionRefusal::UnknownBuildRevision),
            "{refusal}"
        );

        let stale = repo.head();
        repo.write(SOURCE_FILE, "pub fn permanent() -> u8 { 0 }\n");
        repo.commit_all("advance the source revision");
        let refusal = approve_emission_from(&BuildRevision::Recorded(stale.clone()), &campaign)
            .expect_err("a build behind HEAD must refuse");
        match refusal {
            EmissionRefusal::RevisionMismatch { built, head } => {
                assert_eq!(built, stale);
                assert_eq!(head, repo.head());
            }
            other => panic!("a stale build revision must be refused: {other}"),
        }

        let approval = approve(&repo, &campaign).expect("a build at HEAD emits");
        assert_eq!(approval.revision(), &repo.head());
    }

    /// REQ-02, REQ-03: the campaign's own output never refuses its own writer.
    #[test]
    fn emission_admits_a_tree_dirtied_only_by_campaign_outputs() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        repo.commit_all("publish the first dataset files");

        fs::write(campaign.join(SECOND_SHARD), b"{}\n").unwrap();
        fs::create_dir_all(campaign.join("derived")).unwrap();
        fs::write(campaign.join("derived/report.md"), b"# derived\n").unwrap();
        fs::write(campaign.join("shards/q3/n04/shard-000002.json"), b"{}\n").unwrap();

        approve(&repo, &campaign).expect("a tree dirtied only by campaign output emits");
    }

    /// REQ-02, REQ-03: changed source, build metadata, or protocol refuses.
    #[test]
    fn emission_refuses_a_changed_source_dependency_or_protocol_file() {
        for changed in [SOURCE_FILE, DEPENDENCY_MANIFEST, PROTOCOL_DOCUMENT] {
            let repo = TestRepo::new();
            let campaign = repo.write_dataset();
            repo.commit_all("publish the dataset");

            fs::write(campaign.join(FIRST_SHARD), b"{}\n").unwrap();
            repo.write(changed, "changed after the build\n");

            let refusal = approve(&repo, &campaign)
                .expect_err("a changed tracked file outside the campaign must refuse");
            let EmissionRefusal::SourceChanged { changes } = &refusal else {
                panic!("{changed} must refuse as a source change: {refusal}");
            };
            assert_eq!(
                changes,
                &[SourceChange {
                    path: PathBuf::from(changed),
                    kind: SourceChangeKind::Modified,
                }],
                "only {changed} differs outside the campaign subtree"
            );
            assert!(
                refusal.to_string().contains(changed),
                "the refusal must name what changed: {refusal}"
            );
        }
    }

    /// REQ-02: the frozen root manifest is not covered by the subtree exemption.
    #[test]
    fn emission_refuses_a_changed_frozen_root_manifest() {
        let repo = TestRepo::new();
        let campaign = repo.write_dataset();
        repo.commit_all("publish the dataset");
        fs::write(campaign.join(MANIFEST_FILE), b"{}\n").unwrap();

        let refusal = approve(&repo, &campaign).expect_err("a changed frozen manifest must refuse");
        let EmissionRefusal::SourceChanged { changes } = &refusal else {
            panic!("a changed manifest must refuse as a source change: {refusal}");
        };
        assert_eq!(
            changes,
            &[SourceChange {
                path: PathBuf::from(format!(
                    "{CAMPAIGN_AREA}/{FIXTURE_CAMPAIGN_ID}/{MANIFEST_FILE}"
                )),
                kind: SourceChangeKind::Modified,
            }]
        );
    }

    /// REQ-02: a campaign directory outside the repository cannot be approved.
    #[test]
    fn emission_refuses_a_campaign_directory_outside_the_repository() {
        let repo = TestRepo::new();
        let outside = TestDir::new();

        let refusal = approve_emission_from(&BuildRevision::Recorded(repo.head()), outside.root())
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
