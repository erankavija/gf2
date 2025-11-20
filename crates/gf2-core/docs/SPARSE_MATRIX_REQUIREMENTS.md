# Sparse Matrix Requirements for gf2-coding

## Duplicate Edge Handling in SpBitMatrixDual::from_coo

### Requirement

`SpBitMatrixDual::from_coo(m, n, edges)` must handle duplicate edges correctly 
for DVB-T2 LDPC matrix construction.

### Expected Behavior

Given edges list containing duplicates:
```rust
let edges = vec![
    (0, 1),
    (0, 1), // Duplicate
    (1, 2),
];
let matrix = SpBitMatrixDual::from_coo(2, 3, &edges);
```

**Expected**: Matrix has `matrix[0, 1] = 1` (set once, duplicates ignored or XOR'd)

**DVB-T2 Context**: The dual-diagonal parity structure may create duplicate 
edges when combined with information bit connections. The matrix constructor 
must either:
1. **Deduplicate edges during construction** (PREFERRED), OR
2. Apply XOR semantics (duplicate edges cancel: 1 ⊕ 1 = 0)

### Rationale

For LDPC parity-check matrices, duplicate edges in the COO list are typically
construction artifacts rather than intentional. In DVB-T2:
- Information bit connections come from table expansion
- Parity structure adds dual-diagonal edges
- These may accidentally overlap at same (check, variable) position

Deduplication ensures the final matrix represents the intended structure.

### Current Status

**IMPLEMENTED** (2025-11-20): Deduplication support added to gf2-core.

The implementation provides two distinct methods:
- `from_coo()`: XOR semantics (duplicates cancel, even count → 0)
- `from_coo_deduplicated()`: Deduplication semantics (duplicates ignored, first wins)

Both methods are available for `SpBitMatrix` and `SpBitMatrixDual`.

### Implementation

Two methods are now available:

```rust
// XOR semantics (duplicates cancel)
let xor_matrix = SpBitMatrixDual::from_coo(2, 3, &edges);

// Deduplication semantics (duplicates ignored)
let dedup_matrix = SpBitMatrixDual::from_coo_deduplicated(2, 3, &edges);
```

### Test Coverage

Comprehensive tests added:
- **Unit tests**: Basic functionality, edge cases (empty, all duplicates, mixed)
- **Integration tests**: DVB-T2 realistic scenario with parity structure
- **Property tests**: Unique count verification, idempotence, CSR/CSC consistency
- **Comparison tests**: XOR vs deduplication semantics

All tests passing: `cargo test sparse` (28 tests across 3 test files)

### Priority

**COMPLETED** - Feature implemented and tested.

### Recommendation

**Deduplication preferred** over XOR for LDPC applications. Duplicate edges in 
parity-check matrix are typically construction artifacts, not intentional.

Most sparse matrix libraries (scipy, Eigen) deduplicate COO input by default.

### Impact

**Positive impact on gf2-coding**:
- DVB-T2 LDPC implementation can now use `from_coo_deduplicated()` directly
- No need for manual deduplication in `build_dvb_edges()`
- Clear API distinction between XOR and deduplication semantics
- Both CSR and dual (CSR+CSC) formats support deduplication

### References

- DVB-T2 LDPC construction: `gf2-coding/src/ldpc/dvb_t2/builder.rs`
- Standard EN 302 755 defines parity structure that may create overlaps
