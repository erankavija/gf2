use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::identity::{ArtifactPath, GitRevision};

/// The one root that repository paths resolve through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRoot(PathBuf);

impl RepositoryRoot {
    /// Resolves the top level of the git checkout containing `path`, at any depth.
    ///
    /// # Errors
    ///
    /// Fails when `git` cannot run or `path` lies outside a checkout.
    pub fn containing(path: &Path) -> io::Result<Self> {
        let top_level = git(path, &["rev-parse", "--show-toplevel"])?;
        Ok(Self(fs::canonicalize(top_level)?))
    }

    /// Returns the filesystem location of `artifact`.
    pub fn resolve(&self, artifact: &ArtifactPath) -> PathBuf {
        self.0.join(artifact.as_str())
    }

    /// Returns the revision checked out at this root.
    ///
    /// # Errors
    ///
    /// Fails when `git` cannot run or the checkout has no commit.
    pub fn revision(&self) -> io::Result<GitRevision> {
        git(&self.0, &["rev-parse", "HEAD"])?
            .parse()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

fn git(directory: &Path, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git {} in {}: {}",
            arguments.join(" "),
            directory.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.strip_suffix('\n').unwrap_or(&text).to_owned())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
