# Sparse Matrix Deduplication - Design Decision Document

**Status**: ✅ Implemented

This document captures the design decision and implementation of duplicate edge 
handling in sparse matrix COO construction. It serves as historical context for 
the API design choices.

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

## Implementation Decision

**IMPLEMENTED**: Dual API approach chosen.

Rather than choosing between XOR or deduplication, both are provided:

1. **`from_coo()`**: XOR semantics (duplicates cancel, even count → 0)
   - Mathematically correct for GF(2) operations
   - Backward compatible with existing code
   
2. **`from_coo_deduplicated()`**: Deduplication semantics (first occurrence wins)
   - Appropriate for LDPC construction where duplicates are artifacts
   - Matches scipy/Eigen default behavior

Both methods available for `SpBitMatrix` and `SpBitMatrixDual`.

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

## Design Rationale

### Why Two Methods?

1. **XOR semantics (`from_coo()`)**: 
   - Mathematically principled for GF(2) algebra
   - Useful for applications where duplicates have meaning
   - Existing behavior preserved for compatibility

2. **Deduplication (`from_coo_deduplicated()`)**: 
   - Practical for LDPC/construction use cases
   - Matches industry standard libraries (scipy, Eigen)
   - Explicit naming makes intent clear

### Usage Guidance

- **Use `from_coo_deduplicated()`** for LDPC matrices where duplicates are 
  construction artifacts (e.g., DVB-T2, 5G LDPC)
- **Use `from_coo()`** for general GF(2) operations where XOR semantics are desired

### Impact

**Positive impact on gf2-coding**:
- DVB-T2 LDPC implementation can now use `from_coo_deduplicated()` directly
- No need for manual deduplication in `build_dvb_edges()`
- Clear API distinction between XOR and deduplication semantics
- Both CSR and dual (CSR+CSC) formats support deduplication

### References

- DVB-T2 LDPC construction: `gf2-coding/src/ldpc/dvb_t2/builder.rs`
- Standard EN 302 755 defines parity structure that may create overlaps
