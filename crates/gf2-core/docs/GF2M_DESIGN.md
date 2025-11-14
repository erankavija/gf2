# GF(2^m) Extension Field Arithmetic - Design Document

**Status**: Planned (Phase 8)  
**Priority**: HIGH - Blocks DVB-T2 FEC simulation in gf2-coding  
**Estimated Effort**: 2-3 weeks

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

### Phase 1: Core Field Arithmetic (Week 1)
- [ ] `Gf2mField` with primitive polynomial
- [ ] `Gf2mElement` with add/multiply/divide
- [ ] Standard field presets (GF(2^8), GF(2^16))
- [ ] Unit tests for field axioms

### Phase 2: Efficient Multiplication (Week 1-2)
- [ ] Log/antilog table generation
- [ ] Table-based multiplication for m ≤ 16
- [ ] Benchmarks vs. naive multiplication
- [ ] Property tests for arithmetic

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

1. **Table size vs. performance**: For which m should we switch from tables to computation?
2. **Memory allocation**: Pre-allocate tables or lazy initialization?
3. **Thread safety**: Should tables be shared across threads? (Arc<> wrapper?)
4. **API design**: Method syntax (a.multiply(b)) vs. operator overloading (a * b)?

## Future Extensions

- Reed-Solomon codes (generalization of BCH)
- Finite field FFT for fast polynomial multiplication
- Normal basis representation (alternative to polynomial basis)
- Optimal normal basis for hardware efficiency
- Composite field arithmetic (GF((2^m)^k))
