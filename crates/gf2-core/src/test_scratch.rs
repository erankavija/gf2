//! Canonical scratch directory for this workspace's tests, benches, and bench
//! harnesses.

use std::ops::Deref;
use std::path::Path;

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

/// [`scratch`] rooted at `parent`, for work needing a real on-disk filesystem
/// rather than whatever `std::env::temp_dir()` is mounted on.
///
/// # Panics
///
/// Panics if the directory cannot be created.
pub fn scratch_in(parent: &Path, prefix: &str) -> Scratch {
    std::fs::create_dir_all(parent).expect("create scratch parent");
    Scratch(
        Builder::new()
            .prefix(&format!("{prefix}-"))
            .disable_cleanup(keep_scratch())
            .tempdir_in(parent)
            .expect("create scratch directory"),
    )
}

#[cfg(test)]
mod tests {
    use super::{scratch, scratch_with};
    use std::fs;

    #[test]
    fn scratch_tree_is_removed_when_the_test_panics() {
        let handle = scratch("gf2-scratch-panic");
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
    fn scratch_tree_is_removed_when_the_handle_drops() {
        let path = {
            let handle = scratch("gf2-scratch-drop");
            fs::write(handle.join("occupant"), b"x").unwrap();
            handle.to_path_buf()
        };

        assert!(!path.exists());
    }

    #[test]
    fn kept_scratch_tree_survives_its_handle() {
        let handle = scratch_with("gf2-scratch-keep", true);
        let path = handle.to_path_buf();
        drop(handle);

        assert!(path.exists());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn scratch_directories_are_distinct() {
        let first = scratch("gf2-scratch-distinct");
        let second = scratch("gf2-scratch-distinct");

        assert_ne!(first.path(), second.path());
        assert!(first.exists());
        assert!(second.exists());
    }
}
