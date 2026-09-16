//! `gf2_core::test_scratch` is the workspace form, while this crate is dependency-isolated from the gf2 crates.

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

#[cfg(test)]
mod tests {
    use super::{scratch, scratch_with};
    use std::fs;

    #[test]
    fn scratch_tree_is_removed_when_the_test_panics() {
        let handle = scratch("tuning-campaign-support-panic");
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
            let handle = scratch("tuning-campaign-support-drop");
            assert!(handle.path().exists());
            handle.path().to_path_buf()
        };

        assert!(!path.exists());
    }

    #[test]
    fn kept_scratch_tree_survives_its_handle() {
        let handle = scratch_with("tuning-campaign-support-keep", true);
        let path = handle.path().to_path_buf();
        drop(handle);

        assert!(path.exists());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn scratch_directories_are_distinct() {
        let first = scratch("tuning-campaign-support-distinct");
        let second = scratch("tuning-campaign-support-distinct");

        assert_ne!(first.path(), second.path());
        assert!(first.path().exists());
        assert!(second.path().exists());
    }
}
