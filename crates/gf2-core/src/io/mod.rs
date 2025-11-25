//! Binary serialization for GF(2) data structures.
//!
//! Provides efficient file I/O for `BitVec`, `BitMatrix`, and sparse matrices
//! with a versioned binary format.
//!
//! # File Format Specification
//!
//! All files begin with a 32-byte fixed header followed by JSON metadata and binary payload.
//!
//! ## Header Layout (32 bytes)
//!
//! ```text
//! Offset | Size | Field         | Description
//! -------|------|---------------|----------------------------------
//! 0x00   | 8    | Magic         | "GF2DATA\0" (ASCII + null)
//! 0x08   | 2    | Version       | u16 little-endian, currently 1
//! 0x0A   | 1    | Type          | 1=BitVec, 2=BitMatrix, 3=SpBitMatrix, 4=SpBitMatrixDual
//! 0x0B   | 1    | Flags         | Bit 0: compression, Bit 1: checksum
//! 0x0C   | 8    | Reserved      | Must be zeros (future extensions)
//! 0x14   | 4    | Metadata len  | u32 little-endian, JSON byte count
//! 0x18   | 8    | Data len      | u64 little-endian, payload bytes
//! ```
//!
//! ## Variable-Length Section
//!
//! ```text
//! Offset              | Content
//! --------------------|------------------------------------------
//! 0x20                | JSON metadata (UTF-8, length from header)
//! 0x20 + metadata_len | Binary payload (type-specific format)
//! [end - 32]          | BLAKE3 checksum (32 bytes, if Flags bit 1 set)
//! ```
//!
//! ## Type-Specific Payloads
//!
//! ### BitVec (Type=1)
//!
//! **Metadata JSON:**
//! ```json
//! {
//!   "type": "BitVec",
//!   "len_bits": 64800,
//!   "version": 1
//! }
//! ```
//!
//! **Payload:** `[u64; ceil(len_bits / 64)]` in little-endian, tail-masked.
//!
//! ### BitMatrix (Type=2)
//!
//! **Metadata JSON:**
//! ```json
//! {
//!   "type": "BitMatrix",
//!   "rows": 32400,
//!   "cols": 64800,
//!   "version": 1
//! }
//! ```
//!
//! **Payload:** Row-major, each row `ceil(cols / 64)` words, all u64 little-endian.
//!
//! ### SpBitMatrix (Type=3)
//!
//! **Metadata JSON:**
//! ```json
//! {
//!   "type": "SpBitMatrix",
//!   "rows": 32400,
//!   "cols": 64800,
//!   "nnz": 194400,
//!   "format": "coo",
//!   "version": 1
//! }
//! ```
//!
//! **Payload:** `[(u32, u32); nnz]` as (row, col) pairs, sorted by row then col, all u32 little-endian.
//!
//! ### SpBitMatrixDual (Type=4)
//!
//! **Metadata JSON:**
//! ```json
//! {
//!   "type": "SpBitMatrixDual",
//!   "rows": 32400,
//!   "cols": 64800,
//!   "nnz": 194400,
//!   "version": 1
//! }
//! ```
//!
//! **Payload:** CSR + CSC indices:
//! - Row offsets: `[u32; rows + 1]`
//! - Row data: `[u32; nnz]` (column indices)
//! - Col offsets: `[u32; cols + 1]`
//! - Col data: `[u32; nnz]` (row indices)
//!
//! All u32 little-endian.
//!
//! ## Flags
//!
//! - **Bit 0 (0x01)**: Compression enabled (payload is compressed, not implemented yet)
//! - **Bit 1 (0x02)**: Checksum present (32-byte BLAKE3 hash at file end, not implemented yet)
//! - **Bits 2-7**: Reserved (must be 0)
//!
//! ## Endianness
//!
//! All multi-byte integers are **little-endian** for broad platform compatibility.

mod bitvec;
mod error;
mod format;
mod formats;
mod header;
mod matrix;
mod sparse;

pub use error::{IoError, Result};
pub use format::{Flags, Header, TypeTag, FORMAT_VERSION, HEADER_SIZE, MAGIC_BYTES};
pub use formats::SerializationFormat;
