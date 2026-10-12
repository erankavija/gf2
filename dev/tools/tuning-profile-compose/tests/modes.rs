//! Drives the built composer through its command line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const PROFILE_ID: &str = "campaign";
const ASSEMBLED_AT: &str = "2026-08-25T19:00:00Z";
const SOURCE_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const TOOL_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Outputs of the `core-owner`, `algebra-owner` and two-owner `complete`
/// modes for the arguments above: the stable external contract of those modes.
const GOLDEN_CORE_OWNER: &str = include_str!("golden/core-owner.json");
const GOLDEN_ALGEBRA_OWNER: &str = include_str!("golden/algebra-owner.json");
const GOLDEN_COMPLETE: &str = include_str!("golden/complete.json");

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "modes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn compose(mode: &str, paths: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tuning-profile-compose"))
        .arg(mode)
        .args(paths)
        .args([
            PROFILE_ID,
            ASSEMBLED_AT,
            SOURCE_REVISION,
            "false",
            TOOL_SHA256,
        ])
        .output()
        .unwrap()
}

fn composed(mode: &str, paths: &[&Path]) -> String {
    let output = compose(mode, paths);
    assert!(
        output.status.success(),
        "{mode} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(paths.last().unwrap()).unwrap()
}

#[test]
fn two_owner_modes_reproduce_their_pinned_outputs() {
    let scratch = Scratch::new();
    let core = scratch.file("core-owner.json");
    let algebra = scratch.file("algebra-owner.json");
    let complete = scratch.file("complete.json");

    assert_eq!(composed("core-owner", &[&core]), GOLDEN_CORE_OWNER);
    assert_eq!(composed("algebra-owner", &[&algebra]), GOLDEN_ALGEBRA_OWNER);
    assert_eq!(
        composed("complete", &[&core, &algebra, &complete]),
        GOLDEN_COMPLETE
    );
}
