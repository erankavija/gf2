# Required Features for gf2-core

This document tracks features needed by dependent crates that are currently missing from gf2-core.

## Matrix-Vector Operations for Dense BitMatrix

**Required by**: gf2-coding (LDPC encoding)

**Missing operations**:

### 1. `BitMatrix::matvec_transpose()`

Compute `y = A^T × x` for dense matrix A and bit vector x.

```rust
impl BitMatrix {
    /// Compute matrix-vector product with transpose: y = A^T × x
    ///
    /// For an m×n matrix A, computes the product of A^T (n×m) with vector x (m bits).
    /// Returns vector y of length n.
    ///
    /// # Arguments
    ///
    /// * `x` - Input bit vector of length m (must equal self.rows())
    ///
    /// # Returns
    ///
    /// Output bit vector of length n (equals self.cols())
    ///
    /// # Panics
    ///
    /// Panics if x.len() != self.rows()
    ///
    /// # Performance
    ///
    /// Should use word-level operations and SIMD where possible.
    /// For dense matrices (>30% density), this is faster than sparse operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitMatrix, BitVec};
    ///
    /// let a = BitMatrix::from_rows(&[
    ///     BitVec::from_bytes_le(&[0b101]),  // row 0
    ///     BitVec::from_bytes_le(&[0b110]),  // row 1
    /// ]);
    /// let x = BitVec::from_bytes_le(&[0b11]); // [1, 1]
    /// let y = a.matvec_transpose(&x); // A^T is 3×2, x is 2 bits -> y is 3 bits
    /// ```
    pub fn matvec_transpose(&self, x: &BitVec) -> BitVec {
        todo!()
    }
}
```

### 2. `BitMatrix::matvec()`

Compute `y = A × x` for dense matrix A and bit vector x.

```rust
impl BitMatrix {
    /// Compute matrix-vector product: y = A × x
    ///
    /// For an m×n matrix A, computes the product with vector x (n bits).
    /// Returns vector y of length m.
    ///
    /// # Arguments
    ///
    /// * `x` - Input bit vector of length n (must equal self.cols())
    ///
    /// # Returns
    ///
    /// Output bit vector of length m (equals self.rows())
    ///
    /// # Panics
    ///
    /// Panics if x.len() != self.cols()
    ///
    /// # Performance
    ///
    /// Should use word-level operations and SIMD where possible.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitMatrix, BitVec};
    ///
    /// let a = BitMatrix::from_rows(&[
    ///     BitVec::from_bytes_le(&[0b101]),  // row 0
    ///     BitVec::from_bytes_le(&[0b110]),  // row 1
    /// ]);
    /// let x = BitVec::from_bytes_le(&[0b111]); // [1, 1, 1]
    /// let y = a.matvec(&x); // 2 bits output
    /// ```
    pub fn matvec(&self, x: &BitVec) -> BitVec {
        todo!()
    }
}
```

## Priority

**HIGH** - Required for gf2-coding to complete migration from sparse to dense storage for LDPC parity matrices.

## Context

DVB-T2 LDPC parity matrices are 40-50% dense. Sparse storage uses 30× more space than dense (1.2 GB vs 40 MB for 6 Short frames). Dense matrix-vector operations with SIMD will likely be faster than sparse for this density level.

## Implementation Notes

- Use word-level operations (64-bit words)
- SIMD acceleration where possible (AVX2/AVX-512)
- See existing `matvec_transpose()` in `SpBitMatrixDual` for reference
- For transpose operation, iterate columns of A (rows of A^T) for better cache locality
