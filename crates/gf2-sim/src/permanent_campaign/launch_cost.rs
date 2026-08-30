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

use super::schedule::{AcceleratorCostTable, ScheduleError};
use super::schema::CampaignManifest;

/// Stable header of an accelerator launch-cost CSV.
pub const ACCELERATOR_COST_CSV_HEADER: &str = "q,n,per_matrix_us";

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

/// Reads a launch-cost CSV and proves exact agreement with the manifest.
///
/// The CSV must contain exactly one positive integer cost for every and only
/// every accelerator-backed manifest cell. Duplicate keys, processor-backed
/// keys, cells outside the manifest, and missing accelerator keys are refused.
/// The returned table uses `launch_cap` for every cell.
///
/// # Errors
///
/// Returns [`AcceleratorCostTableError`] when the file is unreadable,
/// malformed, non-canonical, or its keys differ from the manifest's exact
/// accelerator-backed key set.
///
/// # Panics
///
/// Does not panic.
///
/// # Complexity
///
/// `O(C log C + R log R)` time and `O(C + R)` space for `C` manifest cells
/// and `R` CSV rows.
pub fn read_accelerator_cost_table(
    path: &Path,
    manifest: &CampaignManifest,
    launch_cap: Duration,
) -> Result<AcceleratorCostTable, AcceleratorCostTableError> {
    let text = fs::read_to_string(path).map_err(|source| AcceleratorCostTableError::Read {
        path: path.to_owned(),
        source,
    })?;
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
