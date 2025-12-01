# GF(2^m) Extension Field Arithmetic

**Status**: ✅ Production Ready (Phase 3 Complete)  
**API**: `gf2m::Gf2mField`, `gf2m::Gf2mElement`, `gf2m::Gf2mPoly`

## Overview

Binary extension fields GF(2^m) are fundamental for algebraic error-correcting codes (BCH, Reed-Solomon) used in DVB-T2, DVB-S2, and similar standards.

**Implemented**:
- Field element representation and arithmetic (add, multiply, divide, exp)
- Table-based multiplication for m ≤ 16 (10× faster than schoolbook)
- Polynomial operations (add, multiply, divide, GCD, evaluation)
- Thread-safe field sharing (Arc-based)
- Standard field presets (GF(2^8), GF(2^16))

## Core Architecture

```rust
pub struct Gf2mField {
    params: Arc<FieldParams>,  // Thread-safe field parameters
}

pub struct Gf2mElement {
    value: u64,                // Polynomial representation (for m ≤ 64)
    params: Arc<FieldParams>,  // Shared field reference
}

pub struct Gf2mPoly {
    coeffs: Vec<Gf2mElement>,  // Coefficients (degree 0 to n)
}
```

### Field Parameters

**Primitive Polynomial**: Defines the field structure via modular reduction.

**Example**: GF(2^8) with `p(x) = x^8 + x^4 + x^3 + x + 1` (0x11D)

**Log/Antilog Tables** (m ≤ 16):
- `exp[i] = α^i` for i = 0..2^m-1
- `log[α^i] = i` (inverse mapping)
- **Memory**: ~1 KB for GF(2^8), ~262 KB for GF(2^16)
- **Performance**: O(1) multiplication vs O(m) schoolbook

## Field Arithmetic

### Addition/Subtraction
```rust
// XOR of polynomial representations
a + b = a ⊕ b
// Self-inverse property
a + a = 0
```

### Multiplication

**Small fields (m ≤ 16)**: Table-based (O(1))
```rust
let field = Gf2mField::new(8, 0b100011101).with_tables();
let a = field.element(0x53);
let b = field.element(0xCA);
let product = &a * &b;  // O(1) via log/antilog tables
```

**Large fields (m > 16)**: Schoolbook with modular reduction
```rust
let field = Gf2mField::new(32, primitive_poly);
let product = &a * &b;  // O(m) schoolbook algorithm
```

### Division and Exponentiation
```rust
// Division via Fermat's Little Theorem: a^(-1) = a^(2^m - 2)
let quotient = &a / &b;

// Square-and-multiply for a^k
let power = a.pow(k);  // O(log k) multiplications
```

## Polynomial Operations

### Construction
```rust
// From coefficients
let coeffs = vec![field.one(), field.zero(), field.element(0x42)];
let poly = Gf2mPoly::new(coeffs);  // 1 + 0·x + 0x42·x²

// Utility constructors (planned):
// - from_exponents(&[0, 2, 5]) → 1 + x² + x⁵
// - from_roots(&[α, α²]) → (x - α)(x - α²)
// - monomial(coeff, deg) → c·x^n
```

### Arithmetic
```rust
// Addition (coefficient-wise XOR)
let sum = &p1 + &p2;

// Multiplication (O(n²) schoolbook)
let product = &p1 * &p2;

// Division with remainder
let (quotient, remainder) = p1.div_rem(&p2);

// GCD (Euclidean algorithm)
let gcd = p1.gcd(&p2);

// Evaluation (Horner's method)
let result = poly.eval(&alpha);
```

## Thread Safety

**Status**: ✅ Send + Sync (Arc-based field sharing)

```rust
use std::thread;
use rayon::prelude::*;

let field = Gf2mField::new(8, 0b100011101).with_tables();

// Thread-safe field cloning
let handles: Vec<_> = (0..4)
    .map(|_| {
        let field = field.clone();  // Cheap Arc clone
        thread::spawn(move || {
            let a = field.element(0x42);
            let b = field.element(0x17);
            &a * &b
        })
    })
    .collect();

// Parallel batch operations
let results: Vec<_> = (0..1000)
    .into_par_iter()
    .map(|i| field.element(i as u64))
    .collect();
```

**Performance**: Arc overhead ~3.2ns per clone (negligible vs 100s-1000s ns operations)

## Standard Field Presets

```rust
// GF(2^8) - BCH(255, k), AES
let gf256 = Gf2mField::new(8, 0b100011101).with_tables();

// GF(2^16) - Reed-Solomon, CRC
let gf65536 = Gf2mField::new(16, 0b10001000000001011).with_tables();

// Custom field
let field = Gf2mField::new(14, 0b100000000100001);  // DVB-T2 BCH
```

## Performance Characteristics

### Field Operations (vs NTL/SageMath)

| Field | Operation | gf2-core | NTL | SageMath | Speedup vs NTL |
|-------|-----------|----------|-----|----------|----------------|
| GF(2^8) | Multiply | 2.70 ns | 46 ns | 308 µs | **17× faster** |
| GF(2^16) | Multiply | 4.41 ns | 56 ns | 308 µs | **13× faster** |
| GF(2^32) | Multiply | 52.2 ns | 99 ns | 338 µs | **1.9× faster** |
| GF(2^64) | Multiply | 204 ns | 103 ns | 803 µs | 0.5× (2× slower) |

**Key Insight**: Table-based approach dominates for m ≤ 16, competitive for m ≤ 32.

### Polynomial Operations

| Operation | Degree | gf2-core | SageMath | Speedup |
|-----------|--------|----------|----------|---------|
| Multiply (GF(2^8)[x]) | 100 | 63 µs | 15.5 ms | **246× faster** |
| Multiply (GF(2^16)[x]) | 100 | 93 µs | 15.6 ms | **168× faster** |

## Use Cases

### BCH Code Construction
```rust
// DVB-T2 BCH (16200, 16008, t=12)
let field = Gf2mField::new(14, 0b100000000100001).with_tables();
let alpha = field.primitive_element().unwrap();

// Generator polynomial from consecutive roots
// g(x) = (x - α)(x - α²)...(x - α^24)
let roots: Vec<_> = (1..=24)
    .map(|i| alpha.pow(i))
    .collect();
// let generator = Gf2mPoly::from_roots(&roots);  // Planned utility
```

### Reed-Solomon Codes
```rust
// Systematic encoding
let message_poly = Gf2mPoly::new(message_coeffs);
let (_, parity) = message_poly.div_rem(&generator);
```

### Syndrome Calculation
```rust
// Evaluate received polynomial at syndrome roots
let syndromes: Vec<_> = (1..=2*t)
    .map(|i| received_poly.eval(&alpha.pow(i)))
    .collect();
```

## API Guidelines

### Construction
- Use `with_tables()` for m ≤ 16 (10× speedup)
- Share `Gf2mField` instances via cheap Arc clones
- All elements from same field must use same field reference

### Operations
- Operators (`+`, `*`, `/`) work with both `&T` and `T`
- Division panics on zero divisor (use checked variants if needed)
- Polynomial normalization removes leading zeros automatically

### Performance Tips
- Batch operations with rayon for parallel speedup (6-8× on 12 cores)
- Reuse field instances (table generation is one-time cost)
- Use table-based multiplication for m ≤ 16

## Testing

**Coverage**: 66 unit tests + property-based tests with proptest

**Tests verify**:
- Field axioms (associativity, commutativity, distributivity)
- Identity elements (0 for addition, 1 for multiplication)
- Inverses (additive, multiplicative)
- Polynomial division invariant: `p = q·d + r`
- Table-based multiply matches schoolbook
- Thread safety (Send + Sync)

## Future Extensions

### Planned (Phase 4 - Optional)
- `Gf2mPoly::from_exponents()` - Sparse polynomial construction
- `Gf2mPoly::from_roots()` - Build from roots for BCH/RS
- Minimal polynomial computation
- Chien search for error locator polynomial

### Possible Optimizations
- SIMD acceleration for large fields (PCLMULQDQ)
- Karatsuba multiplication for high-degree polynomials
- FFT-based polynomial multiplication
- Normal basis representation

## References

- **Implementation**: `src/gf2m/`
- **Tests**: `cargo test --lib gf2m`
- **Examples**: Module-level documentation with educational introduction
- **Theory**: *Error Control Coding* by Lin & Costello
- **Standards**: ETSI EN 302 755 (DVB-T2), ETSI EN 302 307 (DVB-S2)
