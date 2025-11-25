//! BitMatrix serialization and deserialization.

use super::{error::*, format::*};
use crate::BitMatrix;
use std::io::{Read, Write};

/// Metadata for BitMatrix serialization
#[derive(Debug)]
#[cfg_attr(feature = "io", derive(serde::Serialize, serde::Deserialize))]
struct BitMatrixMetadata {
    #[serde(rename = "type")]
    type_name: String,
    rows: usize,
    cols: usize,
    version: u32,
}

impl BitMatrix {
    /// Save BitMatrix to a file with specified format
    pub fn save_to_file_with_format<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        format: super::SerializationFormat,
    ) -> Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write_to_with_format(&mut writer, format)
    }

    /// Save BitMatrix to a file (binary format)
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.save_to_file_with_format(path, super::SerializationFormat::Binary)
    }

    /// Load BitMatrix from a file (binary format)
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_from(&mut reader)
    }

    /// Load BitMatrix from a file with specified format
    pub fn load_from_file_with_format<P: AsRef<std::path::Path>>(
        path: P,
        format: super::SerializationFormat,
    ) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_from_with_format(&mut reader, format)
    }

    /// Write BitMatrix to a writer with specified format
    pub fn write_to_with_format<W: Write>(
        &self,
        writer: &mut W,
        format: super::SerializationFormat,
    ) -> Result<()> {
        match format {
            super::SerializationFormat::Binary => self.write_binary(writer),
            super::SerializationFormat::Text => self.write_text(writer),
            super::SerializationFormat::Hex => self.write_hex(writer),
        }
    }

    /// Write BitMatrix to a writer (binary format)
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.write_binary(writer)
    }

    /// Write BitMatrix in binary format
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Create metadata
        let metadata = BitMatrixMetadata {
            type_name: "BitMatrix".to_string(),
            rows: self.rows(),
            cols: self.cols(),
            version: 1,
        };

        let metadata_json = serde_json::to_vec(&metadata)
            .map_err(|e| IoError::InvalidData(format!("Failed to serialize metadata: {}", e)))?;

        // Calculate data length: rows * stride_words * 8 bytes
        let stride_words = if self.cols() == 0 {
            0
        } else {
            self.cols().div_ceil(64)
        };
        let data_len = self.rows() * stride_words * 8;

        // Write header
        let header = Header::new(
            TypeTag::BitMatrix,
            metadata_json.len() as u32,
            data_len as u64,
        );
        header.write_to(writer)?;

        // Write metadata
        writer.write_all(&metadata_json)?;

        // Write data (row-major, words in little-endian)
        for row_idx in 0..self.rows() {
            for word_idx in 0..stride_words {
                let word = self.get_word(row_idx, word_idx);
                writer.write_all(&word.to_le_bytes())?;
            }
        }

        Ok(())
    }

    /// Write BitMatrix in text format (human-readable)
    fn write_text<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Write dimensions as first line
        writeln!(writer, "{} {}", self.rows(), self.cols())?;

        // Write each row as ASCII '0' and '1'
        for row in 0..self.rows() {
            for col in 0..self.cols() {
                write!(writer, "{}", if self.get(row, col) { '1' } else { '0' })?;
            }
            writeln!(writer)?;
        }

        Ok(())
    }

    /// Write BitMatrix in hex format
    fn write_hex<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Write dimensions as first line
        writeln!(writer, "{} {}", self.rows(), self.cols())?;

        // Write each row's words in hex (16 chars per word)
        let stride_words = if self.cols() == 0 {
            0
        } else {
            self.cols().div_ceil(64)
        };
        for row in 0..self.rows() {
            for word_idx in 0..stride_words {
                let word = self.get_word(row, word_idx);
                write!(writer, "{:016X}", word)?;
            }
            writeln!(writer)?;
        }

        Ok(())
    }

    /// Read BitMatrix from reader with specified format
    pub fn read_from_with_format<R: Read>(
        reader: &mut R,
        format: super::SerializationFormat,
    ) -> Result<Self> {
        match format {
            super::SerializationFormat::Binary => Self::read_binary(reader),
            super::SerializationFormat::Text => Self::read_text(reader),
            super::SerializationFormat::Hex => Self::read_hex(reader),
        }
    }

    /// Read BitMatrix from reader (binary format)
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        Self::read_binary(reader)
    }

    /// Read BitMatrix from binary format
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self> {
        // Read and validate header
        let header = Header::read_from(reader)?;

        if header.type_tag != TypeTag::BitMatrix {
            return Err(IoError::InvalidData(format!(
                "Expected BitMatrix type tag, got {:?}",
                header.type_tag
            )));
        }

        // Read metadata
        let mut metadata_buf = vec![0u8; header.metadata_len as usize];
        reader.read_exact(&mut metadata_buf)?;

        let metadata: BitMatrixMetadata = serde_json::from_slice(&metadata_buf)
            .map_err(|e| IoError::InvalidData(format!("Failed to parse metadata: {}", e)))?;

        // Validate metadata
        if metadata.type_name != "BitMatrix" {
            return Err(IoError::InvalidData(format!(
                "Expected BitMatrix type, got {}",
                metadata.type_name
            )));
        }

        // Create matrix
        let mut matrix = BitMatrix::zeros(metadata.rows, metadata.cols);

        // Read data
        let stride_words = if metadata.cols == 0 {
            0
        } else {
            metadata.cols.div_ceil(64)
        };
        let expected_words = metadata.rows * stride_words;
        let expected_bytes = expected_words * 8;

        if header.data_len as usize != expected_bytes {
            return Err(IoError::InvalidData(format!(
                "Data length mismatch: header says {} bytes, expected {} bytes for {}x{} matrix",
                header.data_len, expected_bytes, metadata.rows, metadata.cols
            )));
        }

        for row in 0..metadata.rows {
            for word_idx in 0..stride_words {
                let mut word_buf = [0u8; 8];
                reader.read_exact(&mut word_buf)?;
                let word = u64::from_le_bytes(word_buf);
                matrix.set_word(row, word_idx, word);
            }
        }

        Ok(matrix)
    }

    /// Read BitMatrix from text format
    fn read_text<R: Read>(reader: &mut R) -> Result<Self> {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(reader);

        // Read dimensions
        let mut dim_line = String::new();
        reader.read_line(&mut dim_line)?;
        let dims: Vec<&str> = dim_line.split_whitespace().collect();
        if dims.len() != 2 {
            return Err(IoError::InvalidData(
                "Text format must start with 'rows cols'".to_string(),
            ));
        }

        let rows: usize = dims[0]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid row count".to_string()))?;
        let cols: usize = dims[1]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid column count".to_string()))?;

        // Create matrix
        let mut matrix = BitMatrix::zeros(rows, cols);

        // Read each row
        for row in 0..rows {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let line = line.trim();

            if line.len() != cols {
                return Err(IoError::InvalidData(format!(
                    "Row {} has {} bits, expected {}",
                    row,
                    line.len(),
                    cols
                )));
            }

            for (col, ch) in line.chars().enumerate() {
                match ch {
                    '0' => { /* already zero */ }
                    '1' => matrix.set(row, col, true),
                    _ => {
                        return Err(IoError::InvalidData(format!(
                            "Invalid character '{}' at row {}, col {}",
                            ch, row, col
                        )))
                    }
                }
            }
        }

        Ok(matrix)
    }

    /// Read BitMatrix from hex format
    fn read_hex<R: Read>(reader: &mut R) -> Result<Self> {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(reader);

        // Read dimensions
        let mut dim_line = String::new();
        reader.read_line(&mut dim_line)?;
        let dims: Vec<&str> = dim_line.split_whitespace().collect();
        if dims.len() != 2 {
            return Err(IoError::InvalidData(
                "Hex format must start with 'rows cols'".to_string(),
            ));
        }

        let rows: usize = dims[0]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid row count".to_string()))?;
        let cols: usize = dims[1]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid column count".to_string()))?;

        // Create matrix
        let mut matrix = BitMatrix::zeros(rows, cols);
        let stride_words = if cols == 0 { 0 } else { cols.div_ceil(64) };

        // Read each row
        for row in 0..rows {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let line = line.trim();

            let expected_hex_chars = stride_words * 16;
            if line.len() != expected_hex_chars {
                return Err(IoError::InvalidData(format!(
                    "Row {} has {} hex chars, expected {}",
                    row,
                    line.len(),
                    expected_hex_chars
                )));
            }

            for word_idx in 0..stride_words {
                let start = word_idx * 16;
                let end = start + 16;
                let hex_word = &line[start..end];
                let word = u64::from_str_radix(hex_word, 16).map_err(|_| {
                    IoError::InvalidData(format!(
                        "Invalid hex word '{}' at row {}, word {}",
                        hex_word, row, word_idx
                    ))
                })?;
                matrix.set_word(row, word_idx, word);
            }
        }

        Ok(matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BitMatrix;

    // Helper to create a simple test matrix
    fn create_test_matrix() -> BitMatrix {
        let mut m = BitMatrix::zeros(3, 5);
        m.set(0, 0, true);
        m.set(0, 4, true);
        m.set(1, 2, true);
        m.set(2, 1, true);
        m.set(2, 3, true);
        m
    }

    #[test]
    fn test_binary_roundtrip_simple() {
        let original = create_test_matrix();
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = BitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_binary_roundtrip_empty() {
        let original = BitMatrix::zeros(0, 0);
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = BitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_binary_roundtrip_single_row() {
        let original = BitMatrix::zeros(1, 100);
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = BitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_binary_roundtrip_single_col() {
        let original = BitMatrix::zeros(100, 1);
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = BitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_binary_roundtrip_word_boundary() {
        // Test exactly at word boundary (64 columns)
        let mut original = BitMatrix::zeros(3, 64);
        original.set(0, 0, true);
        original.set(0, 63, true);
        original.set(1, 32, true);

        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        let loaded = BitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_binary_roundtrip_multi_word() {
        // Test with multiple words per row (65 columns = 2 words)
        let mut original = BitMatrix::zeros(2, 65);
        original.set(0, 0, true);
        original.set(0, 64, true);
        original.set(1, 32, true);

        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        let loaded = BitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_binary_roundtrip_identity() {
        let original = BitMatrix::identity(10);
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = BitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_text_format_simple() {
        let original = create_test_matrix();
        let mut buffer = Vec::new();

        original
            .write_to_with_format(&mut buffer, super::super::SerializationFormat::Text)
            .unwrap();
        let text = String::from_utf8(buffer.clone()).unwrap();

        // Verify format
        assert!(text.starts_with("3 5\n"));
        assert!(text.contains("10001\n"));
        assert!(text.contains("00100\n"));
        assert!(text.contains("01010\n"));

        // Test roundtrip
        let loaded = BitMatrix::read_from_with_format(
            &mut buffer.as_slice(),
            super::super::SerializationFormat::Text,
        )
        .unwrap();
        assert_eq!(original, loaded);
    }

    #[test]
    fn test_text_format_empty() {
        let original = BitMatrix::zeros(0, 0);
        let mut buffer = Vec::new();

        original
            .write_to_with_format(&mut buffer, super::super::SerializationFormat::Text)
            .unwrap();
        let loaded = BitMatrix::read_from_with_format(
            &mut buffer.as_slice(),
            super::super::SerializationFormat::Text,
        )
        .unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_hex_format_simple() {
        let original = create_test_matrix();
        let mut buffer = Vec::new();

        original
            .write_to_with_format(&mut buffer, super::super::SerializationFormat::Hex)
            .unwrap();
        let text = String::from_utf8(buffer.clone()).unwrap();

        // Verify format (each row has 1 word = 16 hex chars)
        assert!(text.starts_with("3 5\n"));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4); // dimensions + 3 rows

        // Test roundtrip
        let loaded = BitMatrix::read_from_with_format(
            &mut buffer.as_slice(),
            super::super::SerializationFormat::Hex,
        )
        .unwrap();
        assert_eq!(original, loaded);
    }

    #[test]
    fn test_file_io_roundtrip() {
        let original = create_test_matrix();
        let temp_file = std::env::temp_dir().join("test_matrix.gf2");

        original.save_to_file(&temp_file).unwrap();
        let loaded = BitMatrix::load_from_file(&temp_file).unwrap();

        assert_eq!(original, loaded);

        // Cleanup
        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_file_io_text_format() {
        let original = create_test_matrix();
        let temp_file = std::env::temp_dir().join("test_matrix.txt");

        original
            .save_to_file_with_format(&temp_file, super::super::SerializationFormat::Text)
            .unwrap();
        let loaded = BitMatrix::load_from_file_with_format(
            &temp_file,
            super::super::SerializationFormat::Text,
        )
        .unwrap();

        assert_eq!(original, loaded);

        // Cleanup
        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_invalid_type_tag() {
        let bv = crate::BitVec::from_bytes_le(&[0xFF]);
        let mut buffer = Vec::new();
        bv.write_to(&mut buffer).unwrap();

        // Try to load as matrix (should fail)
        let result = BitMatrix::read_from(&mut buffer.as_slice());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IoError::InvalidData(_)));
    }

    #[test]
    fn test_metadata_validation() {
        // Create invalid metadata
        let mut buffer = Vec::new();

        let metadata = r#"{"type":"NotAMatrix","rows":10,"cols":20,"version":1}"#;
        let metadata_bytes = metadata.as_bytes();

        let header = Header::new(TypeTag::BitMatrix, metadata_bytes.len() as u32, 0);
        header.write_to(&mut buffer).unwrap();
        buffer.extend_from_slice(metadata_bytes);

        let result = BitMatrix::read_from(&mut buffer.as_slice());
        assert!(result.is_err());
    }

    // Property-based tests with proptest
    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn roundtrip_binary_format(rows in 0..20usize, cols in 0..100usize) {
                let original = BitMatrix::zeros(rows, cols);
                let mut buffer = Vec::new();

                original.write_to(&mut buffer)?;
                let loaded = BitMatrix::read_from(&mut buffer.as_slice())?;

                prop_assert_eq!(original.rows(), loaded.rows());
                prop_assert_eq!(original.cols(), loaded.cols());
                prop_assert_eq!(original, loaded);
            }

            #[test]
            fn roundtrip_text_format(rows in 0..10usize, cols in 0..50usize) {
                let original = BitMatrix::zeros(rows, cols);
                let mut buffer = Vec::new();

                original.write_to_with_format(&mut buffer, super::super::super::SerializationFormat::Text)?;
                let loaded = BitMatrix::read_from_with_format(&mut buffer.as_slice(), super::super::super::SerializationFormat::Text)?;

                prop_assert_eq!(original, loaded);
            }

            #[test]
            fn roundtrip_hex_format(rows in 0..10usize, cols in 0..50usize) {
                let original = BitMatrix::zeros(rows, cols);
                let mut buffer = Vec::new();

                original.write_to_with_format(&mut buffer, super::super::super::SerializationFormat::Hex)?;
                let loaded = BitMatrix::read_from_with_format(&mut buffer.as_slice(), super::super::super::SerializationFormat::Hex)?;

                prop_assert_eq!(original, loaded);
            }

            #[test]
            fn roundtrip_with_identity(n in 1..20usize) {
                let original = BitMatrix::identity(n);
                let mut buffer = Vec::new();

                original.write_to(&mut buffer)?;
                let loaded = BitMatrix::read_from(&mut buffer.as_slice())?;

                prop_assert_eq!(original, loaded);
            }
        }
    }
}
