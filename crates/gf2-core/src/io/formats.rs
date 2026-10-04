//! Serialization format types and detection.

/// Serialization formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerializationFormat {
    /// Binary GF2DATA format.
    #[default]
    Binary,

    /// Human-readable ASCII bit strings: "0110101..."
    Text,

    /// Hexadecimal encoding: "1A2B3C..."
    Hex,
}

impl SerializationFormat {
    /// Guesses the format of `bytes` by the first rule that applies: binary
    /// by its magic; under 8 bytes, text for a non-empty run of digits and
    /// blanks and `None` otherwise; hex when the second line has a hex letter
    /// or is a positive multiple of 16 hex digits that are not all `0`/`1`;
    /// text when the first 100 bytes are digits and blanks with at least 70%
    /// `0`/`1`, or the first line starts with a digit and holds only digits,
    /// spaces and tabs. `None` otherwise.
    pub fn detect(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        if bytes.len() >= 8 && &bytes[0..8] == super::MAGIC_BYTES {
            return Some(SerializationFormat::Binary);
        }

        // Short text inputs such as "0 0\n" (empty matrix) or "3\n" (short BitVec).
        if bytes.len() < 8 {
            let all_text = bytes.iter().all(|&b| {
                b.is_ascii_digit() || b == b'\n' || b == b'\r' || b == b' ' || b == b'\t'
            });
            if all_text {
                return Some(SerializationFormat::Text);
            }
            return None;
        }

        // Hex before text: hex has 16 chars per word, text 1 char per bit.
        if let Some(first_newline) = bytes.iter().position(|&b| b == b'\n') {
            if first_newline + 1 < bytes.len() {
                let second_line_start = first_newline + 1;
                let second_line_end = bytes[second_line_start..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map(|pos| second_line_start + pos)
                    .unwrap_or(bytes.len());
                let second_line = &bytes[second_line_start..second_line_end];

                let has_hex_letter = second_line
                    .iter()
                    .any(|&b| matches!(b, b'A'..=b'F' | b'a'..=b'f'));
                if has_hex_letter {
                    return Some(SerializationFormat::Hex);
                }

                // A line of only 0s and 1s is ambiguous and resolves to text.
                if second_line.len() >= 16 && second_line.len().is_multiple_of(16) {
                    let all_hex = second_line.iter().all(|&b| b.is_ascii_hexdigit());
                    let only_binary = second_line.iter().all(|&b| b == b'0' || b == b'1');
                    if all_hex && !only_binary {
                        return Some(SerializationFormat::Hex);
                    }
                }
            }
        }

        // Text: at least 70% of the first 100 bytes are 0/1.
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

        // A first line of digits and blanks is a text dimension header.
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

    /// Suggested file extension.
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
