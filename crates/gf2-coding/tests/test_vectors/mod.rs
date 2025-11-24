mod config;
mod loader;
mod parser;

pub use config::ConfigError;
pub use loader::TestVectorSet;
pub use parser::ParseError;

use std::env;
use std::path::PathBuf;

/// Get test vector base path from environment or default location
pub fn test_vectors_path() -> PathBuf {
    env::var("DVB_TEST_VECTORS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env::var("HOME").expect("HOME not set")).join("dvb_test_vectors")
        })
}

/// Check if test vectors are available
pub fn test_vectors_available() -> bool {
    test_vectors_path().join("VV001-CR35_CSP").exists()
}

/// Skip test with helpful message if vectors not found
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
