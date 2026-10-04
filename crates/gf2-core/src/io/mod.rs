//! Serialization of `BitVec`, `BitMatrix` and the sparse GF(2) matrices in the
//! versioned binary GF2DATA format and in text and hex forms, and the
//! field-generic [`field_matrix`] format, which has its own layout.
//!
//! # GF2DATA layout
//!
//! A file is a 32-byte header, JSON metadata, then the payload. All integers
//! are little-endian.
//!
//! ```text
//! Offset | Size | Field
//! -------|------|-----------------------------------------------------------
//! 0x00   | 8    | Magic "GF2DATA\0"
//! 0x08   | 2    | Version (u16)
//! 0x0A   | 1    | Type: 1=BitVec, 2=BitMatrix, 3=SpBitMatrix, 4=SpBitMatrixDual
//! 0x0B   | 1    | Flags: bit 0 compression, bit 1 checksum
//! 0x0C   | 8    | Reserved: written as zeros, ignored on read
//! 0x14   | 4    | Metadata length in bytes (u32)
//! 0x18   | 8    | Payload length in bytes (u64)
//! ```
//!
//! The readers and writers of this module neither set nor interpret the flags.
//!
//! Metadata keys and payload by type:
//!
//! - `BitVec` (`type`, `len_bits`, `version`): `ceil(len_bits / 64)` `u64`
//!   words.
//! - `BitMatrix` (`type`, `rows`, `cols`, `version`): row-major,
//!   `ceil(cols / 64)` `u64` words per row.
//! - `SpBitMatrix` (`type`, `rows`, `cols`, `nnz`, `format`, `version`): `nnz`
//!   `(row, col)` pairs of `u32` in row order.
//! - `SpBitMatrixDual` (`type`, `rows`, `cols`, `nnz`, `version`): `u32`
//!   arrays of row offsets (`rows + 1`), column indices (`nnz`), column
//!   offsets (`cols + 1`) and row indices (`nnz`).

mod bitvec;
mod error;
pub mod field_matrix;
mod format;
mod formats;
mod header;
mod matrix;
mod sparse;

pub use error::{IoError, Result};
pub use field_matrix::{FIELD_MATRIX_FORMAT_VERSION, FIELD_MATRIX_HEADER_SIZE, FIELD_MATRIX_MAGIC};
pub use format::{Flags, Header, TypeTag, FORMAT_VERSION, HEADER_SIZE, MAGIC_BYTES};
pub use formats::SerializationFormat;
