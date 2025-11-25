//! Serialization format types and detection.

/// Supported serialization formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationFormat {
    /// Binary format with header (default, most efficient)
    Binary,
    
    /// Human-readable ASCII bit strings: "0110101..."
    Text,
    
    /// Hexadecimal encoding: "1A2B3C..."
    Hex,
}

impl Default for SerializationFormat {
    fn default() -> Self {
        SerializationFormat::Binary
    }
}

impl SerializationFormat {
    /// Detect format from file header/content
    pub fn detect(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 {
            return None;
        }
        
        // Check for binary format magic bytes
        if &bytes[0..8] == super::MAGIC_BYTES {
            return Some(SerializationFormat::Binary);
        }
        
        // For text format: check if mostly 0/1 chars (at least 70%)
        let text_chars: usize = bytes.iter().take(100).filter(|&&b| b == b'0' || b == b'1').count();
        let total_chars = bytes.len().min(100);
        if text_chars > 0 && text_chars * 100 >= total_chars * 70 {
            let all_valid = bytes.iter().take(100).all(|&b| {
                b == b'0' || b == b'1' || b == b'\n' || b == b'\r' || b == b' ' || b == b'\t' || b.is_ascii_digit()
            });
            if all_valid {
                return Some(SerializationFormat::Text);
            }
        }
        
        // For hex format: must have hex chars A-F (not just digits)
        let has_hex_letter = bytes.iter().take(100).any(|&b| {
            matches!(b, b'A'..=b'F' | b'a'..=b'f')
        });
        let all_hex_chars = bytes.iter().take(100).all(|&b| {
            b.is_ascii_hexdigit() || b == b'\n' || b == b'\r' || b == b' ' || b == b'\t'
        });
        if all_hex_chars && has_hex_letter {
            return Some(SerializationFormat::Hex);
        }
        
        None
    }
    
    /// File extension suggestion for this format
    pub fn extension(&self) -> &'static str {
        match self {
            SerializationFormat::Binary => "gf2",
            SerializationFormat::Text => "txt",
            SerializationFormat::Hex => "hex",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_binary() {
        assert_eq!(SerializationFormat::default(), SerializationFormat::Binary);
    }

    #[test]
    fn test_detect_binary() {
        let data = b"GF2DATA\0version data...";
        assert_eq!(SerializationFormat::detect(data), Some(SerializationFormat::Binary));
    }

    #[test]
    fn test_detect_text() {
        let data = b"0110101001\n0101010101";
        assert_eq!(SerializationFormat::detect(data), Some(SerializationFormat::Text));
    }

    #[test]
    fn test_detect_hex() {
        let data = b"1A2B3C4D\nABCDEF01";
        assert_eq!(SerializationFormat::detect(data), Some(SerializationFormat::Hex));
    }

    #[test]
    fn test_detect_too_short() {
        let data = b"01";
        assert_eq!(SerializationFormat::detect(data), None);
    }

    #[test]
    fn test_detect_unknown() {
        let data = b"random binary \x00\x01\x02\x03\x04\x05\x06\x07\x08";
        assert_eq!(SerializationFormat::detect(data), None);
    }

    #[test]
    fn test_extensions() {
        assert_eq!(SerializationFormat::Binary.extension(), "gf2");
        assert_eq!(SerializationFormat::Text.extension(), "txt");
        assert_eq!(SerializationFormat::Hex.extension(), "hex");
    }
}
