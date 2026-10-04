//! Scratch directories for this crate's tests and its driver binary.
//! `gf2_core::test_scratch` is the workspace form of this module. This crate is
//! a dev-dependency of `gf2-core`, so depending back on it would close a package
//! cycle; the two stay in step by convention.

use std::ops::Deref;
use std::path::{Path, PathBuf};

use tempfile::{Builder, TempDir};

/// Set to a non-empty value to keep every scratch tree after its test ends.
pub const KEEP_VAR: &str = "GF2_KEEP_TEST_SCRATCH";

/// Reports whether [`KEEP_VAR`] asks scratch trees to outlive their test.
pub fn keep_scratch() -> bool {
    std::env::var_os(KEEP_VAR).is_some_and(|value| !value.is_empty())
}

/// An empty directory removed when the handle drops, including during panic
/// unwinding. Dereferences to its own path.
pub struct Scratch(TempDir);

impl Scratch {
    /// Returns the directory this handle owns.
    pub fn path(&self) -> &Path {
        self.0.path()
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.0.path()
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        self.0.path()
    }
}

impl std::fmt::Debug for Scratch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.path().fmt(formatter)
    }
}

/// Creates an empty `<prefix>-<random>` directory under `std::env::temp_dir()`.
///
/// # Panics
///
/// Panics if the directory cannot be created.
pub fn scratch(prefix: &str) -> Scratch {
    scratch_with(prefix, keep_scratch())
}

/// [`scratch`] with the keep decision supplied rather than read from the
/// environment.
///
/// # Panics
///
/// Panics if the directory cannot be created.
pub fn scratch_with(prefix: &str, keep: bool) -> Scratch {
    Scratch(
        Builder::new()
            .prefix(&format!("{prefix}-"))
            .disable_cleanup(keep)
            .tempdir()
            .expect("create scratch directory"),
    )
}

/// A path whose tree is removed when the handle drops, for the two cases
/// [`Scratch`] does not cover: a directory the caller must name exactly, and a
/// path a test needs to still be absent.
///
/// [`KEEP_VAR`] suppresses the removal, as it does for [`Scratch`].
pub struct ScratchPath {
    path: PathBuf,
    /// Present for [`ScratchPath::reserved`], whose removal it performs.
    root: Option<Scratch>,
    keep: bool,
}

impl ScratchPath {
    /// Creates `parent/name`, which must not already exist.
    ///
    /// # Panics
    ///
    /// Panics if the directory cannot be created.
    pub fn create(parent: &Path, name: &str) -> Self {
        let path = parent.join(name);
        std::fs::create_dir(&path).expect("create named scratch directory");
        Self {
            path,
            root: None,
            keep: keep_scratch(),
        }
    }

    /// Names `<fresh scratch root>/scratch` without creating it, so a caller
    /// that refuses a pre-existing path still sees one that is absent.
    ///
    /// # Panics
    ///
    /// Panics if the scratch root cannot be created.
    pub fn reserved(prefix: &str) -> Self {
        let root = scratch(prefix);
        let path = root.join("scratch");
        Self {
            path,
            root: Some(root),
            keep: keep_scratch(),
        }
    }

    /// Returns the path this handle owns.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Deref for ScratchPath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for ScratchPath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl std::fmt::Debug for ScratchPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(formatter)
    }
}

impl Drop for ScratchPath {
    fn drop(&mut self) {
        if self.root.is_none() && !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{scratch, scratch_with, ScratchPath};
    use std::fs;

    #[test]
    fn scratch_tree_is_removed_when_the_test_panics() {
        let handle = scratch("tuning-campaign-support-panic");
        let path = handle.to_path_buf();
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(move || {
            let _handle = handle;
            panic!("scratch test panic");
        });
        std::panic::set_hook(previous_hook);

        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn a_named_scratch_tree_is_removed_when_the_test_panics() {
        let parent = scratch("tuning-campaign-support-named-panic");
        let expected = parent.join("fixed-name");
        let handle = ScratchPath::create(&parent, "fixed-name");
        assert_eq!(handle.path(), expected);
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(move || {
            let _handle = handle;
            panic!("named scratch test panic");
        });
        std::panic::set_hook(previous_hook);

        assert!(result.is_err());
        assert!(!expected.exists());
    }

    #[test]
    fn a_reserved_scratch_path_is_absent_and_its_tree_is_removed_on_panic() {
        let handle = ScratchPath::reserved("tuning-campaign-support-reserved-panic");
        let path = handle.to_path_buf();
        assert!(!path.exists());
        fs::create_dir(&path).unwrap();
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(move || {
            let _handle = handle;
            panic!("reserved scratch test panic");
        });
        std::panic::set_hook(previous_hook);

        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn kept_scratch_tree_survives_its_handle() {
        let handle = scratch_with("tuning-campaign-support-keep", true);
        let path = handle.to_path_buf();
        drop(handle);

        assert!(path.exists());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn scratch_directories_are_distinct() {
        let first = scratch("tuning-campaign-support-distinct");
        let second = scratch("tuning-campaign-support-distinct");

        assert_ne!(first.path(), second.path());
        assert!(first.exists());
        assert!(second.exists());
    }
}
