use super::{config::DvbConfig, parser::TestVectorFile, ConfigError, ParseError};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct TestVectorSet {
    pub config: DvbConfig,
    pub tp04: Option<TestVectorFile>,  // BCH input
    pub tp05: Option<TestVectorFile>,  // BCH output / LDPC input
    pub tp06: Option<TestVectorFile>,  // LDPC output
    pub tp07a: Option<TestVectorFile>, // Bit interleaved
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),
    #[error("Configuration directory not found: {0}")]
    DirNotFound(PathBuf),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl TestVectorSet {
    /// Loads the test points under `<base_path>/<reference>_CSP`; a missing
    /// or unparsable file yields `None`.
    pub fn load(base_path: &Path, reference: &str) -> Result<Self, LoadError> {
        let config = DvbConfig::from_reference(reference)?;
        let config_dir = base_path.join(format!("{}_CSP", reference));

        if !config_dir.exists() {
            return Err(LoadError::DirNotFound(config_dir));
        }

        let tp04 = Self::load_test_point_optional(&config_dir, reference, "04");
        let tp05 = Self::load_test_point_optional(&config_dir, reference, "05");
        let tp06 = Self::load_test_point_optional(&config_dir, reference, "06");
        let tp07a = Self::load_test_point_optional(&config_dir, reference, "07a");

        Ok(TestVectorSet {
            config,
            tp04,
            tp05,
            tp06,
            tp07a,
        })
    }

    /// One test point of `reference`; an absent or unparsable file is a [`LoadError`].
    #[allow(dead_code)]
    pub fn load_test_point(
        base_path: &Path,
        reference: &str,
        tp: &str,
    ) -> Result<TestVectorFile, LoadError> {
        let config_dir = base_path.join(format!("{}_CSP", reference));
        let test_point_dir =
            config_dir.join(format!("TestPoint{:0>2}", tp.trim_start_matches('0')));
        let file_path = test_point_dir.join(format!("{}_TP{}_CSP.txt", reference, tp));

        Ok(TestVectorFile::from_file(&file_path)?)
    }

    fn load_test_point_optional(
        config_dir: &Path,
        reference: &str,
        tp: &str,
    ) -> Option<TestVectorFile> {
        // The directory name drops the letter suffix: "07a" -> "TestPoint07".
        let tp_base = tp.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let test_point_dir = config_dir.join(format!("TestPoint{}", tp_base));
        let file_path = test_point_dir.join(format!("{}_TP{}_CSP.txt", reference, tp));

        if file_path.exists() {
            TestVectorFile::from_file(&file_path).ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_vectors;

    #[test]
    #[ignore = "sim: requires the DVB test vectors (DVB_TEST_VECTORS_PATH)"]
    fn test_load_vv001_cr35() {
        if !test_vectors::test_vectors_available() {
            eprintln!("Test vectors not available, skipping test");
            return;
        }

        let base_path = test_vectors::test_vectors_path();
        let vectors = TestVectorSet::load(&base_path, "VV001-CR35").unwrap();

        assert_eq!(vectors.config.name, "VV001-CR35");
        assert!(vectors.tp04.is_some());
        assert!(vectors.tp05.is_some());
        assert!(vectors.tp06.is_some());
        assert!(vectors.tp07a.is_some());
    }

    #[test]
    #[ignore = "sim: requires the DVB test vectors (DVB_TEST_VECTORS_PATH)"]
    fn test_load_tp04_structure() {
        if !test_vectors::test_vectors_available() {
            eprintln!("Test vectors not available, skipping test");
            return;
        }

        let base_path = test_vectors::test_vectors_path();
        let vectors = TestVectorSet::load(&base_path, "VV001-CR35").unwrap();
        let tp04 = vectors.tp04.expect("TP04 should be present");

        assert!(tp04.num_frames() > 0, "Should have at least one frame");

        let frame0 = tp04.frame(0);
        assert!(!frame0.is_empty(), "Frame 0 should have blocks");

        if let Some(first_block) = frame0.first() {
            println!(
                "First block: frame {}, block {} of {}, {} bits",
                first_block.frame_number,
                first_block.block_number,
                first_block.total_blocks,
                first_block.data.len()
            );

            assert_eq!(first_block.frame_number, 1);
            assert_eq!(first_block.block_number, 1);
            assert!(first_block.total_blocks > 0);
            assert!(!first_block.data.is_empty());
        }
    }

    #[test]
    #[ignore = "sim: requires the DVB test vectors (DVB_TEST_VECTORS_PATH)"]
    fn test_all_test_points_consistent() {
        if !test_vectors::test_vectors_available() {
            eprintln!("Test vectors not available, skipping test");
            return;
        }

        let base_path = test_vectors::test_vectors_path();
        let vectors = TestVectorSet::load(&base_path, "VV001-CR35").unwrap();

        let tp04 = vectors.tp04.as_ref().expect("TP04 should be present");
        let tp05 = vectors.tp05.as_ref().expect("TP05 should be present");
        let tp06 = vectors.tp06.as_ref().expect("TP06 should be present");

        assert_eq!(tp04.num_frames(), tp05.num_frames());
        assert_eq!(tp04.num_frames(), tp06.num_frames());

        assert_eq!(tp04.frame(0).len(), tp05.frame(0).len());
        assert_eq!(tp04.frame(0).len(), tp06.frame(0).len());
    }

    #[test]
    fn test_load_nonexistent_config() {
        let base_path = test_vectors::test_vectors_path();
        let result = TestVectorSet::load(&base_path, "VV999-CR99");

        assert!(result.is_err());
    }
}
