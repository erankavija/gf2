//! Embeds the source revision this build of `gf2-sim` was produced from.
//!
//! `permanent_campaign::provenance::build_revision` reads the value emitted
//! here, and the emission guard refuses to publish a dataset unless it equals
//! the repository's current `HEAD`. A revision that cannot be determined is
//! emitted as the empty string, which the guard reads as an unrecorded build
//! and refuses; the build itself still succeeds, so a source tarball or a host
//! without `git` compiles the crate normally and only loses the ability to
//! publish.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // A binary that kept a revision from before the current commit could
    // publish under a source it was not built from, so the crate recompiles
    // whenever HEAD or the branch it names moves. The cost is one `gf2-sim`
    // rebuild per commit.
    for input in revision_inputs() {
        println!("cargo::rerun-if-changed={}", input.display());
    }
    println!(
        "cargo::rustc-env=GF2_SIM_BUILD_GIT_REVISION={}",
        head_revision().unwrap_or_default()
    );
}

/// Returns `HEAD` as a full lowercase object name, if git can resolve one.
fn head_revision() -> Option<String> {
    let revision = git(&["rev-parse", "HEAD"])?;
    let canonical = revision.len() == 40
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    canonical.then_some(revision)
}

/// Returns the files whose change moves `HEAD`.
///
/// Paths that do not exist are dropped: naming one would make Cargo rerun this
/// script on every build.
fn revision_inputs() -> Vec<PathBuf> {
    let mut inputs = Vec::new();
    if let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) {
        inputs.push(PathBuf::from(head));
    }
    if let Some(reference) = git(&["symbolic-ref", "--quiet", "HEAD"]) {
        let loose = git(&["rev-parse", "--git-path", &reference]).map(PathBuf::from);
        match loose {
            // A branch whose loose ref has been packed away moves by a
            // `packed-refs` rewrite instead.
            Some(path) if path.exists() => inputs.push(path),
            _ => {
                if let Some(packed) = git(&["rev-parse", "--git-path", "packed-refs"]) {
                    inputs.push(PathBuf::from(packed));
                }
            }
        }
    }
    inputs.retain(|input| input.exists());
    inputs
}

/// Runs `git` in the package directory and returns its trimmed stdout.
fn git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
