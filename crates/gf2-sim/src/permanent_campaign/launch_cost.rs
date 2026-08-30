//! Canonical parsing and manifest validation for accelerator launch costs.
//!
//! The production CSV is an execution input, not a campaign-manifest field.
//! This module keeps its stable boundary in one place so callers cannot accept
//! different row sets or silently size one cell from another cell's timing.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::schedule::{AcceleratorCostTable, ScheduleError};
use super::schema::{ArtifactIdentity, CampaignManifest};

/// Stable header of an accelerator launch-cost CSV.
pub const ACCELERATOR_COST_CSV_HEADER: &str = "q,n,per_matrix_us";

/// One repository-bound immutable accelerator-cost input.
#[derive(Clone, Debug)]
pub struct ResolvedAcceleratorCostTable {
    /// Parsed table from the exact byte snapshot.
    pub table: AcceleratorCostTable,
    /// Canonical repository-relative path and content digest.
    pub identity: ArtifactIdentity,
}

/// A launch-cost CSV is malformed or disagrees with its campaign manifest.
#[derive(Debug)]
pub enum AcceleratorCostTableError {
    /// The CSV could not be read.
    Read {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The CSV contains no header.
    Empty,
    /// The CSV is not UTF-8 text.
    Encoding,
    /// The stable header differs.
    Header {
        /// Header observed in the CSV.
        found: String,
    },
    /// A blank data row was observed.
    BlankRow {
        /// One-based line number.
        line: usize,
    },
    /// A row does not have exactly three fields.
    FieldCount {
        /// One-based line number.
        line: usize,
        /// Number of fields observed.
        found: usize,
    },
    /// A numeric field is not a canonical unsigned integer.
    Integer {
        /// One-based line number.
        line: usize,
        /// Stable field name.
        field: &'static str,
        /// Value observed in the CSV.
        value: String,
    },
    /// A per-matrix duration is zero.
    Zero {
        /// One-based line number.
        line: usize,
    },
    /// A table key occurs more than once.
    Duplicate {
        /// Field order.
        q: u8,
        /// Matrix dimension.
        n: u16,
        /// One-based line number of the repeated key.
        line: usize,
    },
    /// A row names a processor-backed manifest cell.
    ProcessorBacked {
        /// Field order.
        q: u8,
        /// Matrix dimension.
        n: u16,
    },
    /// A row is not a cell in the manifest.
    NotManifestCell {
        /// Field order.
        q: u8,
        /// Matrix dimension.
        n: u16,
    },
    /// An accelerator-backed manifest cell has no row.
    Missing {
        /// Field order.
        q: u8,
        /// Matrix dimension.
        n: u16,
    },
}

impl fmt::Display for AcceleratorCostTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::Empty => formatter.write_str("file is empty"),
            Self::Encoding => formatter.write_str("file is not UTF-8"),
            Self::Header { found } => write!(
                formatter,
                "header is {found:?}, expected {ACCELERATOR_COST_CSV_HEADER:?}"
            ),
            Self::BlankRow { line } => write!(formatter, "line {line}: blank row"),
            Self::FieldCount { line, found } => {
                write!(formatter, "line {line}: expected 3 fields, found {found}")
            }
            Self::Integer { line, field, value } => {
                write!(
                    formatter,
                    "line {line}: {field} {value:?} is not an integer"
                )
            }
            Self::Zero { line } => {
                write!(formatter, "line {line}: per_matrix_us must be positive")
            }
            Self::Duplicate { q, n, line } => {
                write!(formatter, "line {line}: duplicate entry for q={q} n={n}")
            }
            Self::ProcessorBacked { q, n } => {
                write!(formatter, "q={q} n={n} is a processor-backed manifest cell")
            }
            Self::NotManifestCell { q, n } => {
                write!(formatter, "q={q} n={n} is not a manifest cell")
            }
            Self::Missing { q, n } => {
                write!(formatter, "missing accelerator-backed cell q={q} n={n}")
            }
        }
    }
}

impl std::error::Error for AcceleratorCostTableError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn parse_integer<T>(
    value: &str,
    line: usize,
    field: &'static str,
) -> Result<T, AcceleratorCostTableError>
where
    T: std::str::FromStr,
{
    if value.is_empty()
        || value != value.trim()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AcceleratorCostTableError::Integer {
            line,
            field,
            value: value.to_owned(),
        });
    }
    value
        .parse()
        .map_err(|_| AcceleratorCostTableError::Integer {
            line,
            field,
            value: value.to_owned(),
        })
}

/// Resolves, reads once, identifies, and parses a repository-owned cost table.
///
/// Relative paths resolve against `repository_root`, independent of the
/// process working directory. Absolute paths must resolve inside that root.
/// The returned identity and table derive from the same byte snapshot.
///
/// # Errors
///
/// Returns [`AcceleratorCostTableError::Read`] for path and read failures and
/// the parser's semantic errors for invalid content.
pub fn resolve_accelerator_cost_table(
    repository_root: &Path,
    supplied_path: &Path,
    manifest: &CampaignManifest,
    launch_cap: Duration,
) -> Result<ResolvedAcceleratorCostTable, AcceleratorCostTableError> {
    let repository_root =
        fs::canonicalize(repository_root).map_err(|source| AcceleratorCostTableError::Read {
            path: repository_root.to_owned(),
            source,
        })?;
    let candidate = if supplied_path.is_absolute() {
        supplied_path.to_owned()
    } else {
        repository_root.join(supplied_path)
    };
    let canonical =
        fs::canonicalize(&candidate).map_err(|source| AcceleratorCostTableError::Read {
            path: candidate.clone(),
            source,
        })?;
    let relative =
        canonical
            .strip_prefix(&repository_root)
            .map_err(|_| AcceleratorCostTableError::Read {
                path: canonical.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "accelerator cost table resolves outside the repository",
                ),
            })?;
    let bytes = fs::read(&canonical).map_err(|source| AcceleratorCostTableError::Read {
        path: canonical.clone(),
        source,
    })?;
    let table = parse_accelerator_cost_table(&bytes, manifest, launch_cap)?;
    let path = relative
        .to_string_lossy()
        .replace('\\', "/")
        .parse()
        .map_err(|error| AcceleratorCostTableError::Read {
            path: relative.to_owned(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("accelerator cost table path is not canonical: {error}"),
            ),
        })?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes))
        .parse()
        .expect("SHA-256 formatting is canonical");
    Ok(ResolvedAcceleratorCostTable {
        table,
        identity: ArtifactIdentity { path, sha256 },
    })
}

/// Parses one immutable launch-cost CSV byte snapshot.
///
/// The byte-oriented entry point lets a coordinator bind the content digest
/// and execution configuration to the same single read under its execution
/// lock.
///
/// # Errors
///
/// Returns [`AcceleratorCostTableError`] for non-UTF-8, malformed, or
/// manifest-incomplete content.
///
/// # Panics
///
/// Does not panic.
///
/// # Complexity
///
/// `O(C log C + R log R)` time and `O(C + R)` space.
pub fn parse_accelerator_cost_table(
    bytes: &[u8],
    manifest: &CampaignManifest,
    launch_cap: Duration,
) -> Result<AcceleratorCostTable, AcceleratorCostTableError> {
    let text = std::str::from_utf8(bytes).map_err(|_| AcceleratorCostTableError::Encoding)?;
    let mut lines = text.lines().enumerate();
    let (_, header) = lines.next().ok_or(AcceleratorCostTableError::Empty)?;
    if header != ACCELERATOR_COST_CSV_HEADER {
        return Err(AcceleratorCostTableError::Header {
            found: header.to_owned(),
        });
    }

    let mut costs = BTreeMap::new();
    for (index, row) in lines {
        let line = index + 1;
        if row.is_empty() {
            return Err(AcceleratorCostTableError::BlankRow { line });
        }
        let fields: Vec<&str> = row.split(',').collect();
        let [q, n, per_matrix_us] = fields.as_slice() else {
            return Err(AcceleratorCostTableError::FieldCount {
                line,
                found: fields.len(),
            });
        };
        let q = parse_integer::<u8>(q, line, "q")?;
        let n = parse_integer::<u16>(n, line, "n")?;
        let microseconds = parse_integer::<u64>(per_matrix_us, line, "per_matrix_us")?;
        if microseconds == 0 {
            return Err(AcceleratorCostTableError::Zero { line });
        }
        if costs
            .insert((q, n), Duration::from_micros(microseconds))
            .is_some()
        {
            return Err(AcceleratorCostTableError::Duplicate { q, n, line });
        }
    }
    let table = AcceleratorCostTable::new(costs, launch_cap);
    table
        .validate_manifest(manifest)
        .map_err(|error| match error {
            ScheduleError::AcceleratorCostMissing { q, n } => {
                AcceleratorCostTableError::Missing { q, n }
            }
            ScheduleError::AcceleratorCostUnexpected {
                q,
                n,
                backend: Some(_),
            } => AcceleratorCostTableError::ProcessorBacked { q, n },
            ScheduleError::AcceleratorCostUnexpected {
                q,
                n,
                backend: None,
            } => AcceleratorCostTableError::NotManifestCell { q, n },
            _ => unreachable!("manifest cost validation returns only key-set errors"),
        })?;
    Ok(table)
}
