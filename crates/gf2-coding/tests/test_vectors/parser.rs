use gf2_core::BitVec;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct TestVector {
    pub frame_number: usize,
    pub block_number: usize,
    pub total_blocks: usize,
    pub data: BitVec,
}

#[derive(Debug)]
#[allow(dead_code)] // Test utility - fields may be used in future tests
pub struct TestVectorFile {
    pub test_point: String,
    pub config: String,
    frames: Vec<Vec<TestVector>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
    #[error("Invalid binary string at line {line}: {char}")]
    InvalidBinary { line: usize, char: char },
    #[error("Block count mismatch: expected {expected}, got {actual}")]
    BlockCountMismatch { expected: usize, actual: usize },
}

impl TestVectorFile {
    /// Parse test vector file from path
    pub fn from_file(path: &Path) -> Result<Self, ParseError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Extract test point and config from filename: VV001-CR35_TP04_CSP.txt
        let (config, test_point) = parse_filename(filename);

        let mut frames: Vec<Vec<TestVector>> = Vec::new();
        let mut current_frame: Vec<TestVector> = Vec::new();
        let mut current_frame_number = 0;
        let mut current_block_number = 0;
        let mut current_total_blocks = 0;
        let mut current_data_lines: Vec<String> = Vec::new();
        let mut line_number = 0;

        for line_result in reader.lines() {
            line_number += 1;
            let line = line_result?;
            let trimmed = line.trim();

            // Skip empty lines and comment lines starting with %
            if trimmed.is_empty() || trimmed.starts_with('%') {
                continue;
            }

            // Check for frame marker: # frame N
            if trimmed.starts_with("# frame ") {
                // Save current block if exists
                if !current_data_lines.is_empty() {
                    let data = parse_binary_lines(&current_data_lines)?;
                    current_frame.push(TestVector {
                        frame_number: current_frame_number,
                        block_number: current_block_number,
                        total_blocks: current_total_blocks,
                        data,
                    });
                    current_data_lines.clear();
                }

                // Save previous frame if exists
                if !current_frame.is_empty() {
                    frames.push(current_frame);
                    current_frame = Vec::new();
                }

                current_frame_number = parse_frame_marker(trimmed, line_number)?;
                continue;
            }

            // Check for block marker: # block M of K
            if trimmed.starts_with("# block ") {
                // Save previous block if exists
                if !current_data_lines.is_empty() {
                    let data = parse_binary_lines(&current_data_lines)?;
                    current_frame.push(TestVector {
                        frame_number: current_frame_number,
                        block_number: current_block_number,
                        total_blocks: current_total_blocks,
                        data,
                    });
                    current_data_lines.clear();
                }

                let (block_num, total_blocks) = parse_block_marker(trimmed, line_number)?;
                current_block_number = block_num;
                current_total_blocks = total_blocks;
                continue;
            }

            // Otherwise, treat as binary data line
            if !trimmed.chars().all(|c| c == '0' || c == '1') {
                return Err(ParseError::InvalidBinary {
                    line: line_number,
                    char: trimmed.chars().find(|&c| c != '0' && c != '1').unwrap(),
                });
            }

            current_data_lines.push(trimmed.to_string());
        }

        // Save final block and frame
        if !current_data_lines.is_empty() {
            let data = parse_binary_lines(&current_data_lines)?;
            current_frame.push(TestVector {
                frame_number: current_frame_number,
                block_number: current_block_number,
                total_blocks: current_total_blocks,
                data,
            });
        }

        if !current_frame.is_empty() {
            frames.push(current_frame);
        }

        // Validate block counts
        for frame in frames.iter() {
            if let Some(first) = frame.first() {
                let expected = first.total_blocks;
                let actual = frame.len();
                if expected != actual {
                    return Err(ParseError::BlockCountMismatch { expected, actual });
                }
            }
        }

        Ok(TestVectorFile {
            test_point,
            config,
            frames,
        })
    }

    /// Get all blocks for a specific frame
    pub fn frame(&self, frame_idx: usize) -> &[TestVector] {
        self.frames
            .get(frame_idx)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get total number of frames
    pub fn num_frames(&self) -> usize {
        self.frames.len()
    }
}

fn parse_filename(filename: &str) -> (String, String) {
    // VV001-CR35_TP04_CSP.txt -> ("VV001-CR35", "TP04")
    let parts: Vec<&str> = filename.split('_').collect();
    if parts.len() >= 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        ("unknown".to_string(), "unknown".to_string())
    }
}

fn parse_frame_marker(line: &str, line_number: usize) -> Result<usize, ParseError> {
    // Parse "# frame 1" -> 1
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        parts[2].parse().map_err(|_| ParseError::Parse {
            line: line_number,
            message: format!("Invalid frame number: {}", parts[2]),
        })
    } else {
        Err(ParseError::Parse {
            line: line_number,
            message: "Invalid frame marker format".to_string(),
        })
    }
}

fn parse_block_marker(line: &str, line_number: usize) -> Result<(usize, usize), ParseError> {
    // Parse "# block 1 of 202" -> (1, 202)
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 5 {
        let block_num = parts[2].parse().map_err(|_| ParseError::Parse {
            line: line_number,
            message: format!("Invalid block number: {}", parts[2]),
        })?;
        let total_blocks = parts[4].parse().map_err(|_| ParseError::Parse {
            line: line_number,
            message: format!("Invalid total blocks: {}", parts[4]),
        })?;
        Ok((block_num, total_blocks))
    } else {
        Err(ParseError::Parse {
            line: line_number,
            message: "Invalid block marker format".to_string(),
        })
    }
}

fn parse_binary_lines(lines: &[String]) -> Result<BitVec, ParseError> {
    let mut bits = BitVec::new();
    for line in lines {
        for c in line.chars() {
            match c {
                '0' => bits.push_bit(false),
                '1' => bits.push_bit(true),
                _ => unreachable!("Invalid binary char already checked"),
            }
        }
    }
    Ok(bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_filename() {
        let (config, tp) = parse_filename("VV001-CR35_TP04_CSP.txt");
        assert_eq!(config, "VV001-CR35");
        assert_eq!(tp, "TP04");
    }

    #[test]
    fn test_parse_frame_marker() {
        assert_eq!(parse_frame_marker("# frame 1", 1).unwrap(), 1);
        assert_eq!(parse_frame_marker("# frame 42", 2).unwrap(), 42);
        assert!(parse_frame_marker("# frame", 3).is_err());
    }

    #[test]
    fn test_parse_block_marker() {
        assert_eq!(parse_block_marker("# block 1 of 202", 1).unwrap(), (1, 202));
        assert_eq!(
            parse_block_marker("# block 42 of 100", 2).unwrap(),
            (42, 100)
        );
        assert!(parse_block_marker("# block 1", 3).is_err());
    }

    #[test]
    fn test_parse_binary_lines() {
        let lines = vec!["1010".to_string(), "1100".to_string()];
        let bitvec = parse_binary_lines(&lines).unwrap();
        assert_eq!(bitvec.len(), 8);
        assert!(bitvec.get(0));
        assert!(!bitvec.get(1));
        assert!(bitvec.get(2));
        assert!(!bitvec.get(3));
        assert!(bitvec.get(4));
        assert!(bitvec.get(5));
        assert!(!bitvec.get(6));
        assert!(!bitvec.get(7));
    }

    #[test]
    fn test_parse_simple_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "% Comment line").unwrap();
        writeln!(file, "# frame 1").unwrap();
        writeln!(file, "# block 1 of 2").unwrap();
        writeln!(file, "10101010").unwrap();
        writeln!(file, "11001100").unwrap();
        writeln!(file, "# block 2 of 2").unwrap();
        writeln!(file, "00110011").unwrap();
        writeln!(file, "01010101").unwrap();
        file.flush().unwrap();

        let tvf = TestVectorFile::from_file(file.path()).unwrap();
        assert_eq!(tvf.num_frames(), 1);
        assert_eq!(tvf.frame(0).len(), 2);

        let block1 = &tvf.frame(0)[0];
        assert_eq!(block1.frame_number, 1);
        assert_eq!(block1.block_number, 1);
        assert_eq!(block1.total_blocks, 2);
        assert_eq!(block1.data.len(), 16);

        let block2 = &tvf.frame(0)[1];
        assert_eq!(block2.frame_number, 1);
        assert_eq!(block2.block_number, 2);
        assert_eq!(block2.total_blocks, 2);
        assert_eq!(block2.data.len(), 16);
    }

    #[test]
    fn test_parse_multiple_frames() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# frame 1").unwrap();
        writeln!(file, "# block 1 of 1").unwrap();
        writeln!(file, "1010").unwrap();
        writeln!(file, "# frame 2").unwrap();
        writeln!(file, "# block 1 of 1").unwrap();
        writeln!(file, "0101").unwrap();
        file.flush().unwrap();

        let tvf = TestVectorFile::from_file(file.path()).unwrap();
        assert_eq!(tvf.num_frames(), 2);
        assert_eq!(tvf.frame(0)[0].data.len(), 4);
        assert_eq!(tvf.frame(1)[0].data.len(), 4);
    }

    #[test]
    fn test_invalid_binary() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# frame 1").unwrap();
        writeln!(file, "# block 1 of 1").unwrap();
        writeln!(file, "10X0").unwrap();
        file.flush().unwrap();

        let result = TestVectorFile::from_file(file.path());
        assert!(matches!(result, Err(ParseError::InvalidBinary { .. })));
    }

    #[test]
    fn test_block_count_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# frame 1").unwrap();
        writeln!(file, "# block 1 of 3").unwrap();
        writeln!(file, "1010").unwrap();
        writeln!(file, "# block 2 of 3").unwrap();
        writeln!(file, "0101").unwrap();
        // Missing block 3
        file.flush().unwrap();

        let result = TestVectorFile::from_file(file.path());
        assert!(matches!(result, Err(ParseError::BlockCountMismatch { .. })));
    }
}
