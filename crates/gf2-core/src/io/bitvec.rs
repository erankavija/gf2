//! BitVec serialization and deserialization.

use super::{error::*, format::*};
use crate::BitVec;
use std::io::{Read, Write};

/// Metadata for BitVec serialization
#[derive(Debug)]
#[cfg_attr(feature = "io", derive(serde::Serialize, serde::Deserialize))]
struct BitVecMetadata {
    #[serde(rename = "type")]
    type_name: String,
    len_bits: usize,
    version: u32,
}

impl BitVec {
    /// Save BitVec to a file with specified format
    pub fn save_to_file_with_format<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        format: super::SerializationFormat,
    ) -> Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write_to_with_format(&mut writer, format)
    }

    /// Save BitVec to a file (binary format)
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.save_to_file_with_format(path, super::SerializationFormat::Binary)
    }

    /// Load BitVec from a file with auto-detection
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_from_with_auto_detect(&mut reader)
    }

    /// Write BitVec to a writer with specified format
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

    /// Write BitVec to a writer (binary format)
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.write_binary(writer)
    }

    /// Write BitVec in binary format
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Create metadata
        let metadata = BitVecMetadata {
            type_name: "BitVec".to_string(),
            len_bits: self.len(),
            version: 1,
        };
        
        let metadata_json = serde_json::to_vec(&metadata)
            .map_err(|e| IoError::InvalidData(format!("Failed to serialize metadata: {}", e)))?;
        
        // Calculate data length (words as bytes)
        let data_len = self.words().len() * 8;
        
        // Write header
        let header = Header::new(
            TypeTag::BitVec,
            metadata_json.len() as u32,
            data_len as u64,
        );
        header.write_to(writer)?;
        
        // Write metadata
        writer.write_all(&metadata_json)?;
        
        // Write data (words in little-endian)
        for &word in self.words() {
            writer.write_all(&word.to_le_bytes())?;
        }
        
        Ok(())
    }
    
    /// Write BitVec in text format (human-readable)
    fn write_text<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Write length as first line
        writeln!(writer, "{}", self.len())?;
        
        // Write bits as ASCII '0' and '1'
        const CHARS_PER_LINE: usize = 80;
        for chunk_start in (0..self.len()).step_by(CHARS_PER_LINE) {
            let chunk_end = (chunk_start + CHARS_PER_LINE).min(self.len());
            for i in chunk_start..chunk_end {
                writer.write_all(if self.get(i) { b"1" } else { b"0" })?;
            }
            writer.write_all(b"\n")?;
        }
        
        Ok(())
    }

    /// Write BitVec in hexadecimal format
    fn write_hex<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Write length as first line
        writeln!(writer, "{}", self.len())?;
        
        // Write words as hex
        for &word in self.words() {
            writeln!(writer, "{:016X}", word)?;
        }
        
        Ok(())
    }

    /// Read BitVec from a reader with auto-detection
    fn read_from_with_auto_detect<R: Read>(reader: &mut R) -> Result<Self> {
        // Read all content into a buffer for format detection
        let mut content = Vec::new();
        reader.read_to_end(&mut content)?;
        
        // Detect format
        let format = super::SerializationFormat::detect(&content)
            .ok_or_else(|| IoError::InvalidData("Unable to detect file format".to_string()))?;
        
        // Create cursor from buffer
        let mut cursor = std::io::Cursor::new(content);
        
        match format {
            super::SerializationFormat::Binary => Self::read_binary(&mut cursor),
            super::SerializationFormat::Text => Self::read_text(&mut cursor),
            super::SerializationFormat::Hex => Self::read_hex(&mut cursor),
        }
    }

    /// Read BitVec from a reader (binary format)
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        Self::read_binary(reader)
    }

    /// Read BitVec in binary format
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self> {
        // Read header
        let header = Header::read_from(reader)?;
        
        // Validate type
        if header.type_tag != TypeTag::BitVec {
            return Err(IoError::InvalidData(format!(
                "Expected BitVec type, got {:?}",
                header.type_tag
            )));
        }
        
        // Read metadata
        let mut metadata_bytes = vec![0u8; header.metadata_len as usize];
        reader.read_exact(&mut metadata_bytes)?;
        
        let metadata: BitVecMetadata = serde_json::from_slice(&metadata_bytes)
            .map_err(|e| IoError::InvalidData(format!("Failed to parse metadata: {}", e)))?;
        
        // Validate metadata
        if metadata.type_name != "BitVec" {
            return Err(IoError::InvalidData(format!(
                "Metadata type mismatch: expected 'BitVec', got '{}'",
                metadata.type_name
            )));
        }
        
        // Calculate expected word count
        let num_words = (metadata.len_bits + 63) / 64;
        let expected_data_len = num_words * 8;
        
        if header.data_len as usize != expected_data_len {
            return Err(IoError::InvalidData(format!(
                "Data length mismatch: expected {} bytes, got {}",
                expected_data_len, header.data_len
            )));
        }
        
        // Read words
        let mut words = Vec::with_capacity(num_words);
        for _ in 0..num_words {
            let mut word_bytes = [0u8; 8];
            reader.read_exact(&mut word_bytes)?;
            words.push(u64::from_le_bytes(word_bytes));
        }
        
        // Construct BitVec directly from words
        let bv = BitVec::from_words(words, metadata.len_bits);
        
        Ok(bv)
    }

    /// Read BitVec in text format
    fn read_text<R: Read>(reader: &mut R) -> Result<Self> {
        use std::io::BufRead;
        
        let mut buf_reader = std::io::BufReader::new(reader);
        let mut first_line = String::new();
        buf_reader.read_line(&mut first_line)?;
        
        // Parse length from first line
        let len_bits = first_line.trim().parse::<usize>()
            .map_err(|e| IoError::InvalidData(format!("Invalid length: {}", e)))?;
        
        // Read bits
        let mut bv = BitVec::new();
        let mut line = String::new();
        while buf_reader.read_line(&mut line)? > 0 {
            for c in line.trim().chars() {
                match c {
                    '0' => bv.push_bit(false),
                    '1' => bv.push_bit(true),
                    _ => return Err(IoError::InvalidData(format!("Invalid character: {}", c))),
                }
            }
            line.clear();
        }
        
        if bv.len() != len_bits {
            return Err(IoError::InvalidData(format!(
                "Length mismatch: expected {}, got {}",
                len_bits, bv.len()
            )));
        }
        
        Ok(bv)
    }

    /// Read BitVec in hexadecimal format
    fn read_hex<R: Read>(reader: &mut R) -> Result<Self> {
        use std::io::BufRead;
        
        let mut buf_reader = std::io::BufReader::new(reader);
        let mut first_line = String::new();
        buf_reader.read_line(&mut first_line)?;
        
        // Parse length from first line
        let len_bits = first_line.trim().parse::<usize>()
            .map_err(|e| IoError::InvalidData(format!("Invalid length: {}", e)))?;
        
        // Read hex words
        let mut words = Vec::new();
        let mut line = String::new();
        while buf_reader.read_line(&mut line)? > 0 {
            let hex_str = line.trim();
            if !hex_str.is_empty() {
                let word = u64::from_str_radix(hex_str, 16)
                    .map_err(|e| IoError::InvalidData(format!("Invalid hex: {}", e)))?;
                words.push(word);
            }
            line.clear();
        }
        
        Ok(BitVec::from_words(words, len_bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_bitvec_write_read_roundtrip_empty() {
        let original = BitVec::new();
        
        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_from(&mut cursor).unwrap();
        
        assert_eq!(original.len(), restored.len());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_write_read_roundtrip_single_bit() {
        let mut original = BitVec::new();
        original.push_bit(true);
        
        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_from(&mut cursor).unwrap();
        
        assert_eq!(original.len(), restored.len());
        assert_eq!(original.get(0), restored.get(0));
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_write_read_roundtrip_one_word() {
        let mut original = BitVec::new();
        for i in 0..64 {
            original.push_bit(i % 3 == 0);
        }
        
        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_from(&mut cursor).unwrap();
        
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_write_read_roundtrip_multiple_words() {
        let mut original = BitVec::new();
        for i in 0..200 {
            original.push_bit(i % 2 == 0);
        }
        
        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_from(&mut cursor).unwrap();
        
        assert_eq!(original.len(), restored.len());
        for i in 0..original.len() {
            assert_eq!(original.get(i), restored.get(i), "Bit {} mismatch", i);
        }
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_write_read_roundtrip_not_word_aligned() {
        let mut original = BitVec::new();
        for i in 0..100 {
            original.push_bit(i % 5 == 0);
        }
        
        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_from(&mut cursor).unwrap();
        
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_write_has_correct_header() {
        let mut bv = BitVec::new();
        for _ in 0..100 {
            bv.push_bit(true);
        }
        
        let mut buffer = Vec::new();
        bv.write_to(&mut buffer).unwrap();
        
        // Check magic bytes
        assert_eq!(&buffer[0..8], MAGIC_BYTES);
        
        // Check version
        let version = u16::from_le_bytes([buffer[8], buffer[9]]);
        assert_eq!(version, FORMAT_VERSION);
        
        // Check type tag
        assert_eq!(buffer[10], TypeTag::BitVec as u8);
    }

    #[test]
    fn test_bitvec_read_wrong_type() {
        // Create a header with wrong type
        let header = Header::new(TypeTag::BitMatrix, 50, 100);
        
        let mut buffer = Vec::new();
        header.write_to(&mut buffer).unwrap();
        buffer.resize(buffer.len() + 150, 0);
        
        let mut cursor = Cursor::new(buffer);
        let result = BitVec::read_from(&mut cursor);
        
        assert!(matches!(result, Err(IoError::InvalidData(_))));
    }

    #[test]
    fn test_bitvec_preserves_tail_masking() {
        // Create BitVec with non-word-aligned length
        let mut original = BitVec::new();
        for i in 0..100 {
            original.push_bit(i < 50);
        }
        
        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_from(&mut cursor).unwrap();
        
        // Verify tail masking is preserved
        assert_eq!(original.words(), restored.words());
    }

    #[test]
    fn test_bitvec_large() {
        let mut original = BitVec::zeros(10000);
        for i in (0..10000).step_by(7) {
            original.set(i, true);
        }
        
        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_from(&mut cursor).unwrap();
        
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_file_roundtrip() {
        let mut original = BitVec::zeros(500);
        for i in (0..500).step_by(3) {
            original.set(i, true);
        }
        
        let temp_file = std::env::temp_dir().join("test_bitvec.gf2");
        original.save_to_file(&temp_file).unwrap();
        
        let restored = BitVec::load_from_file(&temp_file).unwrap();
        assert_eq!(original, restored);
        
        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_bitvec_text_format() {
        let mut original = BitVec::new();
        for i in 0..100 {
            original.push_bit(i % 3 == 0);
        }
        
        let mut buffer = Vec::new();
        original.write_to_with_format(&mut buffer, super::super::SerializationFormat::Text).unwrap();
        
        // Verify text format
        let text = String::from_utf8(buffer.clone()).unwrap();
        assert!(text.starts_with("100\n")); // Length
        assert!(text.contains('0'));
        assert!(text.contains('1'));
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_text(&mut cursor).unwrap();
        
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_hex_format() {
        let mut original = BitVec::new();
        for i in 0..128 {
            original.push_bit(i % 2 == 0);
        }
        
        let mut buffer = Vec::new();
        original.write_to_with_format(&mut buffer, super::super::SerializationFormat::Hex).unwrap();
        
        // Verify hex format
        let text = String::from_utf8(buffer.clone()).unwrap();
        assert!(text.starts_with("128\n")); // Length
        assert!(text.chars().any(|c| c.is_ascii_hexdigit()));
        
        let mut cursor = Cursor::new(buffer);
        let restored = BitVec::read_hex(&mut cursor).unwrap();
        
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bitvec_format_auto_detect() {
        let original = BitVec::from_bytes_le(&[0xAA, 0x55]);
        
        // Test binary format detection
        let mut binary_buf = Vec::new();
        original.write_to_with_format(&mut binary_buf, super::super::SerializationFormat::Binary).unwrap();
        let mut cursor = Cursor::new(binary_buf);
        let restored_binary = BitVec::read_from_with_auto_detect(&mut cursor).unwrap();
        assert_eq!(original, restored_binary);
        
        // Test text format detection
        let mut text_buf = Vec::new();
        original.write_to_with_format(&mut text_buf, super::super::SerializationFormat::Text).unwrap();
        let mut cursor = Cursor::new(text_buf);
        let restored_text = BitVec::read_from_with_auto_detect(&mut cursor).unwrap();
        assert_eq!(original, restored_text);
        
        // Test hex format detection
        let mut hex_buf = Vec::new();
        original.write_to_with_format(&mut hex_buf, super::super::SerializationFormat::Hex).unwrap();
        let mut cursor = Cursor::new(hex_buf);
        let restored_hex = BitVec::read_from_with_auto_detect(&mut cursor).unwrap();
        assert_eq!(original, restored_hex);
    }

    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_bitvec_roundtrip(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
                let original = BitVec::from_bytes_le(&bytes);
                
                let mut buffer = Vec::new();
                original.write_to(&mut buffer).unwrap();
                
                let mut cursor = Cursor::new(buffer);
                let restored = BitVec::read_from(&mut cursor).unwrap();
                
                prop_assert_eq!(original.len(), restored.len());
                prop_assert_eq!(original, restored);
            }

            #[test]
            fn prop_bitvec_preserves_all_bits(
                len in 0usize..1000,
                seed in any::<u64>()
            ) {
                let mut original = BitVec::zeros(len);
                for i in 0..len {
                    if (i as u64).wrapping_mul(seed) % 7 == 0 {
                        original.set(i, true);
                    }
                }
                
                let mut buffer = Vec::new();
                original.write_to(&mut buffer).unwrap();
                
                let mut cursor = Cursor::new(buffer);
                let restored = BitVec::read_from(&mut cursor).unwrap();
                
                prop_assert_eq!(original.len(), restored.len());
                for i in 0..len {
                    prop_assert_eq!(
                        original.get(i),
                        restored.get(i),
                        "Bit {} mismatch", i
                    );
                }
            }
        }
    }
}
