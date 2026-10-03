//! Runtime resolution of the repository root.

use std::io;
use std::path::PathBuf;
use std::process::Command;

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
