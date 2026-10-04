mod config;
mod loader;
mod parser;

pub use config::ConfigError;
pub use loader::TestVectorSet;
pub use parser::ParseError;

use std::path::PathBuf;

pub fn test_vectors_path() -> PathBuf {
    gf2_coding::test_support::dvb_vectors_path()
}

pub fn test_vectors_available() -> bool {
    test_vectors_path().join("VV001-CR35_CSP").exists()
}

/// Returns from the calling test when the DVB test vectors (`@/citation/DvbVerification2010`) are absent.
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
