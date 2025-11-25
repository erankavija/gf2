//! Error types for GF(2) data structure I/O operations.

use std::io;

/// Errors that can occur during serialization or deserialization.
#[derive(Debug)]
pub enum IoError {
    /// File does not start with correct magic bytes
    InvalidMagic,
    
    /// File format version is not supported
    UnsupportedVersion(u16),
    
    /// Unknown data type tag
    UnknownType(u8),
    
    /// Checksum verification failed
    ChecksumMismatch,
    
    /// Underlying I/O error
    Io(io::Error),
    
    /// Data is malformed or invalid
    InvalidData(String),
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::InvalidMagic => write!(f, "Invalid magic bytes"),
            IoError::UnsupportedVersion(v) => write!(f, "Unsupported format version: {}", v),
            IoError::UnknownType(t) => write!(f, "Unknown type tag: {}", t),
            IoError::ChecksumMismatch => write!(f, "Checksum mismatch"),
            IoError::Io(e) => write!(f, "I/O error: {}", e),
            IoError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IoError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for IoError {
    fn from(e: io::Error) -> Self {
        IoError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, IoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(IoError::InvalidMagic.to_string(), "Invalid magic bytes");
        assert_eq!(IoError::UnsupportedVersion(99).to_string(), "Unsupported format version: 99");
        assert_eq!(IoError::UnknownType(42).to_string(), "Unknown type tag: 42");
        assert_eq!(IoError::ChecksumMismatch.to_string(), "Checksum mismatch");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: IoError = io_err.into();
        
        match err {
            IoError::Io(e) => assert_eq!(e.kind(), io::ErrorKind::NotFound),
            _ => panic!("Expected IoError::Io"),
        }
    }

    #[test]
    fn test_invalid_data_with_message() {
        let err = IoError::InvalidData("dimension mismatch".to_string());
        assert!(err.to_string().contains("dimension mismatch"));
    }
}
