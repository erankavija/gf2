mod config;
mod loader;
mod parser;

pub use config::ConfigError;
pub use loader::TestVectorSet;
pub use parser::ParseError;

use std::path::PathBuf;

/// The tree root that [`gf2_coding::test_support::dvb_vectors_path`] resolves.
pub fn test_vectors_path() -> PathBuf {
    gf2_coding::test_support::dvb_vectors_path()
}

/// True when the tree holds the `VV001-CR35_CSP` stream directory.
pub fn test_vectors_available() -> bool {
    test_vectors_path().join("VV001-CR35_CSP").exists()
}

/// Returns from the calling test when the DVB test vectors are absent.
#[macro_export]
macro_rules! require_test_vectors {
    () => {
        if !$crate::test_vectors::test_vectors_available() {
            eprintln!(
                "Skipping test: DVB test vectors not found at {:?}",
                $crate::test_vectors::test_vectors_path()
            );
            eprintln!(
                "Set DVB_TEST_VECTORS_PATH environment variable to the test vectors directory"
            );
            return;
        }
    };
}
