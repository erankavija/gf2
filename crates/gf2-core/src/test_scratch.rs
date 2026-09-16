//! Canonical scratch directory for this workspace's tests, benches, and bench
//! harnesses.

use tempfile::{Builder, TempDir};

/// Set to a non-empty value to keep every scratch tree after its test ends.
pub const KEEP_VAR: &str = "GF2_KEEP_TEST_SCRATCH";

/// Reports whether [`KEEP_VAR`] asks scratch trees to outlive their test.
pub fn keep_scratch() -> bool {
    std::env::var_os(KEEP_VAR).is_some_and(|value| !value.is_empty())
}

/// Creates an empty `<prefix>-<random>` directory under `std::env::temp_dir()`.
///
/// The directory is removed when the returned handle drops, including during
/// panic unwinding.
///
/// # Panics
///
/// Panics if the directory cannot be created.
pub fn scratch(prefix: &str) -> TempDir {
    scratch_with(prefix, keep_scratch())
}

/// [`scratch`] with the keep decision supplied rather than read from the
/// environment.
pub fn scratch_with(prefix: &str, keep: bool) -> TempDir {
    Builder::new()
        .prefix(&format!("{prefix}-"))
        .disable_cleanup(keep)
        .tempdir()
        .expect("create scratch directory")
}

/// [`scratch`] rooted at `parent` instead of `std::env::temp_dir()`, for work
/// that needs a real on-disk filesystem rather than the system temp mount.
///
/// # Panics
///
/// Panics if the directory cannot be created.
pub fn scratch_in(parent: &std::path::Path, prefix: &str) -> TempDir {
    std::fs::create_dir_all(parent).expect("create scratch parent");
    Builder::new()
        .prefix(&format!("{prefix}-"))
        .disable_cleanup(keep_scratch())
        .tempdir_in(parent)
        .expect("create scratch directory")
}

#[cfg(test)]
mod tests {
    use super::{scratch, scratch_with};
    use std::fs;

    #[test]
    fn scratch_tree_is_removed_when_the_test_panics() {
        let handle = scratch("gf2-scratch-panic");
        let path = handle.path().to_path_buf();
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
            assert!(handle.path().exists());
            handle.path().to_path_buf()
        };

        assert!(!path.exists());
    }

    #[test]
    fn kept_scratch_tree_survives_its_handle() {
        let handle = scratch_with("gf2-scratch-keep", true);
        let path = handle.path().to_path_buf();
        drop(handle);

        assert!(path.exists());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn scratch_directories_are_distinct() {
        let first = scratch("gf2-scratch-distinct");
        let second = scratch("gf2-scratch-distinct");

        assert_ne!(first.path(), second.path());
        assert!(first.path().exists());
        assert!(second.path().exists());
    }
}
