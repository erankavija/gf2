# GF(2) Data Structures File I/O Design

**Status**: Phase 1 complete (BitVec I/O), Phase 2 in progress (Matrix I/O)

## Overview

Design for efficient binary serialization/deserialization of `BitVec`, `BitMatrix`, and sparse matrices with optional compression.

## Motivation

1. **LDPC encoder preprocessing**: Generator matrices take 2-3 minutes to compute. Store them once, load in milliseconds.
2. **Test data**: Large test vectors (DVB-T2: 64,800 bits/block × 202 blocks) benefit from binary storage.
3. **Configuration persistence**: Pre-computed matrices for standard codes (DVB-T2, 5G NR).
4. **Cross-language compatibility**: Enable other tools to consume our matrices.

## Requirements

### Functional Requirements

1. **Serialize BitVec**: Efficient binary format with metadata
2. **Serialize BitMatrix**: Dense matrix storage (row-major)
3. **Serialize SpBitMatrix**: Sparse matrix in COO or CSR format
4. **Serialize SpBitMatrixDual**: Both row and column indices
5. **File format versioning**: Support format evolution
6. **Compression**: Optional zstd/gzip for large matrices
7. **Checksums**: Validate data integrity
8. **Zero-copy deserialization**: Memory-map large files where possible

### Non-Functional Requirements

1. **Performance**: Load 64K-bit DVB-T2 matrix in <10ms
2. **Compactness**: Sparse matrices use O(edges) space, not O(rows × cols)
3. **Simplicity**: Single-file format, no external dependencies (except optional compression)
4. **Safety**: Validate all inputs, return `Result<T, Error>`
5. **No `unsafe`**: Maintain `#![deny(unsafe_code)]`

## File Format Design

### Binary Format Structure

```
┌─────────────────────────────────────────────┐
│ Magic bytes (8 bytes): "GF2DATA\0"         │
├─────────────────────────────────────────────┤
│ Version (u16): format version (e.g., 1)    │
├─────────────────────────────────────────────┤
│ Type tag (u8): BitVec=1, BitMatrix=2, etc. │
├─────────────────────────────────────────────┤
│ Flags (u8): compression, checksums          │
├─────────────────────────────────────────────┤
│ Reserved (4 bytes): future use              │
├─────────────────────────────────────────────┤
│ Metadata length (u32): JSON metadata bytes  │
├─────────────────────────────────────────────┤
│ Data length (u64): payload bytes            │
├─────────────────────────────────────────────┤
│ Metadata (variable): JSON with dimensions   │
├─────────────────────────────────────────────┤
│ Data payload (variable): type-specific      │
├─────────────────────────────────────────────┤
│ Checksum (32 bytes): BLAKE3 hash (optional)│
└─────────────────────────────────────────────┘
```

**Total header**: 32 bytes (fixed) + metadata (variable)

### Type-Specific Payloads

#### 1. BitVec Payload

```
Metadata JSON:
{
  "type": "BitVec",
  "len_bits": 64800,
  "version": 1
}

Payload:
  - Words: [u64; ceil(len_bits / 64)]
  - Little-endian encoding
  - Tail masked (padding bits zero)
```

**Size**: `ceil(len_bits / 64) * 8` bytes

#### 2. BitMatrix Payload (Dense)

```
Metadata JSON:
{
  "type": "BitMatrix",
  "rows": 32400,
  "cols": 64800,
  "version": 1
}

Payload:
  - Row-major order
  - Each row: ceil(cols / 64) words
  - Words: [u64; rows * stride_words]
```

**Size**: `rows * ceil(cols / 64) * 8` bytes

#### 3. SpBitMatrix Payload (Sparse COO)

```
Metadata JSON:
{
  "type": "SpBitMatrix",
  "rows": 32400,
  "cols": 64800,
  "nnz": 194400,  // number of edges
  "format": "coo",
  "version": 1
}

Payload:
  - Edges: [(u32, u32); nnz]  // (row, col) pairs
  - Sorted by row, then col
  - Little-endian u32s
```

**Size**: `nnz * 8` bytes (huge savings for sparse matrices!)

**Example**: DVB-T2 Normal H matrix:
- Dense: 32400 × 64800 bits = ~255 MB
- Sparse: 194400 edges × 8 bytes = **1.5 MB** (170× smaller!)

#### 4. SpBitMatrixDual Payload (Dual Index)

```
Metadata JSON:
{
  "type": "SpBitMatrixDual",
  "rows": 32400,
  "cols": 64800,
  "nnz": 194400,
  "version": 1
}

Payload:
  - Row index offsets: [u32; rows + 1]
  - Row indices: [u32; nnz]
  - Col index offsets: [u32; cols + 1]
  - Col indices: [u32; nnz]
```

**Size**: `(rows + cols + 2) * 4 + nnz * 8` bytes

## API Design (gf2-core)

### Module Structure

```
gf2-core/src/io/
├── mod.rs           // Public API
├── format.rs        // Binary format constants and types
├── bitvec.rs        // BitVec serialization
├── matrix.rs        // BitMatrix serialization
├── sparse.rs        // SpBitMatrix serialization
├── error.rs         // Error types
└── compression.rs   // Optional compression (feature-gated)
```

### Public API

```rust
// gf2-core/src/io/mod.rs

/// File I/O for GF(2) data structures
pub mod io {
    use std::path::Path;
    use std::io::{Read, Write};
    
    /// Error types for I/O operations
    #[derive(Debug, thiserror::Error)]
    pub enum IoError {
        #[error("Invalid magic bytes")]
        InvalidMagic,
        #[error("Unsupported format version: {0}")]
        UnsupportedVersion(u16),
        #[error("Checksum mismatch")]
        ChecksumMismatch,
        #[error("I/O error: {0}")]
        Io(#[from] std::io::Error),
        #[error("Invalid data: {0}")]
        InvalidData(String),
    }
    
    pub type Result<T> = std::result::Result<T, IoError>;
    
    /// Serialization options
    #[derive(Debug, Clone)]
    pub struct WriteOptions {
        /// Enable compression
        pub compress: bool,
        /// Compression level (0-22 for zstd)
        pub compression_level: i32,
        /// Write checksum
        pub checksum: bool,
    }
    
    impl Default for WriteOptions {
        fn default() -> Self {
            Self {
                compress: false,
                compression_level: 3,
                checksum: true,
            }
        }
    }
    
    /// Deserialization options
    #[derive(Debug, Clone)]
    pub struct ReadOptions {
        /// Verify checksum if present
        pub verify_checksum: bool,
    }
    
    impl Default for ReadOptions {
        fn default() -> Self {
            Self {
                verify_checksum: true,
            }
        }
    }
}
```

### Multiple Format Support ✅ **IMPLEMENTED**

Three serialization formats are supported with automatic detection:

```rust
pub enum SerializationFormat {
    Binary,  // Efficient binary format with header (default)
    Text,    // Human-readable "0110101..." format
    Hex,     // Hexadecimal word encoding
}
```

**Format Detection**: Automatic based on file content
- Binary: Magic bytes "GF2DATA\0"
- Text: 70%+ characters are '0' or '1'  
- Hex: Contains hex letters A-F

### BitVec API

```rust
impl BitVec {
    // Primary API (binary format, auto-detect on load)
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()>;
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self>;
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()>;
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self>;
    
    // Format selection API
    pub fn save_to_file_with_format<P: AsRef<Path>>(
        &self,
        path: P,
        format: SerializationFormat,
    ) -> io::Result<()>;
    
    pub fn write_to_with_format<W: Write>(
        &self,
        writer: &mut W,
        format: SerializationFormat,
    ) -> io::Result<()>;
}
```

### BitMatrix API

```rust
impl BitMatrix {
    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        path: P,
        options: &WriteOptions,
    ) -> io::Result<()>;
    
    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
        options: &ReadOptions,
    ) -> io::Result<Self>;
    
    pub fn write_to<W: Write>(
        &self,
        writer: &mut W,
        options: &WriteOptions,
    ) -> io::Result<()>;
    
    pub fn read_from<R: Read>(
        reader: &mut R,
        options: &ReadOptions,
    ) -> io::Result<Self>;
}
```

### SpBitMatrix API

```rust
impl SpBitMatrix {
    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        path: P,
        options: &WriteOptions,
    ) -> io::Result<()>;
    
    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
        options: &ReadOptions,
    ) -> io::Result<Self>;
    
    pub fn write_to<W: Write>(
        &self,
        writer: &mut W,
        options: &WriteOptions,
    ) -> io::Result<()>;
    
    pub fn read_from<R: Read>(
        reader: &mut R,
        options: &ReadOptions,
    ) -> io::Result<Self>;
}
```

### SpBitMatrixDual API

Same pattern as SpBitMatrix.

## Implementation Plan

### Phase 1: Core Infrastructure ✅ **COMPLETE**

**Status**: Completed 2024-11-25

**Implemented**:
- ✅ `gf2-core/src/io/` module structure
- ✅ `IoError` type (without thiserror, using manual impl)
- ✅ Binary format with 32-byte header (not 28 as originally planned)
- ✅ Header serialization with full validation
- ✅ BitVec serialization with JSON metadata
- ✅ Optional `io` feature (enabled by default)
- ✅ File I/O: `save_to_file()` / `load_from_file()`
- ✅ Stream I/O: `write_to()` / `read_from()`

**Tests**: 47 tests (unit + property + integration + format detection), all passing

**Dependencies**: `serde` + `serde_json` (feature-gated)

**Formats Supported**:
- **Binary**: Efficient format with 32-byte header + JSON metadata + binary payload
- **Text**: Human-readable ASCII "0110101..." format with length header
- **Hex**: Hexadecimal word encoding (16 chars per word)
- **Auto-detection**: Automatic format detection on load

**Format Changes from Original Design**:
- Header is 32 bytes (not 28) for natural alignment: 8 reserved bytes instead of 4
- Removed `WriteOptions` / `ReadOptions` - keep it simple for now
- Error handling without `thiserror` dependency
- JSON metadata uses serde (not manual serialization)
- **Added multi-format support**: Binary, Text, Hex with auto-detection (10 additional tests)

### Phase 2: Matrix I/O ⏳ **NEXT**

**Goal**: Implement dense and sparse matrix I/O

**Estimated**: 2-3 hours

**Tasks**:
1. Implement `BitMatrix` serialization (row-major)
2. Implement `SpBitMatrix` serialization (COO format)
3. Implement `SpBitMatrixDual` serialization
4. Property-based tests: roundtrip verification
5. Performance tests: verify load times

**Target Deliverables**:
- All matrix types support file I/O
- Sparse format achieves 100×+ compression
- Dense matrices load efficiently

### Phase 3: Compression Support ⏸️ **DEFERRED**

**Goal**: Add optional compression for large matrices

**Note**: Deferred until core I/O is validated in production use

**Tasks**:
1. Add `compression` feature flag with `zstd` dependency
2. Implement compression in `write_to` path
3. Implement decompression in `read_from` path
4. Benchmark: compressed vs uncompressed size/speed

**Deliverables**:
- Optional `compression` feature
- 2-5× additional compression for sparse matrices
- <50ms load time with decompression

### Phase 4: Checksum Support ⏸️ **DEFERRED**

**Goal**: Data integrity verification

**Note**: Will implement custom checksum algorithm when needed

**Tasks**:
1. Add `blake3` dependency (fast cryptographic hash)
2. Compute checksum during write
3. Verify checksum during read (optional)
4. Test: detect corrupted files

**Deliverables**:
- Checksums in file format
- Corruption detection
- Negligible performance impact

### Phase 5: LDPC Integration ⏸️ **FUTURE**

**Goal**: Use file I/O for LDPC generator matrices

**Note**: Depends on Phase 2 completion (matrix serialization)

**Tasks**:
1. Add `save_to_file`/`load_from_file` to `RuEncodingMatrices`
2. Generate all DVB-T2 generator matrices once
3. Store in `data/ldpc/dvb_t2/` directory
4. Update `EncodingCache` to load from files if available
5. Fallback to computation if files missing

**Deliverables**:
- Pre-computed DVB-T2 generator matrices (12 files, ~100 MB total)
- `EncodingCache::from_directory()` method
- Encoder creation: 2 minutes → **10ms** (200× speedup!)

## File Organization

```
gf2-coding/
└── data/
    └── ldpc/
        └── dvb_t2/
            ├── short_rate_1_2.gf2mat
            ├── short_rate_3_5.gf2mat
            ├── short_rate_2_3.gf2mat
            ├── short_rate_3_4.gf2mat
            ├── short_rate_4_5.gf2mat
            ├── short_rate_5_6.gf2mat
            ├── normal_rate_1_2.gf2mat
            ├── normal_rate_3_5.gf2mat
            ├── normal_rate_2_3.gf2mat
            ├── normal_rate_3_4.gf2mat
            ├── normal_rate_4_5.gf2mat
            └── normal_rate_5_6.gf2mat
```

**Note**: These files are NOT checked into git (too large). Generated via:
```bash
cargo run --release --example generate_dvb_t2_matrices
```

## Usage Examples

### Example 1: Save/Load BitVec (Binary Format)

```rust
use gf2_core::BitVec;

// Save in binary format (default)
let bv = BitVec::random(64800);
bv.save_to_file("data.gf2")?;

// Load with auto-detection
let loaded = BitVec::load_from_file("data.gf2")?;
assert_eq!(bv, loaded);
```

### Example 2: Human-Readable Text Format

```rust
use gf2_core::{BitVec, io::SerializationFormat};

let mut bv = BitVec::new();
bv.push_bit(true);
bv.push_bit(false);
bv.push_bit(true);

// Save as text
bv.save_to_file_with_format("data.txt", SerializationFormat::Text)?;
// File contains:
// 3
// 101

// Load automatically detects format
let loaded = BitVec::load_from_file("data.txt")?;
assert_eq!(bv, loaded);
```

### Example 3: Hexadecimal Format

```rust
use gf2_core::{BitVec, io::SerializationFormat};

let bv = BitVec::from_bytes_le(&[0xAA, 0x55]);

// Save as hex
bv.save_to_file_with_format("data.hex", SerializationFormat::Hex)?;
// File contains:
// 16
// 000000000000AA55

// Auto-detection works
let loaded = BitVec::load_from_file("data.hex")?;
assert_eq!(bv, loaded);
```

### Example 4: Save/Load Sparse Matrix (Future)

```rust
use gf2_core::SpBitMatrix;

let h = SpBitMatrix::from_coo(32400, 64800, &edges);

// Save (sparse COO format)
h.save_to_file("matrix.gf2")?;
// File size: ~1.5 MB instead of 255 MB!

// Load
let loaded = SpBitMatrix::load_from_file("matrix.gf2")?;
assert_eq!(h.rows(), loaded.rows());
assert_eq!(h.cols(), loaded.cols());
assert_eq!(h.nnz(), loaded.nnz());
```

### Example 5: Compressed Generator Matrix (Future)

```rust
use gf2_core::BitMatrix;

let g = compute_generator_matrix(&h);  // Expensive!

// Save (binary format, will support compression in Phase 3)
g.save_to_file("generator.gf2")?;

// Load (fast!)
let loaded = BitMatrix::load_from_file("generator.gf2")?;
// Takes ~10ms instead of 2 minutes
```

### Example 6: LDPC Encoder with Pre-Computed Matrices (Future - Phase 5)

```rust
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::ldpc::encoding::EncodingCache;

// Initialize cache from pre-computed files
let cache = EncodingCache::from_directory("data/ldpc/dvb_t2")?;
// Loads all 12 configs in ~100ms

// Now encoder creation is instant
let encoder = LdpcEncoder::with_cache(
    LdpcCode::dvb_t2_normal(CodeRate::Rate3_5),
    &cache
);
// <10ms instead of 2 minutes!
```

## Extensibility: Adding New Formats

The design makes it straightforward to add new serialization formats:

**Steps to add a new format**:
1. Add variant to `SerializationFormat` enum
2. Implement `write_FORMAT()` method in type impl
3. Implement `read_FORMAT()` method in type impl  
4. Add detection logic to `SerializationFormat::detect()`
5. Add format-specific tests

**Example**: Adding Base64 format would require ~100 lines of code and 5 tests.

## Performance Targets

| Operation | Current | With File I/O | Improvement |
|-----------|---------|---------------|-------------|
| DVB-T2 Short preprocessing | 2-3 minutes | 5-10ms | 12,000× |
| DVB-T2 Normal preprocessing | 10-15 minutes | 10-20ms | 45,000× |
| Test suite startup | N/A | 100ms | Instant |
| Memory (all configs) | ~1.5 GB | ~200 MB | 7.5× |

## Dependencies

**Required**:
- `thiserror = "1.0"` (error handling)

**Optional** (feature-gated):
- `zstd = "0.13"` (compression, feature = "compression")
- `blake3 = "1.5"` (checksums, feature = "checksums")

**Total dependency cost**: 0 (base), 2 crates (full features)

## Risks & Mitigations

### Risk 1: File Format Evolution
**Mitigation**: Version field in header, graceful fallback to recomputation

### Risk 2: Corrupted Files
**Mitigation**: Checksums (BLAKE3), validate on load, recompute if invalid

### Risk 3: Binary Portability
**Mitigation**: Little-endian encoding, explicit sizes, JSON metadata for flexibility

### Risk 4: Large Files in Repository
**Mitigation**: `.gitignore` data files, provide generation script

## Testing Strategy

### Unit Tests
- Header read/write roundtrip
- Metadata JSON parsing
- Checksum computation/verification
- Error handling (invalid files)

### Property Tests
- `forall bv: BitVec. load(save(bv)) == bv`
- `forall m: BitMatrix. load(save(m)) == m`
- Sparse matrix roundtrip preserves edges

### Integration Tests
- Save/load DVB-T2 matrices
- Verify encoding still works with loaded matrices
- Performance: load time <10ms

### Benchmarks
- Serialize/deserialize speed vs size
- Compressed vs uncompressed trade-offs
- Memory-mapped vs buffered I/O

## Timeline Estimate

| Phase | Effort | Cumulative |
|-------|--------|------------|
| 1. Core infrastructure | 2-3 hours | 3 hours |
| 2. Matrix I/O | 2-3 hours | 6 hours |
| 3. Compression (optional) | 1-2 hours | 8 hours |
| 4. Checksums | 1 hour | 9 hours |
| 5. LDPC integration | 1-2 hours | 11 hours |
| **Total** | **9-11 hours** | **1-2 days** |

## Success Criteria

**Phase 1 (Complete)**:
- ✅ BitVec file I/O working
- ✅ Format specification documented
- ✅ Zero unsafe code
- ✅ Comprehensive test coverage (47 tests)
- ✅ Optional `io` feature
- ✅ Multiple format support (Binary, Text, Hex)
- ✅ Automatic format detection

**Phase 2 (In Progress)**:
- ⏳ All matrix types support file I/O
- ⏳ Sparse matrices store in O(edges) space
- ⏳ Roundtrip tests for all types

**Future Phases**:
- ⏸️ Optional compression/checksums (Phase 3-4)
- ⏸️ LDPC integration with pre-computed matrices (Phase 5)  

## Conclusion

File I/O for GF(2) structures solves:
1. **LDPC preprocessing bottleneck** (minutes → milliseconds)
2. **Sparse matrix storage** (100× compression)
3. **Test suite speed** (instant startup)
4. **Production deployment** (ship pre-computed matrices)

**Recommendation**: Implement phases 1-2 (core + matrices) immediately to unblock LDPC test validation. Add compression/checksums later as optimization.

Ready to proceed with implementation?
