//! Serialization format types and detection.

/// Supported serialization formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerializationFormat {
    /// Binary format with header (default, most efficient)
    #[default]
    Binary,

    /// Human-readable ASCII bit strings: "0110101..."
    Text,

    /// Hexadecimal encoding: "1A2B3C..."
    Hex,
}

impl SerializationFormat {
    /// Detect format from file header/content
    pub fn detect(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        // Check for binary format magic bytes (need at least 8 bytes)
        if bytes.len() >= 8 && &bytes[0..8] == super::MAGIC_BYTES {
            return Some(SerializationFormat::Binary);
        }

        // For short files that are all ASCII text, assume text format
        // This handles cases like "0 0\n" (empty matrix) or "3\n" (short BitVec)
        if bytes.len() < 8 {
            let all_text = bytes.iter().all(|&b| {
                b.is_ascii_digit() || b == b'\n' || b == b'\r' || b == b' ' || b == b'\t'
            });
            if all_text {
                return Some(SerializationFormat::Text);
            }
            return None;
        }

        // Check for hex format FIRST (before text format)
        // Hex format: check if second line has long hex strings (16+ chars)
        // This distinguishes hex (16 chars per word) from text (1 char per bit)
        if let Some(first_newline) = bytes.iter().position(|&b| b == b'\n') {
            if first_newline + 1 < bytes.len() {
                let second_line_start = first_newline + 1;
                let second_line_end = bytes[second_line_start..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map(|pos| second_line_start + pos)
                    .unwrap_or(bytes.len());
                let second_line = &bytes[second_line_start..second_line_end];

                // If has hex letters A-F, definitely hex format (not text)
                let has_hex_letter = second_line
                    .iter()
                    .any(|&b| matches!(b, b'A'..=b'F' | b'a'..=b'f'));
                if has_hex_letter {
                    return Some(SerializationFormat::Hex);
                }

                // If second line is all hex chars and length is a multiple of 16, it's hex format
                // BUT: if it's only 0s and 1s, it's text format (ambiguous case)
                if second_line.len() >= 16 && second_line.len() % 16 == 0 {
                    let all_hex = second_line.iter().all(|&b| b.is_ascii_hexdigit());
                    let only_binary = second_line.iter().all(|&b| b == b'0' || b == b'1');
                    if all_hex && !only_binary {
                        return Some(SerializationFormat::Hex);
                    }
                }
            }
        }

        // For text format: check if mostly 0/1 chars (at least 70%)
        let text_chars: usize = bytes
            .iter()
            .take(100)
            .filter(|&&b| b == b'0' || b == b'1')
            .count();
        let total_chars = bytes.len().min(100);
        if text_chars > 0 && text_chars * 100 >= total_chars * 70 {
            let all_valid = bytes.iter().take(100).all(|&b| {
                b == b'0'
                    || b == b'1'
                    || b == b'\n'
                    || b == b'\r'
                    || b == b' '
                    || b == b'\t'
                    || b.is_ascii_digit()
            });
            if all_valid {
                return Some(SerializationFormat::Text);
            }
        }

        // Fallback: if starts with digits and newline, likely text format (handles dimension headers)
        if bytes[0].is_ascii_digit() {
            let first_line_end = bytes
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or(bytes.len());
            let first_line = &bytes[0..first_line_end];
            let all_dimension_chars = first_line
                .iter()
                .all(|&b| b.is_ascii_digit() || b == b' ' || b == b'\t');
            if all_dimension_chars {
                return Some(SerializationFormat::Text);
            }
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
        assert_eq!(
            SerializationFormat::detect(data),
            Some(SerializationFormat::Binary)
        );
    }

    #[test]
    fn test_detect_text() {
        let data = b"0110101001\n0101010101";
        assert_eq!(
            SerializationFormat::detect(data),
            Some(SerializationFormat::Text)
        );
    }

    #[test]
    fn test_detect_hex() {
        let data = b"1A2B3C4D\nABCDEF01";
        assert_eq!(
            SerializationFormat::detect(data),
            Some(SerializationFormat::Hex)
        );
    }

    #[test]
    fn test_detect_short_text() {
        let data = b"01";
        // Short files with digits are now detected as text format
        assert_eq!(
            SerializationFormat::detect(data),
            Some(SerializationFormat::Text)
        );
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
