//! Committed-and-clean guard over a campaign's producing-input closure.
//!
//! A timed run measures the tree it pins, so every file whose bytes can reach
//! a sample is tracked by git and identical to its committed content before
//! the runner launches. The closure is the `build_inputs` section of the
//! producing-input manifest the plan names: the manifest's own validation
//! makes that section a superset of the behavior and lifecycle sections, so it
//! is the one list the guard walks. The guard fails closed: a path git does
//! not track, a path deleted from the working tree and a path whose bytes
//! differ from `HEAD` are each a refusal, and the refusal names every offending
//! path rather than the first.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::process::Command;
use tuning_campaign_support::provenance::ProducingInputs;

/// Why one closure path cannot enter a timed run.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Refusal {
    /// git does not track the path at all.
    Untracked(String),
    /// git tracks the path and the working tree or index differs from `HEAD`.
    Dirty {
        /// Repository-relative path.
        path: String,
        /// Two-letter porcelain status git reported for it.
        status: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Untracked(path) => write!(formatter, "{path} is not a committed campaign input"),
            Self::Dirty { path, status } => write!(
                formatter,
                "{path} differs from the committed campaign input (git status {status:?})"
            ),
        }
    }
}

/// Every path a timed run requires committed and clean, sorted and deduplicated.
///
/// `extra` carries the campaign inputs a plan names outside the manifest: its
/// campaign addendum and its family ledger.
pub fn closure(inputs: &ProducingInputs, extra: &[String]) -> Vec<String> {
    let mut paths: BTreeSet<String> = inputs.build_inputs.iter().cloned().collect();
    paths.extend(extra.iter().cloned());
    paths.into_iter().collect()
}

/// Classifies one closure against git's tracked set and porcelain status.
pub fn refusals(
    requested: &[String],
    tracked: &BTreeSet<String>,
    status: &BTreeMap<String, String>,
) -> Vec<Refusal> {
    requested
        .iter()
        .filter_map(|path| {
            if !tracked.contains(path) {
                Some(Refusal::Untracked(path.clone()))
            } else {
                status.get(path).map(|code| Refusal::Dirty {
                    path: path.clone(),
                    status: code.clone(),
                })
            }
        })
        .collect()
}

/// Decodes the NUL-separated paths of `git ls-files -z`.
pub fn parse_tracked(stdout: &[u8]) -> Result<BTreeSet<String>, String> {
    fields(stdout)?.into_iter().map(Ok).collect()
}

/// Decodes `git status --porcelain=v1 -z` into a path-to-status map.
///
/// A rename or copy entry carries its origin path as a second NUL-terminated
/// field; both the reported path and that origin are dirty.
pub fn parse_status(stdout: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let fields = fields(stdout)?;
    let mut status = BTreeMap::new();
    let mut index = 0;
    while index < fields.len() {
        let entry = &fields[index];
        index += 1;
        let (code, path) = entry
            .split_at_checked(3)
            .filter(|(_, path)| !path.is_empty())
            .ok_or_else(|| format!("git status entry {entry:?} is not a porcelain record"))?;
        let code = code[..2].to_owned();
        if code.starts_with('R') || code.starts_with('C') {
            let origin = fields
                .get(index)
                .ok_or("a git status rename entry lacks its origin path")?;
            index += 1;
            status.insert(origin.clone(), code.clone());
        }
        status.insert(path.to_owned(), code);
    }
    Ok(status)
}

/// Refuses unless every closure path is committed and clean under `root`.
///
/// Returns the checked closure on success. `manifest_path` and every entry of
/// `extra` are repository-relative.
pub fn check(root: &Path, manifest_path: &str, extra: &[String]) -> Result<Vec<String>, String> {
    let inputs = ProducingInputs::read_at(root, manifest_path)
        .map_err(|error| format!("{manifest_path}: {error}"))?;
    let requested = closure(&inputs, extra);
    let tracked = parse_tracked(&git(root, &["ls-files", "-z", "--"], &requested)?)?;
    let status = parse_status(&git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
        ],
        &requested,
    )?)?;
    let refusals = refusals(&requested, &tracked, &status);
    if refusals.is_empty() {
        return Ok(requested);
    }
    Err(refusals
        .iter()
        .map(Refusal::to_string)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn fields(stdout: &[u8]) -> Result<Vec<String>, String> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| {
            String::from_utf8(field.to_vec())
                .map_err(|_| "git reported a path that is not UTF-8".to_owned())
        })
        .collect()
}

fn git(root: &Path, arguments: &[&str], paths: &[String]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .args(paths)
        .output()
        .map_err(|error| format!("cannot run git {}: {error}", arguments[0]))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments[0],
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}
