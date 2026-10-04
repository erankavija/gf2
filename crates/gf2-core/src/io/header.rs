//! Reading and writing the GF2DATA fixed header.

use super::{error::*, format::*};
use std::io::{Read, Write};

impl Header {
    /// Writes the 32-byte header.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(MAGIC_BYTES)?;
        writer.write_all(&self.version.to_le_bytes())?;
        writer.write_all(&[self.type_tag as u8])?;
        writer.write_all(&[self.flags.to_u8()])?;
        writer.write_all(&[0u8; 8])?;
        writer.write_all(&self.metadata_len.to_le_bytes())?;
        writer.write_all(&self.data_len.to_le_bytes())?;

        Ok(())
    }

    /// Reads and validates the 32-byte header.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC_BYTES {
            return Err(IoError::InvalidMagic);
        }
        let mut version_bytes = [0u8; 2];
        reader.read_exact(&mut version_bytes)?;
        let version = u16::from_le_bytes(version_bytes);

        if version != FORMAT_VERSION {
            return Err(IoError::UnsupportedVersion(version));
        }
        let mut type_byte = [0u8; 1];
        reader.read_exact(&mut type_byte)?;
        let type_tag = TypeTag::from_u8(type_byte[0]).ok_or(IoError::UnknownType(type_byte[0]))?;
        let mut flags_byte = [0u8; 1];
        reader.read_exact(&mut flags_byte)?;
        let flags = Flags::from_u8(flags_byte[0]);
        let mut reserved = [0u8; 8];
        reader.read_exact(&mut reserved)?;
        // Reserved bytes are not validated.
        let mut metadata_len_bytes = [0u8; 4];
        reader.read_exact(&mut metadata_len_bytes)?;
        let metadata_len = u32::from_le_bytes(metadata_len_bytes);
        let mut data_len_bytes = [0u8; 8];
        reader.read_exact(&mut data_len_bytes)?;
        let data_len = u64::from_le_bytes(data_len_bytes);

        Ok(Self {
            version,
            type_tag,
            flags,
            metadata_len,
            data_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_header_write_read_roundtrip() {
        let original = Header::new(TypeTag::BitVec, 100, 8000);

        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();

        assert_eq!(buffer.len(), HEADER_SIZE);

        let mut cursor = Cursor::new(buffer);
        let restored = Header::read_from(&mut cursor).unwrap();

        assert_eq!(original.version, restored.version);
        assert_eq!(original.type_tag, restored.type_tag);
        assert_eq!(original.metadata_len, restored.metadata_len);
        assert_eq!(original.data_len, restored.data_len);
    }

    #[test]
    fn test_header_write_starts_with_magic() {
        let header = Header::new(TypeTag::BitMatrix, 50, 4000);

        let mut buffer = Vec::new();
        header.write_to(&mut buffer).unwrap();

        assert_eq!(&buffer[0..8], MAGIC_BYTES);
    }

    #[test]
    fn test_header_read_invalid_magic() {
        let mut buffer = vec![0u8; HEADER_SIZE];
        buffer[0..8].copy_from_slice(b"BADMAGIC");

        let mut cursor = Cursor::new(buffer);
        let result = Header::read_from(&mut cursor);

        assert!(matches!(result, Err(IoError::InvalidMagic)));
    }

    #[test]
    fn test_header_read_unsupported_version() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(MAGIC_BYTES);
        buffer.extend_from_slice(&99u16.to_le_bytes());
        buffer.resize(HEADER_SIZE, 0);

        let mut cursor = Cursor::new(buffer);
        let result = Header::read_from(&mut cursor);

        assert!(matches!(result, Err(IoError::UnsupportedVersion(99))));
    }

    #[test]
    fn test_header_read_unknown_type() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(MAGIC_BYTES);
        buffer.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        buffer.push(99);
        buffer.resize(HEADER_SIZE, 0);

        let mut cursor = Cursor::new(buffer);
        let result = Header::read_from(&mut cursor);

        assert!(matches!(result, Err(IoError::UnknownType(99))));
    }

    #[test]
    fn test_header_with_flags() {
        let mut header = Header::new(TypeTag::SpBitMatrix, 200, 16000);
        header.flags = Flags::new().with_compression().with_checksum();

        let mut buffer = Vec::new();
        header.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let restored = Header::read_from(&mut cursor).unwrap();

        assert!(restored.flags.has_compression());
        assert!(restored.flags.has_checksum());
    }

    #[test]
    fn test_header_all_type_tags() {
        for type_tag in [
            TypeTag::BitVec,
            TypeTag::BitMatrix,
            TypeTag::SpBitMatrix,
            TypeTag::SpBitMatrixDual,
        ] {
            let header = Header::new(type_tag, 10, 100);

            let mut buffer = Vec::new();
            header.write_to(&mut buffer).unwrap();

            let mut cursor = Cursor::new(buffer);
            let restored = Header::read_from(&mut cursor).unwrap();

            assert_eq!(restored.type_tag, type_tag);
        }
    }

    #[test]
    fn test_header_exact_32_bytes() {
        let header = Header::new(TypeTag::BitVec, 0, 0);
        let mut buffer = Vec::new();
        header.write_to(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 32);
        assert_eq!(buffer.len(), HEADER_SIZE);
    }
}
