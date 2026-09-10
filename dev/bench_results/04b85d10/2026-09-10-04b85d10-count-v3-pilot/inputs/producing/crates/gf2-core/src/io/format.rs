//! Binary format constants and structures for GF(2) data serialization.

/// Magic bytes that identify a GF(2) data file: "GF2DATA\0"
pub const MAGIC_BYTES: &[u8; 8] = b"GF2DATA\0";

/// Current format version
pub const FORMAT_VERSION: u16 = 1;

/// Fixed header size in bytes (32 bytes for natural alignment)
/// Layout: 8 (magic) + 2 (version) + 1 (type) + 1 (flags) + 8 (reserved) + 4 (metadata_len) + 8 (data_len)
pub const HEADER_SIZE: usize = 32;

/// Type tags for different data structures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeTag {
    /// BitVec type (tag = 1)
    BitVec = 1,
    /// BitMatrix type (tag = 2)
    BitMatrix = 2,
    /// SpBitMatrix type (tag = 3)
    SpBitMatrix = 3,
    /// SpBitMatrixDual type (tag = 4)
    SpBitMatrixDual = 4,
}

impl TypeTag {
    /// Convert a u8 to a TypeTag
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(TypeTag::BitVec),
            2 => Some(TypeTag::BitMatrix),
            3 => Some(TypeTag::SpBitMatrix),
            4 => Some(TypeTag::SpBitMatrixDual),
            _ => None,
        }
    }
}

/// Flags for optional features
#[derive(Debug, Clone, Copy, Default)]
pub struct Flags {
    bits: u8,
}

impl Flags {
    /// Create new flags with no bits set
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    /// Set compression flag
    pub fn with_compression(mut self) -> Self {
        self.bits |= 0x01;
        self
    }

    /// Set checksum flag
    pub fn with_checksum(mut self) -> Self {
        self.bits |= 0x02;
        self
    }

    /// Check if compression is enabled
    pub fn has_compression(&self) -> bool {
        self.bits & 0x01 != 0
    }

    /// Check if checksum is enabled
    pub fn has_checksum(&self) -> bool {
        self.bits & 0x02 != 0
    }

    /// Convert to u8
    pub fn to_u8(&self) -> u8 {
        self.bits
    }

    /// Create from u8
    pub fn from_u8(bits: u8) -> Self {
        Self { bits }
    }
}

/// File header structure (32 bytes)
#[derive(Debug, Clone)]
pub struct Header {
    /// Format version
    pub version: u16,
    /// Data structure type
    pub type_tag: TypeTag,
    /// Optional feature flags
    pub flags: Flags,
    /// Length of JSON metadata in bytes
    pub metadata_len: u32,
    /// Length of binary payload in bytes
    pub data_len: u64,
}

impl Header {
    /// Create a new header with default flags
    pub fn new(type_tag: TypeTag, metadata_len: u32, data_len: u64) -> Self {
        Self {
            version: FORMAT_VERSION,
            type_tag,
            flags: Flags::new(),
            metadata_len,
            data_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_bytes() {
        assert_eq!(MAGIC_BYTES.len(), 8);
        assert_eq!(MAGIC_BYTES, b"GF2DATA\0");
    }

    #[test]
    fn test_header_size() {
        // 8 (magic) + 2 (version) + 1 (type) + 1 (flags) + 8 (reserved) + 4 (metadata_len) + 8 (data_len) = 32
        assert_eq!(HEADER_SIZE, 32);
    }

    #[test]
    fn test_type_tag_values() {
        assert_eq!(TypeTag::BitVec as u8, 1);
        assert_eq!(TypeTag::BitMatrix as u8, 2);
        assert_eq!(TypeTag::SpBitMatrix as u8, 3);
        assert_eq!(TypeTag::SpBitMatrixDual as u8, 4);
    }

    #[test]
    fn test_type_tag_roundtrip() {
        for tag in [
            TypeTag::BitVec,
            TypeTag::BitMatrix,
            TypeTag::SpBitMatrix,
            TypeTag::SpBitMatrixDual,
        ] {
            let byte = tag as u8;
            assert_eq!(TypeTag::from_u8(byte), Some(tag));
        }
    }

    #[test]
    fn test_type_tag_invalid() {
        assert_eq!(TypeTag::from_u8(0), None);
        assert_eq!(TypeTag::from_u8(5), None);
        assert_eq!(TypeTag::from_u8(255), None);
    }

    #[test]
    fn test_flags_default() {
        let flags = Flags::new();
        assert!(!flags.has_compression());
        assert!(!flags.has_checksum());
        assert_eq!(flags.to_u8(), 0);
    }

    #[test]
    fn test_flags_compression() {
        let flags = Flags::new().with_compression();
        assert!(flags.has_compression());
        assert!(!flags.has_checksum());
        assert_eq!(flags.to_u8(), 0x01);
    }

    #[test]
    fn test_flags_checksum() {
        let flags = Flags::new().with_checksum();
        assert!(!flags.has_compression());
        assert!(flags.has_checksum());
        assert_eq!(flags.to_u8(), 0x02);
    }

    #[test]
    fn test_flags_both() {
        let flags = Flags::new().with_compression().with_checksum();
        assert!(flags.has_compression());
        assert!(flags.has_checksum());
        assert_eq!(flags.to_u8(), 0x03);
    }

    #[test]
    fn test_flags_roundtrip() {
        let original = Flags::new().with_compression().with_checksum();
        let byte = original.to_u8();
        let restored = Flags::from_u8(byte);

        assert_eq!(original.has_compression(), restored.has_compression());
        assert_eq!(original.has_checksum(), restored.has_checksum());
    }

    #[test]
    fn test_header_creation() {
        let header = Header::new(TypeTag::BitVec, 100, 8000);

        assert_eq!(header.version, FORMAT_VERSION);
        assert_eq!(header.type_tag, TypeTag::BitVec);
        assert_eq!(header.metadata_len, 100);
        assert_eq!(header.data_len, 8000);
        assert!(!header.flags.has_compression());
        assert!(!header.flags.has_checksum());
    }
}
