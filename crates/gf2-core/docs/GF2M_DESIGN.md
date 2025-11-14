# GF(2^m) Extension Field Arithmetic - Design Document

**Status**: In Progress - Phase 1 Complete (2024-11-14)  
**Priority**: HIGH - Blocks DVB-T2 FEC simulation in gf2-coding  
**Estimated Effort**: 2-3 weeks (Phase 1 complete, ~10-16 days remaining)

## Overview

Implements arithmetic over binary extension fields GF(2^m) to support BCH codes and other algebraic error-correcting codes used in DVB-T2, DVB-S2, and similar standards.

## Motivation

DVB-T2 uses BCH outer codes operating over GF(2^m) for error floor reduction. Current `gf2-core` only supports base field GF(2). This phase adds:
- Field element representation and arithmetic
- Polynomial operations over GF(2^m)
- Efficient algorithms for BCH encoding/decoding primitives

## Architecture

### Module Structure

**New module: `gf2-core/src/gf2m.rs`**

```rust
pub struct Gf2mField {
    m: usize,                    // Extension degree
    primitive_poly: u64,         // Primitive polynomial for reduction
    log_table: Option<Vec<u16>>, // Discrete log table
    exp_table: Option<Vec<u16>>, // Antilog table
}

pub struct Gf2mElement {
    value: u64,        // Polynomial representation (for m ≤ 64)
    field: &Gf2mField, // Reference to field parameters
}

pub struct Gf2mPoly {
    coeffs: Vec<Gf2mElement>, // Coefficients (degree 0 to n)
}
```

## Field Arithmetic

### Addition/Subtraction
- XOR of polynomial representations: `a + b = a ⊕ b`
- Identity: `a + 0 = a`
- Inverse: `a + a = 0` (self-inverse)

### Multiplication Strategies

**Small fields (m ≤ 8)**: Direct shift-and-add
```rust
fn multiply_naive(a: u8, b: u8, prim_poly: u16) -> u8 {
    let mut result = 0;
    let mut temp_a = a;
    for i in 0..8 {
        if (b >> i) & 1 == 1 {
            result ^= temp_a;
        }
        let carry = temp_a & 0x80;
        temp_a <<= 1;
        if carry != 0 {
            temp_a ^= prim_poly as u8;
        }
    }
    result
}
```

**Medium fields (8 < m ≤ 16)**: Log/antilog tables
- Precompute: `log_table[α^i] = i` for i = 0..2^m-1
- Multiply: `a * b = exp[log[a] + log[b] mod (2^m - 1)]`
- Memory: ~128 KB for GF(2^16)
- Speedup: O(1) vs. O(m) for shift-and-add

**Large fields (m > 16)**: Schoolbook with optimizations
- Use when table size becomes prohibitive
- Optimize for sparse polynomials
- Consider CLMUL acceleration (future)

### Division
- Compute via multiplicative inverse: `a / b = a * b^(-1)`
- Inverse via Extended Euclidean algorithm or table lookup

### Exponentiation
- Square-and-multiply: `a^k` in O(log k) multiplications
- Useful for syndrome computation in BCH

## Polynomial Operations

### Gf2mPoly Arithmetic

**Addition**: Coefficient-wise XOR (extends to different degrees)

**Multiplication**: 
- Schoolbook for small degree: O(n²)
- Karatsuba for larger: O(n^1.58)
- Result degree: deg(a) + deg(b)

**Division with Remainder**:
```rust
// Returns (quotient, remainder) such that dividend = quotient * divisor + remainder
fn divide(dividend: &Gf2mPoly, divisor: &Gf2mPoly) 
    -> (Gf2mPoly, Gf2mPoly)
```

**GCD**: Euclidean algorithm
- Essential for minimal polynomial computation
- Used in Berlekamp-Massey algorithm

## BCH-Specific Operations

### Generator Polynomial Construction

For BCH(n, k, t) code correcting t errors:
1. Choose primitive element α in GF(2^m)
2. Find minimal polynomials of α, α², ..., α^(2t)
3. Generator g(x) = LCM of minimal polynomials

```rust
impl Gf2mField {
    pub fn bch_generator(&self, n: usize, t: usize) -> Gf2mPoly {
        // Construct from consecutive roots
    }
}
```

### Syndrome Computation

Evaluate received polynomial r(x) at syndrome roots:
```
S_i = r(α^i) for i = 1, 2, ..., 2t
```

Optimized batch evaluation for all syndrome values.

### Chien Search

Efficiently find roots of error locator polynomial σ(x):
1. Evaluate σ(α^i) for i = 0, 1, ..., n-1
2. Roots indicate error positions
3. Incremental evaluation: reuse previous computation

```rust
impl Gf2mPoly {
    pub fn chien_search(&self, field: &Gf2mField) -> Vec<usize> {
        // Returns error positions
    }
}
```

## Implementation Strategy

### Phase 1: Core Field Arithmetic ✅ COMPLETE (2024-11-14)
- ✅ `Gf2mField` with primitive polynomial
- ✅ `Gf2mElement` with add/multiply (division deferred to Phase 2)
- ✅ Standard field presets (GF(2^8), GF(2^16))
- ✅ Unit tests for field axioms (19 tests)
- ✅ **Educational example** with comprehensive mathematical introduction
  - Introduction to finite field theory (fields, extension fields, primitive polynomials)
  - Polynomial representation of field elements
  - Step-by-step arithmetic examples with both mathematical and code notation
  - Practical application: computing in GF(2^4) with worked examples
  - Rust doc format suitable for `cargo doc` and educational reference

**Key Implementation Details**:
- **File**: `src/gf2m.rs` (529 lines)
- **Architecture**: Used `Rc<FieldParams>` for safe field parameter sharing (no unsafe code)
- **Operators**: Both reference `&T` and owned `T` implementations
- **Multiplication**: Schoolbook algorithm with modular reduction
- **Tests**: 19 unit tests + 6 doc tests covering field axioms and worked examples
- **Status**: All 158 total tests passing, zero warnings

**Next Steps**: Proceed to Phase 2 for efficient multiplication with log/antilog tables

### Phase 2: Efficient Multiplication (Next - 3-5 days)
- [ ] **Division operation** via multiplicative inverse
  - Extended Euclidean algorithm for inverse computation
  - Division operator implementation
  - Tests for a/b = a * b^(-1)
- [ ] Log/antilog table generation
  - Compute exp[i] = α^i for i = 0..2^m-1 (α is generator)
  - Compute log[α^i] = i (inverse mapping)
  - Special handling for zero element
- [ ] Table-based multiplication for m ≤ 16
  - O(1) multiply: `a * b = exp[(log[a] + log[b]) mod (2^m - 1)]`
  - Fallback to schoolbook for m > 16
  - Memory usage: ~128 KB for GF(2^16)
- [ ] Lazy table initialization (on-demand generation)
- [ ] Benchmarks vs. schoolbook multiplication
  - Target: 10x speedup for m ≥ 8
  - Criterion benchmarks for GF(2^8), GF(2^16)
- [ ] Property tests for arithmetic
  - Use `proptest` for randomized testing
  - Verify table-based matches schoolbook results
  - Test edge cases (zero, one, all field elements)

**Starting Point**: Current schoolbook implementation in `src/gf2m.rs::Mul::mul()`

### Phase 3: Polynomial Operations (Week 2)
- [ ] `Gf2mPoly` type
- [ ] Polynomial addition/multiplication
- [ ] Division with remainder
- [ ] GCD algorithm
- [ ] Tests and benchmarks

### Phase 4: BCH Primitives (Week 2-3)
- [ ] Minimal polynomial computation
- [ ] Generator polynomial construction
- [ ] Chien search implementation
- [ ] Integration tests with known BCH codes
- [ ] Documentation and examples

## Educational Example

### Mathematical Introduction (Rust Doc Format)

The implementation will include a comprehensive educational example demonstrating GF(2^m) arithmetic with mathematical context. This will be included as module-level documentation in `gf2m.rs`.

**Content structure**:
1. **Finite Field Fundamentals**
   - What is a field? (additive/multiplicative groups, axioms)
   - Why binary extension fields? (GF(2) as base field)
   - Construction via irreducible polynomials

2. **Polynomial Representation**
   - Elements as polynomials over GF(2): `a₃x³ + a₂x² + a₁x + a₀`
   - Binary coefficient vectors: `(a₃, a₂, a₁, a₀)` → integer representation
   - Example: In GF(2^4), element `x³ + x + 1` → binary `1011` → decimal `11`

3. **Arithmetic Operations with Examples**
   - **Addition**: XOR of coefficients
     - `(x² + 1) + (x³ + x²) = x³ + 1` 
     - Binary: `0101 ⊕ 1100 = 1001`
   - **Multiplication**: Polynomial multiply then reduce modulo primitive polynomial
     - Example in GF(2^4) with primitive polynomial `x⁴ + x + 1`
     - `(x² + 1) * (x + 1) = x³ + x² + x + 1`
     - Step-by-step reduction demonstration
   - **Division**: Multiplication by inverse (computed via Extended Euclidean)

4. **Worked Example: Computing in GF(2^4)**
   - Use primitive polynomial `p(x) = x⁴ + x + 1` (binary `10011`)
   - Demonstrate all 15 non-zero elements as powers of primitive element α
   - Show addition table excerpt
   - Show multiplication via table lookup (foreshadowing Phase 2)
   - Code examples matching mathematical notation

5. **Code Integration**
   ```rust
   /// # Example: Computing in GF(2^4)
   ///
   /// Let's work through arithmetic in GF(16) using primitive polynomial
   /// p(x) = x⁴ + x + 1.
   ///
   /// ```
   /// use gf2_core::gf2m::Gf2mField;
   ///
   /// // Create GF(2^4) with primitive polynomial x^4 + x + 1
   /// let field = Gf2mField::new(4, 0b10011);
   ///
   /// // Elements represented as polynomials over GF(2)
   /// // x² + 1 is binary 0101 = 5
   /// let a = field.element(0b0101);
   /// // x³ + x is binary 1010 = 10  
   /// let b = field.element(0b1010);
   ///
   /// // Addition is XOR: (x² + 1) + (x³ + x) = x³ + x² + x + 1
   /// let sum = a + b;  // 0101 ⊕ 1010 = 1111
   /// assert_eq!(sum.value(), 0b1111);
   ///
   /// // Multiplication with reduction modulo p(x)
   /// let product = a * b;
   /// // ... (show result and reduction steps in comments)
   /// ```
   ```

**Location**: Module-level documentation in `src/gf2m.rs`, accessible via `cargo doc`

**Benefits**:
- Lowers barrier to entry for users unfamiliar with finite field theory
- Demonstrates correct usage patterns
- Serves as executable documentation (doc tests)
- Aligns with Rust community documentation standards
- Provides mathematical foundation for understanding BCH/RS codes

## Testing Strategy

### Unit Tests
- Field axioms: (a + b) + c = a + (b + c), etc.
- Identity elements: a + 0 = a, a * 1 = a
- Inverses: a + (-a) = 0, a * a^(-1) = 1
- Polynomial division: p = q * d + r

### Property Tests (proptest)
```rust
proptest! {
    #[test]
    fn multiply_commutative(a: u8, b: u8) {
        let field = Gf2mField::gf256();
        let ea = field.element(a);
        let eb = field.element(b);
        assert_eq!(ea * eb, eb * ea);
    }
    
    #[test]
    fn division_roundtrip(a: u8, b in 1u8..=255) {
        let field = Gf2mField::gf256();
        let ea = field.element(a);
        let eb = field.element(b);
        assert_eq!((ea * eb) / eb, ea);
    }
}
```

### Edge Cases
- Zero element: 0 + a = a, 0 * a = 0
- Multiplicative inverse of all non-zero elements exists
- Primitive polynomial generates full multiplicative group (order 2^m - 1)

### Known Answer Tests
- GF(2^8) with standard primitive polynomial x^8 + x^4 + x^3 + x + 1
- Verify against reference implementations
- Test BCH generator polynomials from standards

## Performance Targets

### Field Multiplication
- GF(2^8) table-based: < 5 cycles/op
- GF(2^16) table-based: < 10 cycles/op
- 10x faster than naive for m ≥ 8

### Polynomial Operations
- Multiplication: O(n²) for degree n < 100
- Division: Comparable to multiplication
- GCD: O(n²) worst case

### BCH Primitives
- Syndrome computation: O(t) field operations
- Chien search: O(n) incremental evaluations
- Target: > 1 Gbps equivalent throughput for DVB-T2 use case

## Integration with gf2-core

### BitVec Integration
- Use `BitVec` for large m (m > 64)
- Consistent memory representation

### Future CLMUL Integration
- Phase 7 (GF(2) polynomials) introduces CLMUL
- Can accelerate GF(2^m) multiplication via carry-less multiply
- Defer to Phase 7+ for SIMD optimization

### API Consistency
- Follow gf2-core conventions (functional style at API level)
- Clear documentation with mathematical notation
- Comprehensive examples

## Standard Field Presets

```rust
impl Gf2mField {
    /// GF(2^8) with primitive polynomial x^8 + x^4 + x^3 + x + 1
    pub fn gf256() -> Self;
    
    /// GF(2^16) with primitive polynomial x^16 + x^12 + x^3 + x + 1
    pub fn gf65536() -> Self;
    
    /// Custom field with specified primitive polynomial
    pub fn new(m: usize, primitive_poly: u64) -> Self;
}
```

## Dependencies

**Internal**: None (foundational layer in gf2-core)

**External**: 
- Standard library only
- Optional: `proptest` for property-based testing (dev dependency)

**Blocks**: 
- gf2-coding Phase C9 (BCH codes)
- gf2-coding Phase C10 (DVB-T2 FEC simulation)

## References

- *Error Control Coding* by Lin & Costello (BCH theory)
- ETSI EN 302 755 (DVB-T2 standard, BCH parameters)
- *Finite Fields for Computer Scientists and Engineers* by McEliece
- Handbook of Applied Cryptography (Chapter 2: Finite Fields)

## Open Questions

1. **Table size vs. performance**: For which m should we switch from tables to computation? → **Decision: m ≤ 16 use tables, m > 16 use optimized schoolbook**
2. **Memory allocation**: Pre-allocate tables or lazy initialization? → **Decision: Pre-allocate for standard fields, lazy for custom**
3. **Thread safety**: Should tables be shared across threads? (Arc<> wrapper?) → **Decision: Yes, use Arc<> for shared access**
4. **API design**: Method syntax (a.multiply(b)) vs. operator overloading (a * b)? → **Decision: Operator overloading for mathematical clarity**
5. **Educational example field size**: GF(2^4) for simplicity vs. GF(2^8) for practical relevance? → **Decision GF(2^4) for hand-traceable examples, with GF(2^8) notes**

## Future Extensions

- Reed-Solomon codes (generalization of BCH)
- Finite field FFT for fast polynomial multiplication
- Normal basis representation (alternative to polynomial basis)
- Optimal normal basis for hardware efficiency
- Composite field arithmetic (GF((2^m)^k))
