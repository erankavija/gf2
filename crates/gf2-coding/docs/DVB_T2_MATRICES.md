# Adding DVB-T2 LDPC Base Matrices

This document explains how to add the actual DVB-T2 LDPC base matrices from ETSI EN 302 755.

## Current Status

✅ **Structure complete** - All 12 configurations have placeholder matrices  
⏳ **Data needed** - Actual base matrices from ETSI EN 302 755 Tables 6a-6f and 7a-7f

## File Structure

```
src/ldpc/dvb_t2/
├── mod.rs                - Factory methods (dvb_t2_normal, dvb_t2_long)
└── dvb_t2_matrices.rs    - Const array tables (12 configurations)
```

## Matrix Format

Each base matrix uses:
- **`-1`**: Empty position (no circulant submatrix)
- **`0..359`**: Circulant shift amount (for Z=360)

Example structure:
```rust
pub const SHORT_RATE_1_2: &[[i16; 45]] = &[
    [  0,   1,   2,  -1,  -1, ...],  // Row 0: check equation 0
    [ -1,  10,  -1,   5,  -1, ...],  // Row 1: check equation 1
    // ... more rows
];
```

## How to Add Real Matrices

### 1. Obtain ETSI EN 302 755 Standard

The official DVB-T2 base matrices are in:
- **ETSI EN 302 755 V1.4.1** (2015-07) or later
- Tables 6a-6f (short frames, n=16200)
- Tables 7a-7f (normal frames, n=64800)

### 2. Matrix Dimensions

| Frame  | Rate | Base Rows | Base Cols | Expanded n |
|--------|------|-----------|-----------|------------|
| Short  | 1/2  | ~23       | 45        | 16200      |
| Short  | 3/5  | ~18       | 45        | 16200      |
| Short  | 2/3  | ~15       | 45        | 16200      |
| Short  | 3/4  | ~11       | 45        | 16200      |
| Short  | 4/5  | ~9        | 45        | 16200      |
| Short  | 5/6  | ~8        | 45        | 16200      |
| Normal | 1/2  | ~90       | 180       | 64800      |
| Normal | 3/5  | ~72       | 180       | 64800      |
| Normal | 2/3  | ~60       | 180       | 64800      |
| Normal | 3/4  | ~45       | 180       | 64800      |
| Normal | 4/5  | ~36       | 180       | 64800      |
| Normal | 5/6  | ~30       | 180       | 64800      |

### 3. Steps to Replace Placeholders

For each configuration (e.g., SHORT_RATE_1_2):

1. **Count dimensions** in the standard table
2. **Update constant declaration** with correct row count:
   ```rust
   pub const SHORT_RATE_1_2: &[[i16; 45]] = &[
       // Actual number of rows from Table 6a
   ```

3. **Transcribe each row** from the standard:
   ```rust
   [  0,   1,   2, -1, ..., -1],  // Copy shift values, -1 for empty
   ```

4. **Verify** dimensions match:
   - rows × expansion_factor = number of parity checks
   - cols × expansion_factor = n (16200 or 64800)

### 4. Validation

After adding matrices, run:

```bash
# Test basic construction
cargo test dvb_t2

# Test with examples
cargo run --example qc_ldpc_demo
```

Expected validations:
- `n = base_cols × 360` (16200 for normal, 64800 for long)
- All-zeros codeword is valid
- Code rate approximately matches (k/n ≈ rate)

### 5. Reference Implementations

For verification, compare with:
- GNU Radio `gr-dvbt2` module
- `ldpc_tool` from DVB standards
- Academic papers with full tables

## Example: Adding Rate 1/2 Normal Frame

```rust
// File: src/ldpc/dvb_t2/dvb_t2_matrices.rs

/// DVB-T2 short frame (n=16200, Z=360) rate 1/2 base matrix.
/// From ETSI EN 302 755 Table 6a.
/// Dimensions: 23 rows × 45 columns
pub const SHORT_RATE_1_2: &[[i16; 45]] = &[
    // Row 0 (from Table 6a)
    [  0,   1,   2,   0,   3,   1, ..., -1, -1],
    
    // Row 1
    [  1,   2,   3,   1,   4,   2, ..., -1, -1],
    
    // ... rows 2-21 ...
    
    // Row 22 (last row)
    [ -1,  -1,  -1,  20, -1, ...,   0,   1],
];
```

## Current Placeholders

All 12 configurations currently use simplified placeholder matrices:
- They have correct column dimensions
- They have minimal row structure (often just 1 row)
- They will **not** achieve correct error correction performance
- They are **only** for structural testing

## Priority Order

Implement in this order for DVB-T2 simulation:

1. **SHORT_RATE_1_2** - Most common, used for testing
2. **SHORT_RATE_3_4** - Higher throughput mode
3. **NORMAL_RATE_1_2** - Best performance mode
4. Other rates as needed

## Integration with Tests

Once real matrices are added:

1. Update `tests/dvb_t2_ldpc_tests.rs` with known answer tests
2. Add FER simulation in `examples/`
3. Compare performance curves with DVB-T2 standard plots

## References

- ETSI EN 302 755: DVB-T2 Frame structure specification
- IEEE 802.11n: Similar QC-LDPC structure (for comparison)
- Richardson & Urbanke: "Modern Coding Theory" (LDPC design principles)
