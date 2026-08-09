//! Embeds the source revision this build of `gf2-sim` was produced from.
//!
//! `permanent_campaign::provenance::build_revision` reads the value emitted
//! here, and the emission guard refuses to publish a dataset unless it equals
//! the repository's current `HEAD`. A revision that cannot be determined is
//! emitted as the empty string, which the guard reads as an unrecorded build
//! and refuses; the build itself still succeeds, so a source tarball or a host
//! without `git` compiles the crate normally and only loses the ability to
//! publish.
//!
//! # Following `HEAD` is opt-in
//!
//! By default this script declares no rebuild dependency on `HEAD` or on the
//! ref it names, so landing a commit does not recompile `gf2-sim`. That matters
//! because most commits here touch no code at all — a working session lands
//! many workflow-state commits, each followed by a CI run — while `gf2-sim` is
//! the heaviest crate in the workspace.
//!
//! The embedded revision therefore goes stale the moment a commit lands, and
//! `approve_emission` refuses with a revision mismatch until the crate is next
//! rebuilt. That is the intended outcome rather than a gap: the guard fails
//! closed, so a stale binary can never publish under a source it was not built
//! from. REQ-01 asks the emitting binary to embed the revision it was built
//! from and to emit only when that equals the current `HEAD`; it asks nothing
//! about the binary keeping itself current, and the refusal is precisely what
//! makes the criterion hold. The only thing the default gives up is automatic
//! freshness, which costs nothing to anyone who is not publishing.
//!
//! A publisher sets `GF2_SIM_TRACK_HEAD` to any value other than `0` for the
//! build that will emit, matching this repository's existing `GF2_BENCH`
//! convention for opt-in build and test behaviour. That build declares `HEAD`
//! and its ref as rebuild inputs and follows them commit by commit. The switch
//! is deliberately an environment variable rather than a Cargo feature:
//! `scripts/cargo-ci.sh` builds with `--all-features` on a ROCm host, which
//! would turn tracking back on for exactly the runs the default protects.

use std::path::PathBuf;
use std::process::Command;

/// Opt-in variable that makes this build follow `HEAD`.
const TRACK_HEAD: &str = "GF2_SIM_TRACK_HEAD";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Toggling the variable has to re-run this script, or switching tracking on
    // would leave the previous build's decision — and its stale revision — in
    // place.
    println!("cargo::rerun-if-env-changed={TRACK_HEAD}");
    if tracking_head() {
        for input in revision_inputs() {
            println!("cargo::rerun-if-changed={}", input.display());
        }
    }
    println!(
        "cargo::rustc-env=GF2_SIM_BUILD_GIT_REVISION={}",
        head_revision().unwrap_or_default()
    );
}

/// Returns whether this build was asked to follow `HEAD`.
fn tracking_head() -> bool {
    matches!(std::env::var(TRACK_HEAD), Ok(value) if value != "0")
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
