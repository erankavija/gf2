//! Runtime resolution of the repository root and of the live files below it.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directory name of receipt input snapshots, which hold byte copies of live
/// files beside each receipt.
pub const SNAPSHOT_DIRECTORY: &str = "inputs";

/// Returns the canonical top-level directory of the git checkout containing
/// the current working directory.
///
/// # Errors
///
/// Fails when `git` cannot run or the working directory lies outside a
/// checkout.
pub fn repository_root() -> io::Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            "git cannot resolve the repository root from the working directory",
        ));
    }
    let top = String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::canonicalize(top.trim_end())
}

/// Sorted `root`-relative paths of the files whose name matches the glob
/// `name`, among those `git ls-files` reports as tracked or untracked and not
/// ignored; build output and nested worktrees stay out of the listing.
///
/// # Errors
///
/// Fails when git cannot list `root`.
pub fn listed_files(root: &Path, name: &str) -> io::Result<Vec<String>> {
    let listing = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
        ])
        .arg(format!(":(glob)**/{name}"))
        .output()?;
    if !listing.status.success() {
        return Err(io::Error::other(format!("git cannot list {name} files")));
    }
    let paths = listing
        .stdout
        .split(|&b| b == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .collect::<io::Result<BTreeSet<_>>>()?;
    Ok(paths.into_iter().collect())
}

/// Whether the `root`-relative `path` lies in a receipt input snapshot.
pub fn is_snapshot_copy(path: &str) -> bool {
    Path::new(path)
        .parent()
        .is_some_and(|parent| parent.iter().any(|part| part == SNAPSHOT_DIRECTORY))
}

/// The first path of each distinct content among `files`, in their order:
/// byte-identical copies are one file wherever they lie.
pub fn distinct_contents(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    files
        .into_iter()
        .filter_map(|(path, bytes)| seen.insert(bytes).then_some(path))
        .collect()
}

/// The `root`-relative path of the one live file named by the glob `name`
/// whose bytes satisfy `identifies`; receipt input snapshots are not live.
/// Byte-identical copies are one file, named by its lexicographically first
/// path.
///
/// # Errors
///
/// Fails when git cannot list `root`, a candidate cannot be read, or the
/// number of distinct identified contents is not exactly one.
pub fn locate_live(
    root: &Path,
    name: &str,
    identifies: impl Fn(&[u8]) -> bool,
) -> io::Result<String> {
    let mut found = Vec::new();
    for path in listed_files(root, name)? {
        if is_snapshot_copy(&path) {
            continue;
        }
        let bytes = match fs::read(root.join(&path)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            other => other?,
        };
        if identifies(&bytes) {
            found.push((path, bytes));
        }
    }
    match <[String; 1]>::try_from(distinct_contents(found)) {
        Ok([path]) => Ok(path),
        Err(found) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} live {name} files carry the identity, not one",
                found.len()
            ),
        )),
    }
}
