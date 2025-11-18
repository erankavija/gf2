# Helper Methods That Could Belong in gf2-core

**Date**: 2025-11-18  
**Context**: During BCH generator matrix implementation, we identified reusable conversion helpers

---

## 🔄 BitVec ↔ Gf2mPoly Conversions

### Current Status: Implemented in Multiple Places

These conversions appear in both `BchEncoder` and `BchDecoder` as private helper methods:

#### 1. `bitvec_to_poly()` - BitVec → Gf2mPoly

**Current Implementation** (appears 3 times in `bch.rs`):
```rust
// In BchCode (line ~305)
fn encode_systematic(&self, message: &BitVec) -> BitVec {
    let m_coeffs: Vec<Gf2mElement> = (0..message.len())
        .map(|i| {
            if message.get(i) {
                self.field.one()
            } else {
                self.field.zero()
            }
        })
        .collect();
    let m = Gf2mPoly::new(m_coeffs);
    // ...
}

// In BchEncoder (line ~408)
fn bitvec_to_poly(&self, bits: &BitVec) -> Gf2mPoly {
    let coeffs: Vec<Gf2mElement> = (0..bits.len())
        .map(|i| {
            if bits.get(i) {
                self.code.field.one()
            } else {
                self.code.field.zero()
            }
        })
        .collect();
    Gf2mPoly::new(coeffs)
}

// In BchDecoder (line ~549)
fn bitvec_to_poly(&self, bits: &BitVec) -> Gf2mPoly {
    let coeffs: Vec<Gf2mElement> = (0..bits.len())
        .map(|i| {
            if bits.get(i) {
                self.code.field.one()
            } else {
                self.code.field.zero()
            }
        })
        .collect();
    Gf2mPoly::new(coeffs)
}
```

**Proposed Core API**:
```rust
// In gf2_core::gf2m::Gf2mPoly or new gf2_core::conversions module

impl Gf2mPoly {
    /// Constructs a polynomial from a BitVec over GF(2^m).
    ///
    /// Each bit in the BitVec is interpreted as a coefficient in GF(2^m):
    /// - `false` (0) → field.zero()
    /// - `true` (1) → field.one()
    ///
    /// The polynomial is in ascending degree order: bit 0 is x^0 coefficient.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut bits = BitVec::new();
    /// bits.push_bit(true);  // x^0 term
    /// bits.push_bit(false); // x^1 term
    /// bits.push_bit(true);  // x^2 term
    ///
    /// let poly = Gf2mPoly::from_bitvec(&bits, &field);
    /// // Represents: 1 + x^2 over GF(2^4)
    /// ```
    pub fn from_bitvec(bits: &BitVec, field: &Gf2mField) -> Self {
        let coeffs: Vec<Gf2mElement> = (0..bits.len())
            .map(|i| {
                if bits.get(i) {
                    field.one()
                } else {
                    field.zero()
                }
            })
            .collect();
        
        Self::new(coeffs)
    }
}
```

**Usage Impact**:
- Eliminates code duplication (3 copies → 1 core implementation)
- More discoverable API for users
- Natural place for the conversion (polynomial type owns conversion)

---

#### 2. `poly_to_bitvec()` - Gf2mPoly → BitVec

**Current Implementation** (appears 2 times in `bch.rs`):
```rust
// In BchCode (line ~350)
fn encode_systematic(&self, message: &BitVec) -> BitVec {
    // ... compute codeword_poly ...
    
    let mut codeword = BitVec::new();
    for i in 0..self.n {
        let coeff = codeword_poly.coeff(i);
        codeword.push_bit(coeff.is_one());
    }
    codeword
}

// In BchEncoder (line ~425)
fn poly_to_bitvec(&self, poly: &Gf2mPoly, len: usize) -> BitVec {
    let mut bits = BitVec::new();
    
    for i in 0..len {
        let coeff = poly.coeff(i);
        bits.push_bit(coeff.is_one());
    }
    
    bits
}
```

**Proposed Core API**:
```rust
// In gf2_core::gf2m::Gf2mPoly

impl Gf2mPoly {
    /// Converts polynomial to BitVec, extracting binary coefficients.
    ///
    /// Only works for polynomials where all coefficients are binary (0 or 1).
    /// Non-binary coefficients are treated as 1.
    ///
    /// # Arguments
    ///
    /// * `len` - Desired length of output BitVec (may be > polynomial degree)
    ///
    /// # Returns
    ///
    /// BitVec where bit i = true iff coefficient of x^i is non-zero
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = /* ... polynomial 1 + x^2 ... */;
    ///
    /// let bits = poly.to_bitvec(5);
    /// assert_eq!(bits.len(), 5);
    /// assert!(bits.get(0)); // x^0 term present
    /// assert!(!bits.get(1)); // x^1 term absent
    /// assert!(bits.get(2)); // x^2 term present
    /// ```
    ///
    /// # Notes
    ///
    /// Coefficients beyond the polynomial degree are treated as zero.
    /// This is useful for BCH and other coding applications where
    /// codewords have fixed length.
    pub fn to_bitvec(&self, len: usize) -> BitVec {
        let mut bits = BitVec::new();
        
        for i in 0..len {
            let coeff = self.coeff(i);
            bits.push_bit(coeff.is_one());
        }
        
        bits
    }
    
    /// Converts polynomial to BitVec with minimal length (degree + 1).
    ///
    /// Convenience method that calls `to_bitvec(degree + 1)`.
    pub fn to_bitvec_minimal(&self) -> BitVec {
        let len = self.degree().map(|d| d + 1).unwrap_or(0);
        self.to_bitvec(len)
    }
}
```

**Usage Impact**:
- Natural method on polynomial type
- Eliminates duplication (2 copies → 1 core implementation)
- Clear API for coding theory applications

---

## 📐 Matrix-Related Helpers (Lower Priority)

### BitMatrix Row/Column Extraction

**Current Usage** (in BCH test code):
```rust
// Extracting row as BitVec manually
for i in 0..code.k() {
    let mut row = BitVec::new();
    for j in 0..code.n() {
        row.push_bit(g.get(i, j));
    }
    // use row...
}

// Extracting column as BitVec manually
for j in 0..self.n {
    codeword.push_bit(codeword_matrix.get(0, j));
}
```

**Proposed Core API** (if not already exists):
```rust
// In gf2_core::matrix::BitMatrix

impl BitMatrix {
    /// Extracts a row as a BitVec.
    ///
    /// # Panics
    ///
    /// Panics if row >= self.rows()
    pub fn row_as_bitvec(&self, row: usize) -> BitVec {
        assert!(row < self.rows(), "Row index out of bounds");
        
        let mut bits = BitVec::new();
        for j in 0..self.cols() {
            bits.push_bit(self.get(row, j));
        }
        bits
    }
    
    /// Extracts a column as a BitVec.
    ///
    /// # Panics
    ///
    /// Panics if col >= self.cols()
    pub fn col_as_bitvec(&self, col: usize) -> BitVec {
        assert!(col < self.cols(), "Column index out of bounds");
        
        let mut bits = BitVec::new();
        for i in 0..self.rows() {
            bits.push_bit(self.get(i, col));
        }
        bits
    }
    
    /// Creates a 1×k matrix from a BitVec.
    ///
    /// Convenience constructor for row vectors.
    pub fn from_row_bitvec(bits: &BitVec) -> Self {
        let mut matrix = Self::zeros(1, bits.len());
        for j in 0..bits.len() {
            matrix.set(0, j, bits.get(j));
        }
        matrix
    }
}
```

**Usage Impact**:
- Natural API for matrix operations
- Common operation in linear algebra code
- May already exist - should check core first!

---

## 🎯 Recommendation

### High Priority (Should Move to Core)

1. **`Gf2mPoly::from_bitvec()`** ✅
   - Used 3 times in BCH code
   - Fundamental operation for coding theory
   - Natural place: `gf2_core::gf2m::Gf2mPoly`

2. **`Gf2mPoly::to_bitvec()`** ✅
   - Used 2 times in BCH code
   - Natural companion to `from_bitvec()`
   - Natural place: `gf2_core::gf2m::Gf2mPoly`

### Medium Priority (Check if Already Exists)

3. **`BitMatrix::row_as_bitvec()` / `col_as_bitvec()`**
   - Check if already in `gf2-core`
   - If not, add for convenience
   - Natural place: `gf2_core::matrix::BitMatrix`

4. **`BitMatrix::from_row_bitvec()`**
   - Convenience constructor
   - Check if similar functionality exists

---

## 📋 Action Items for Next Session

### Before Starting LDPC Implementation

1. **Check gf2-core for Existing Methods**:
   ```bash
   cd /path/to/gf2-core
   grep -r "row_as_bitvec\|from_bitvec\|to_bitvec" src/
   ```

2. **If Methods Don't Exist, Create Issue/PR for gf2-core**:
   - Add `Gf2mPoly::from_bitvec()` and `to_bitvec()`
   - Add `BitMatrix::row_as_bitvec()` etc. if missing
   - Write tests for new methods
   - Update BCH code to use core methods

3. **Benefits**:
   - Cleaner BCH code
   - Reusable for LDPC and other codes
   - Better API discoverability
   - Single source of truth for conversions

---

## 💡 Implementation Notes

### For gf2-core Contributors

**Module Structure**:
```
gf2-core/
├── src/
│   ├── gf2m/
│   │   ├── poly.rs         # Add from_bitvec, to_bitvec here
│   │   └── ...
│   └── matrix.rs            # Add row_as_bitvec etc. here
```

**Test Coverage Needed**:
- Round-trip: `poly -> bitvec -> poly` equals original
- Round-trip: `bitvec -> poly -> bitvec` equals original
- Edge cases: empty bitvec, zero polynomial
- Large inputs: 1000+ bit vectors
- Matrix extraction: verify row/column values

**Performance Considerations**:
- Both conversions are O(n) - optimal
- Could add unchecked variants for hot paths if needed
- BitVec uses word-level storage, efficient iteration

---

## 🔗 Related

- BCH implementation: `gf2-coding/src/bch.rs`
- Current usage: Lines 305, 408, 425, 549
- LDPC will likely need similar conversions for systematic form

**Last Updated**: 2025-11-18
